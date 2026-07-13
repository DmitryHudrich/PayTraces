use std::{sync::Arc, time::Duration};

use chrono::TimeZone;
use serde_json::json;

use domain::error::{DomainError, DomainResult};
use domain::primitives::Address;

use super::mapping::{build_transfer_topics, is_log_response_too_large};
use super::parse::parse_hex_u64;
use super::{AlchemyEvmSource, PageKey, PageValue};

const RATE_LIMIT_BACKOFF_BASE_MS: u64 = 400;

/// CU costs per JSON-RPC method, as billed by Alchemy on the public
/// compute-unit table. We're conservative — when in doubt, pick the
/// higher of two documented numbers so we under-spend the bucket rather
/// than risk silent throttling. Batched calls multiply by the inner count.
fn cu_for_method(method: &str) -> f64 {
    match method {
        "eth_blockNumber" => 10.0,
        "eth_getCode" => 19.0,
        "eth_getBlockByNumber" => 16.0,
        "eth_call" => 26.0,
        // eth_getLogs varies — Alchemy charges per block range. 75 is the
        // standard rate without enhanced filters; if you hit logs heavy
        // workloads, tune up.
        "eth_getLogs" => 75.0,
        "trace_filter" => 75.0,
        "trace_block" => 75.0,
        // Conservative default for anything we forget — better to slightly
        // under-utilise than to silently exceed and trigger throttling.
        _ => 30.0,
    }
}

impl AlchemyEvmSource {
    /// Single point of contact with Alchemy: builds the per-key URL, sends
    /// the JSON-RPC envelope, parses the response. Two layers of throttle:
    /// * token-bucket rate limiter (`requests_per_second`) — proactive;
    /// * per-key cooldown + `pick_or_wait` — reactive on observed 429/RPC
    ///   rate-limit, with REAL retries (loop continues after sleep instead
    ///   of bailing the first time the only key cools).
    pub(super) async fn jsonrpc_call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> DomainResult<serde_json::Value> {
        let mut last_err = String::new();
        let cost = cu_for_method(method);
        for attempt in 0..self.http_max_attempts {
            // Layer 1: CU-priced token bucket. Each single-method call
            // pays its method's CU cost so the bucket reflects Alchemy's
            // actual billing dimension, not raw request count.
            self.rate_limiter.acquire(cost).await;

            // Layer 2: live key or wait for soonest cooldown to lapse.
            let api_key = match self.key_pool.pick_or_wait() {
                Ok(k) => k,
                Err(wait) => {
                    tracing::warn!(
                        method,
                        attempt,
                        wait_ms = wait.as_millis() as u64,
                        "alchemy: all keys cooled, waiting before retry"
                    );
                    tokio::time::sleep(wait).await;
                    last_err = "all keys cooled".to_string();
                    continue;
                }
            };

            if attempt > 0 && last_err.starts_with("http") {
                let backoff = Duration::from_millis(
                    RATE_LIMIT_BACKOFF_BASE_MS.saturating_mul(1u64 << (attempt - 1)),
                );
                tokio::time::sleep(backoff).await;
            }

            let url = format!("{}/{}", self.base_url.trim_end_matches('/'), api_key);
            let body = json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": method,
                "params": params,
            });

            let permit = match self.request_permits.clone().acquire_owned().await {
                Ok(p) => p,
                Err(e) => {
                    return Err(DomainError::InsufficientData(format!(
                        "alchemy: semaphore closed: {e}"
                    )));
                }
            };

            let resp = match self.client.post(&url).json(&body).send().await {
                Ok(r) => r,
                Err(e) => {
                    drop(permit);
                    tracing::warn!(method, attempt, error = %e, "alchemy http failed");
                    last_err = e.to_string();
                    continue;
                }
            };
            let status = resp.status();
            let text = match resp.text().await {
                Ok(t) => t,
                Err(e) => {
                    drop(permit);
                    tracing::warn!(method, attempt, error = %e, "alchemy body read failed");
                    last_err = e.to_string();
                    continue;
                }
            };
            drop(permit);

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                tracing::warn!(method, attempt, "alchemy HTTP 429, cooling key");
                self.key_pool.cool(&api_key);
                last_err = format!("http {status}");
                continue;
            }
            if status.is_server_error() {
                tracing::warn!(method, attempt, status = status.as_u16(), "alchemy 5xx, retrying");
                last_err = format!("http {status}");
                continue;
            }
            if !status.is_success() {
                return Err(DomainError::InsufficientData(format!(
                    "alchemy {method}: http {status}: {}",
                    text.chars().take(200).collect::<String>()
                )));
            }

            let v: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => {
                    return Err(DomainError::InsufficientData(format!(
                        "alchemy {method} parse: {e}: {}",
                        text.chars().take(200).collect::<String>()
                    )));
                }
            };

            if let Some(err) = v.get("error") {
                let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
                let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("");
                // Standard JSON-RPC rate-limit codes Alchemy uses.
                if code == 429 || code == -32007 || msg.to_ascii_lowercase().contains("rate limit")
                {
                    tracing::warn!(method, attempt, code, msg, "alchemy rpc rate-limited, cooling key");
                    self.key_pool.cool(&api_key);
                    last_err = msg.to_string();
                    continue;
                }
                return Err(DomainError::InsufficientData(format!(
                    "alchemy {method} rpc error {code}: {msg}"
                )));
            }

            return Ok(v.get("result").cloned().unwrap_or(serde_json::Value::Null));
        }
        Err(DomainError::RateLimited(format!(
            "alchemy {method}: after {} attempts: {last_err}",
            self.http_max_attempts
        )))
    }

    pub(super) async fn eth_block_number(&self) -> DomainResult<u64> {
        if let Some(h) = self.latest_block_cache.get(&()).await {
            return Ok(h);
        }
        let res = self.jsonrpc_call("eth_blockNumber", json!([])).await?;
        let hex = res
            .as_str()
            .ok_or_else(|| DomainError::InsufficientData("alchemy eth_blockNumber: not a hex string".into()))?;
        let h = parse_hex_u64(hex)
            .map_err(|e| DomainError::InsufficientData(format!("alchemy eth_blockNumber: {e}")))?;
        self.latest_block_cache.insert((), h).await;
        Ok(h)
    }

    /// Send a JSON-RPC batch request (array of envelopes) — Alchemy
    /// accepts up to several hundred per call. Each envelope has its own
    /// `id`; we preserve them so the caller can stitch results by index.
    /// Throttling and retries mirror `jsonrpc_call`: one token from the
    /// rate limiter for the entire batch, key rotation on rate-limit,
    /// `http_max_attempts` retries.
    pub(super) async fn jsonrpc_batch_call(
        &self,
        batch: &[serde_json::Value],
    ) -> DomainResult<Vec<serde_json::Value>> {
        let mut last_err = String::new();
        // A JSON-RPC batch costs the SUM of inner methods' CUs — Alchemy
        // bills each inner call, even when wrapped in one HTTP envelope.
        // Inspect each request's `method` to price correctly; default to
        // a conservative per-call CU when not detectable.
        let cost: f64 = batch
            .iter()
            .map(|req| {
                req.get("method")
                    .and_then(|v| v.as_str())
                    .map(cu_for_method)
                    .unwrap_or(30.0)
            })
            .sum();
        for attempt in 0..self.http_max_attempts {
            self.rate_limiter.acquire(cost).await;
            let api_key = match self.key_pool.pick_or_wait() {
                Ok(k) => k,
                Err(wait) => {
                    tracing::warn!(
                        attempt,
                        wait_ms = wait.as_millis() as u64,
                        batch_cu = cost,
                        "alchemy batch: all keys cooled, waiting before retry"
                    );
                    tokio::time::sleep(wait).await;
                    last_err = "all keys cooled".to_string();
                    continue;
                }
            };

            if attempt > 0 && last_err.starts_with("http") {
                let backoff = Duration::from_millis(
                    RATE_LIMIT_BACKOFF_BASE_MS.saturating_mul(1u64 << (attempt - 1)),
                );
                tokio::time::sleep(backoff).await;
            }

            let url = format!("{}/{}", self.base_url.trim_end_matches('/'), api_key);
            let permit = match self.request_permits.clone().acquire_owned().await {
                Ok(p) => p,
                Err(e) => {
                    return Err(DomainError::InsufficientData(format!(
                        "alchemy batch: semaphore closed: {e}"
                    )));
                }
            };

            let resp = match self.client.post(&url).json(&batch).send().await {
                Ok(r) => r,
                Err(e) => {
                    drop(permit);
                    tracing::warn!(attempt, error = %e, "alchemy batch http failed");
                    last_err = e.to_string();
                    continue;
                }
            };
            let status = resp.status();
            let text = match resp.text().await {
                Ok(t) => t,
                Err(e) => {
                    drop(permit);
                    tracing::warn!(attempt, error = %e, "alchemy batch body read failed");
                    last_err = e.to_string();
                    continue;
                }
            };
            drop(permit);

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                tracing::warn!(attempt, "alchemy batch HTTP 429, cooling key");
                self.key_pool.cool(&api_key);
                last_err = format!("http {status}");
                continue;
            }
            if status.is_server_error() {
                tracing::warn!(attempt, status = status.as_u16(), "alchemy batch 5xx, retrying");
                last_err = format!("http {status}");
                continue;
            }
            if !status.is_success() {
                return Err(DomainError::InsufficientData(format!(
                    "alchemy batch: http {status}: {}",
                    text.chars().take(200).collect::<String>()
                )));
            }

            let v: serde_json::Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => {
                    return Err(DomainError::InsufficientData(format!(
                        "alchemy batch parse: {e}: {}",
                        text.chars().take(200).collect::<String>()
                    )));
                }
            };
            let arr = match v.as_array() {
                Some(a) => a.clone(),
                None => {
                    return Err(DomainError::InsufficientData(format!(
                        "alchemy batch: expected array, got: {}",
                        text.chars().take(200).collect::<String>()
                    )));
                }
            };
            return Ok(arr);
        }
        Err(DomainError::RateLimited(format!(
            "alchemy batch: after {} attempts: {last_err}",
            self.http_max_attempts
        )))
    }

    /// Bulk `eth_getCode` via JSON-RPC batch with per-entry retry for
    /// inner 429s. Alchemy explicitly documents that batched entries may
    /// individually exceed CU/s and return `{"code": 429, "message": "...
    /// you can safely ignore this message"}` — they expect the client to
    /// retry those entries. We do exactly that: each pass sends only the
    /// addresses still unresolved (or specifically inner-429'd), with
    /// fresh batch-local IDs, and the rate limiter slows us between
    /// passes so the retry actually has new budget.
    ///
    /// Capped at `BATCH_INNER_MAX_ATTEMPTS` so a persistently-broken
    /// downstream (genuinely malformed responses, RPC errors that aren't
    /// 429) eventually settles to "soft-unknown" instead of looping.
    pub(super) async fn eth_get_code_batch(
        &self,
        addresses_hex: &[String],
    ) -> DomainResult<Vec<Option<bool>>> {
        const BATCH_INNER_MAX_ATTEMPTS: u8 = 3;

        if addresses_hex.is_empty() {
            return Ok(Vec::new());
        }
        let total = addresses_hex.len();
        let mut out: Vec<Option<bool>> = vec![None; total];
        // Map from "batch-local id we sent" → "original output slot".
        // Refreshed every attempt because we re-id only the unresolved.
        let mut remaining: Vec<usize> = (0..total).collect();

        let mut last_response_len: usize = 0;
        let mut last_sample: String = String::new();

        for attempt in 0..BATCH_INNER_MAX_ATTEMPTS {
            if remaining.is_empty() {
                break;
            }
            let batch: Vec<serde_json::Value> = remaining
                .iter()
                .enumerate()
                .map(|(local_id, &orig_slot)| {
                    json!({
                        "jsonrpc": "2.0",
                        "id": local_id,
                        "method": "eth_getCode",
                        "params": [&addresses_hex[orig_slot], "latest"],
                    })
                })
                .collect();

            let arr = self.jsonrpc_batch_call(&batch).await?;
            last_response_len = arr.len();
            if last_sample.is_empty() {
                if let Some(first) = arr.first() {
                    last_sample = first.to_string();
                }
            }

            let mut next_remaining: Vec<usize> = Vec::new();
            let mut seen_local: std::collections::HashSet<usize> =
                std::collections::HashSet::new();
            let mut inner_429 = 0usize;
            let mut errors = 0usize;
            let mut null_results = 0usize;
            let mut id_out_of_range = 0usize;
            let mut id_unparseable = 0usize;
            let mut malformed = 0usize;
            let mut empty_code = 0usize;
            let mut with_code = 0usize;

            for entry in &arr {
                let id_u64 = entry
                    .get("id")
                    .and_then(|v| v.as_u64())
                    .or_else(|| {
                        entry
                            .get("id")
                            .and_then(|v| v.as_i64())
                            .and_then(|n| u64::try_from(n).ok())
                    })
                    .or_else(|| {
                        entry
                            .get("id")
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse().ok())
                    });
                let Some(id_u64) = id_u64 else {
                    id_unparseable += 1;
                    continue;
                };
                let local_id = id_u64 as usize;
                if local_id >= remaining.len() {
                    id_out_of_range += 1;
                    continue;
                }
                seen_local.insert(local_id);
                let orig_slot = remaining[local_id];

                if let Some(err) = entry.get("error") {
                    let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
                    if code == 429 {
                        inner_429 += 1;
                        next_remaining.push(orig_slot);
                        if inner_429 <= 3 {
                            tracing::debug!(
                                local_id,
                                orig_slot,
                                attempt,
                                "alchemy batch entry inner 429 — will retry"
                            );
                        }
                    } else {
                        errors += 1;
                        if errors <= 3 {
                            tracing::debug!(
                                local_id,
                                orig_slot,
                                ?err,
                                "alchemy batch entry returned non-429 RPC error — leaving Unknown"
                            );
                        }
                    }
                    continue;
                }

                let Some(s) = entry.get("result").and_then(|v| v.as_str()) else {
                    null_results += 1;
                    continue;
                };
                let stripped = match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                    Some(s) => s,
                    None => {
                        malformed += 1;
                        continue;
                    }
                };
                if !stripped.chars().all(|c| c.is_ascii_hexdigit()) || stripped.len() % 2 != 0 {
                    malformed += 1;
                    continue;
                }
                if stripped.is_empty() {
                    empty_code += 1;
                    out[orig_slot] = Some(false);
                } else {
                    with_code += 1;
                    out[orig_slot] = Some(true);
                }
            }

            // Any entry in `remaining` that we never saw a response for —
            // Alchemy returned a short array. Treat as needs-retry too.
            for (local_id, &orig_slot) in remaining.iter().enumerate() {
                if !seen_local.contains(&local_id) && out[orig_slot].is_none() {
                    next_remaining.push(orig_slot);
                }
            }
            next_remaining.sort();
            next_remaining.dedup();

            if inner_429 > 0 || !next_remaining.is_empty() {
                tracing::debug!(
                    attempt,
                    sent = remaining.len(),
                    response_len = arr.len(),
                    ok = empty_code + with_code,
                    inner_429,
                    errors,
                    null_results,
                    id_out_of_range,
                    id_unparseable,
                    malformed,
                    next_attempt_size = next_remaining.len(),
                    "alchemy batch attempt finished"
                );
            }

            // Sanity check: if NOTHING resolved AND nothing was 429, the
            // shape is wrong — break early so the caller's sticky fallback
            // kicks in instead of looping at full cost.
            let resolved_this_pass = empty_code + with_code;
            if attempt == 0
                && resolved_this_pass == 0
                && inner_429 == 0
                && !remaining.is_empty()
            {
                tracing::warn!(
                    batch_size = remaining.len(),
                    response_len = arr.len(),
                    errors,
                    null_results,
                    id_out_of_range,
                    id_unparseable,
                    malformed,
                    sample_entry = %last_sample.chars().take(300).collect::<String>(),
                    "alchemy batch: 0 resolved AND 0 inner-429 on first pass — likely a shape issue, breaking out"
                );
                break;
            }

            remaining = next_remaining;
        }

        let unresolved = out.iter().filter(|x| x.is_none()).count();
        if unresolved == total && total > 0 {
            tracing::warn!(
                batch_size = total,
                last_response_len,
                sample_entry = %last_sample.chars().take(300).collect::<String>(),
                "alchemy batch: ALL entries still unresolved after retries"
            );
        }

        Ok(out)
    }

    pub(super) async fn eth_get_code(&self, address_hex: &str) -> DomainResult<Option<bool>> {
        let res = self
            .jsonrpc_call("eth_getCode", json!([address_hex, "latest"]))
            .await?;
        let s = match res.as_str() {
            Some(s) => s,
            None => return Ok(None),
        };
        // Mirror Etherscan's parse_get_code semantics: `0x` → EOA, `0x...`
        // (non-empty even-hex) → contract, anything else → soft-unknown.
        let stripped = match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            Some(s) => s,
            None => return Ok(None),
        };
        if !stripped.chars().all(|c| c.is_ascii_hexdigit()) || stripped.len() % 2 != 0 {
            return Ok(None);
        }
        Ok(Some(!stripped.is_empty()))
    }

    /// A page is "hot" if its upper bound sits inside the unfinalized tail
    /// `latest - confirmation_depth`. Anything older is finalized and safe
    /// to cache for `cold_ttl`.
    pub(super) async fn classify_hot(&self, key: &PageKey) -> bool {
        let latest = match self.eth_block_number().await {
            Ok(h) => h,
            Err(_) => return true,
        };
        let cutoff = latest.saturating_sub(self.confirmation_depth);
        key.to_block() > cutoff
    }

    pub(super) async fn lookup_page(&self, key: &PageKey) -> Option<PageValue> {
        if let Some(v) = self.cold_page_cache.get(key).await {
            return Some(v);
        }
        self.hot_page_cache.get(key).await
    }

    pub(super) async fn insert_page(&self, key: PageKey, value: PageValue, is_hot: bool) {
        if is_hot {
            if self.cache_hot_tail {
                self.hot_page_cache.insert(key, value).await;
            }
            // Hot pages never enter the cold cache — they may reorg.
        } else {
            self.cold_page_cache.insert(key, value).await;
        }
    }

    /// Fetch one `trace_filter` page, consulting the page cache first.
    /// Cache hits are returned as-is; misses fall through to the network
    /// and store under the right (cold/hot) cache based on classify_hot.
    pub(super) async fn trace_filter_page(
        &self,
        key: PageKey,
    ) -> DomainResult<PageValue> {
        if let Some(v) = self.lookup_page(&key).await {
            tracing::debug!(?key, "alchemy trace_filter page cache hit");
            return Ok(v);
        }

        let (address, filter_field, from_block, to_block, after) = match &key {
            PageKey::TraceFrom { address, from_block, to_block, after } => {
                (address.clone(), "fromAddress", *from_block, *to_block, *after)
            }
            PageKey::TraceTo { address, from_block, to_block, after } => {
                (address.clone(), "toAddress", *from_block, *to_block, *after)
            }
            _ => unreachable!("trace_filter_page called with non-trace key"),
        };
        let mut filter = serde_json::Map::new();
        filter.insert("fromBlock".into(), json!(format!("0x{from_block:x}")));
        filter.insert("toBlock".into(), json!(format!("0x{to_block:x}")));
        filter.insert(filter_field.into(), json!([address]));
        filter.insert("after".into(), json!(after));
        filter.insert("count".into(), json!(self.trace_page_size));

        let res = self.jsonrpc_call("trace_filter", json!([filter])).await?;
        let rows = res.as_array().cloned().unwrap_or_default();
        let arc = Arc::new(rows);

        let is_hot = self.classify_hot(&key).await;
        self.insert_page(key, Arc::clone(&arc), is_hot).await;
        Ok(arc)
    }

    /// Paginate `trace_filter` results until empty page, `count` cap, or
    /// `max_traces` budget. Each page is cached individually so repeat BFS
    /// passes hit the cache before issuing HTTP.
    pub(super) async fn trace_filter_by_address(
        &self,
        from_block: u64,
        to_block: u64,
        filter_key: &str,
        address_hex: &str,
        max_traces: usize,
    ) -> DomainResult<Vec<serde_json::Value>> {
        let mut out: Vec<serde_json::Value> = Vec::new();
        let mut after: u32 = 0;
        for page in 0..self.trace_max_pages {
            let key = match filter_key {
                "fromAddress" => PageKey::TraceFrom {
                    address: address_hex.to_string(),
                    from_block,
                    to_block,
                    after,
                },
                "toAddress" => PageKey::TraceTo {
                    address: address_hex.to_string(),
                    from_block,
                    to_block,
                    after,
                },
                other => {
                    return Err(DomainError::InsufficientData(format!(
                        "alchemy: unknown trace filter key '{other}'"
                    )));
                }
            };
            let rows = self.trace_filter_page(key).await?;
            let n = rows.len();
            out.extend(rows.iter().cloned());
            tracing::debug!(
                filter_key,
                address_hex,
                page,
                page_len = n,
                total = out.len(),
                "alchemy trace_filter paginated"
            );
            if n < self.trace_page_size as usize || out.len() >= max_traces {
                break;
            }
            after = after.saturating_add(self.trace_page_size);
        }
        Ok(out)
    }

    /// Issue one `eth_getLogs` chunk, consulting the page cache first.
    /// Returns the network/cache rows wrapped in Arc; if the upstream
    /// flagged the window as too large, propagates `InsufficientData` so
    /// the bisection loop above can subdivide and retry.
    pub(super) async fn get_logs_chunk(
        &self,
        lo: u64,
        hi: u64,
        topic_from: Option<&str>,
        topic_to: Option<&str>,
    ) -> DomainResult<PageValue> {
        let key = match (topic_from, topic_to) {
            (Some(t), None) => PageKey::LogsFrom {
                topic: t.to_string(),
                from_block: lo,
                to_block: hi,
            },
            (None, Some(t)) => PageKey::LogsTo {
                topic: t.to_string(),
                from_block: lo,
                to_block: hi,
            },
            // Caller always sends exactly one side; reject otherwise so we
            // don't quietly cache mixed-shape entries.
            _ => {
                return Err(DomainError::InsufficientData(
                    "alchemy get_logs_chunk: exactly one of topic_from/topic_to is required".into(),
                ));
            }
        };

        if let Some(v) = self.lookup_page(&key).await {
            tracing::debug!(?key, "alchemy eth_getLogs page cache hit");
            return Ok(v);
        }

        let topics = build_transfer_topics(topic_from, topic_to);
        let filter = json!({
            "fromBlock": format!("0x{lo:x}"),
            "toBlock": format!("0x{hi:x}"),
            "topics": topics,
        });
        let res = self.jsonrpc_call("eth_getLogs", json!([filter])).await?;
        let rows = res.as_array().cloned().unwrap_or_default();
        let arc = Arc::new(rows);

        let is_hot = self.classify_hot(&key).await;
        self.insert_page(key, Arc::clone(&arc), is_hot).await;
        Ok(arc)
    }

    /// Recursive `eth_getLogs` over `(from, to)`, bisecting the block range
    /// on response-size errors. `topic_from` / `topic_to` are padded 32-byte
    /// hex addresses placed into topic1 / topic2 respectively; passing one
    /// implements a from-side or to-side filter (Alchemy supports OR within
    /// a topic slot but not across — hence two separate calls upstream).
    pub(super) async fn get_logs_chunked(
        &self,
        from_block: u64,
        to_block: u64,
        topic_from: Option<&str>,
        topic_to: Option<&str>,
        max_logs: usize,
    ) -> DomainResult<Vec<serde_json::Value>> {
        let mut stack: Vec<(u64, u64)> = vec![(from_block, to_block)];
        let mut out: Vec<serde_json::Value> = Vec::new();
        while let Some((lo, hi)) = stack.pop() {
            if out.len() >= max_logs {
                break;
            }
            // Optimistically respect the configured chunk size; on response
            // overflow we'll bisect via the error branch below.
            let actual_hi = hi.min(lo.saturating_add(self.log_chunk_blocks.saturating_sub(1)));
            let queue_rest = actual_hi < hi;

            match self.get_logs_chunk(lo, actual_hi, topic_from, topic_to).await {
                Ok(rows) => {
                    let n = rows.len();
                    out.extend(rows.iter().cloned());
                    tracing::debug!(
                        from = lo,
                        to = actual_hi,
                        page_len = n,
                        total = out.len(),
                        "alchemy eth_getLogs chunk"
                    );
                    if queue_rest {
                        stack.push((actual_hi + 1, hi));
                    }
                }
                Err(DomainError::InsufficientData(msg)) if is_log_response_too_large(&msg) => {
                    let span = actual_hi.saturating_sub(lo);
                    if span < self.min_log_chunk_blocks {
                        return Err(DomainError::InsufficientData(format!(
                            "alchemy eth_getLogs: cannot subdivide below {} blocks: {msg}",
                            self.min_log_chunk_blocks
                        )));
                    }
                    let mid = lo + span / 2;
                    tracing::warn!(
                        from = lo,
                        to = actual_hi,
                        mid,
                        "alchemy eth_getLogs response too large, bisecting"
                    );
                    if queue_rest {
                        stack.push((actual_hi + 1, hi));
                    }
                    stack.push((mid + 1, actual_hi));
                    stack.push((lo, mid));
                }
                Err(e) => return Err(e),
            }
        }
        Ok(out)
    }

    /// Parallel single-shot fallback when the batch path can't be trusted.
    /// `out` carries the cache hits we already resolved; we only fan out
    /// for `needs_fetch_idx` entries so we don't re-fetch cached ones.
    pub(super) async fn fan_out_individual(
        &self,
        addrs: &[Address],
        mut out: Vec<Option<bool>>,
        needs_fetch_idx: &[usize],
    ) -> DomainResult<Vec<Option<bool>>> {
        use futures::future::join_all;
        let futs = needs_fetch_idx.iter().map(|i| {
            let addr = &addrs[*i];
            let addr_hex = format!("0x{}", hex::encode(addr.bytes()));
            async move { (*i, self.eth_get_code(&addr_hex).await) }
        });
        let results = join_all(futs).await;
        for (slot, res) in results {
            match res {
                Ok(Some(v)) => {
                    let bytes = addrs[slot].bytes().to_vec();
                    self.is_contract_cache.insert(bytes, v).await;
                    out[slot] = Some(v);
                }
                Ok(None) => {}
                Err(_) => {} // soft-unknown; do not sink the whole batch
            }
        }
        Ok(out)
    }

    pub(super) async fn block_timestamp(&self, height: u64) -> DomainResult<chrono::DateTime<chrono::Utc>> {
        let res = self
            .jsonrpc_call(
                "eth_getBlockByNumber",
                json!([format!("0x{:x}", height), false]),
            )
            .await?;
        let ts_hex = res
            .get("timestamp")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DomainError::InsufficientData("alchemy: block missing timestamp".into()))?;
        let ts_secs = parse_hex_u64(ts_hex)
            .map_err(|e| DomainError::InsufficientData(format!("alchemy block timestamp: {e}")))?
            as i64;
        chrono::Utc
            .timestamp_opt(ts_secs, 0)
            .single()
            .ok_or_else(|| DomainError::InsufficientData(format!("alchemy: bad timestamp {ts_secs}")))
    }
}
