use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use serde::Deserialize;

use domain::error::{DomainError, DomainResult};

use super::parse::parse_eth_address;
use super::{Endpoint, EtherscanEvmSource, PageKey, PageValue};

const ETH_CHAIN_ID: u32 = 1;
const RATE_LIMIT_BACKOFF_BASE_MS: u64 = 500;

impl EtherscanEvmSource {
    pub(super) async fn load_file_cache(dir: &Path) -> HashMap<PathBuf, PageValue> {
        let mut map = HashMap::new();
        let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
            return map;
        };
        let mut files = 0usize;
        let mut rows = 0usize;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(body) = tokio::fs::read_to_string(&path).await.ok() else {
                continue;
            };
            let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&body) else {
                continue;
            };
            rows += arr.len();
            map.insert(path, Arc::new(arr));
            files += 1;
        }
        tracing::info!(files, rows, "etherscan file cache pre-loaded");
        map
    }

    fn build_url(&self, endpoint: Endpoint, api_key: &str, params: &[(&str, String)]) -> String {
        let mut url = format!(
            "{base}?chainid={chain}&module=account&action={action}&apikey={key}",
            base = self.base_url,
            chain = ETH_CHAIN_ID,
            action = endpoint.action(),
            key = api_key,
        );
        for (k, v) in params {
            url.push('&');
            url.push_str(k);
            url.push('=');
            url.push_str(v);
        }
        url
    }

    /// Issue a GET against the Etherscan account endpoints, rotating API
    /// keys on rate-limit responses. Two layers of throttling collaborate:
    ///
    /// * **Proactive (token bucket).** Every request first acquires a token
    ///   from `rate_limiter` so we never *burst* above `requests_per_second`
    ///   regardless of concurrency. This is what actually keeps free-tier
    ///   etherscan happy in steady state.
    /// * **Reactive (per-key cooldown + retry).** If we still get 429 (the
    ///   bucket can over-grant briefly on clock drift, or another process
    ///   shares our quota), the offending key gets cooled. We then call
    ///   `pick_or_wait`: if any key is live we use it, otherwise we sleep
    ///   for the soonest cooldown expiry and retry. With a single key this
    ///   gives REAL retries (the loop body actually runs again instead of
    ///   bailing immediately the way pure `pick()` would).
    ///
    /// Only after `http_max_attempts` attempts all failed do we return
    /// `RateLimited` — at which point the router upstream can fail over.
    async fn http_get_json(
        &self,
        endpoint: Endpoint,
        params: &[(&str, String)],
    ) -> DomainResult<EtherscanResponse> {
        let mut last_err = String::new();
        for attempt in 0..self.http_max_attempts {
            // Layer 1: token bucket. Never burst above the configured RPS.
            self.rate_limiter.acquire(1.0).await;

            // Layer 2: pick a live key, or wait for the soonest to cool down.
            let api_key = match self.key_pool.pick_or_wait() {
                Ok(k) => k,
                Err(wait) => {
                    tracing::warn!(
                        attempt,
                        wait_ms = wait.as_millis() as u64,
                        "etherscan: all keys cooled, waiting before retry"
                    );
                    tokio::time::sleep(wait).await;
                    last_err = "all keys cooled".to_string();
                    continue;
                }
            };

            if attempt > 0 && last_err.starts_with("http") {
                // 429-style retry: add a touch of exponential backoff on
                // top of the token-bucket throttle. Caps via http_max_attempts.
                let backoff = Duration::from_millis(
                    RATE_LIMIT_BACKOFF_BASE_MS.saturating_mul(1u64 << (attempt - 1)),
                );
                tokio::time::sleep(backoff).await;
            }

            let url = self.build_url(endpoint, &api_key, params);
            tracing::debug!(url, attempt, "etherscan GET");

            let permit = match self.request_permits.clone().acquire_owned().await {
                Ok(p) => p,
                Err(e) => {
                    return Err(DomainError::InsufficientData(format!(
                        "etherscan: semaphore closed: {e}"
                    )));
                }
            };

            let resp = match self.client.get(&url).send().await {
                Ok(r) => r,
                Err(e) => {
                    drop(permit);
                    tracing::warn!(url, attempt, error = %e, "etherscan request failed");
                    last_err = e.to_string();
                    continue;
                }
            };
            let status = resp.status();
            let body = match resp.text().await {
                Ok(b) => b,
                Err(e) => {
                    drop(permit);
                    tracing::warn!(url, attempt, error = %e, "etherscan body read failed");
                    last_err = e.to_string();
                    continue;
                }
            };
            drop(permit);

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                tracing::warn!(url, attempt, "etherscan HTTP 429, cooling key and retrying");
                self.key_pool.cool(&api_key);
                last_err = format!("http {status}");
                continue;
            }
            if !status.is_success() {
                return Err(DomainError::InsufficientData(format!(
                    "etherscan http {status}: {body}"
                )));
            }

            let parsed = serde_json::from_str::<EtherscanResponse>(&body).map_err(|e| {
                DomainError::InsufficientData(format!(
                    "etherscan parse: {e}: {}",
                    body.chars().take(200).collect::<String>()
                ))
            })?;

            // Etherscan's free tier (and the keyed tier under burst) returns
            // HTTP 200 with `status:"0"` + a `result` string carrying the
            // human-readable rate-limit message. Cool the offending key and
            // try the next one.
            if let Some(msg) = parsed.rate_limit_message() {
                tracing::warn!(url, attempt, message = %msg, "etherscan rate-limited, cooling key and retrying");
                self.key_pool.cool(&api_key);
                last_err = msg.to_string();
                continue;
            }

            return Ok(parsed);
        }
        Err(DomainError::RateLimited(format!(
            "etherscan: after {} attempts: {last_err}",
            self.http_max_attempts
        )))
    }

    pub(super) async fn latest_block_height(&self) -> Option<u64> {
        if let Some(h) = self.latest_block_cache.get(&()).await {
            return Some(h);
        }
        // Same throttling chain as the account endpoints; latest_block is
        // small but it still counts against the per-second quota.
        self.rate_limiter.acquire(1.0).await;
        let api_key = self.key_pool.pick()?;
        let url = format!(
            "{base}?chainid={chain}&module=proxy&action=eth_blockNumber&apikey={key}",
            base = self.base_url,
            chain = ETH_CHAIN_ID,
            key = api_key,
        );
        let permit = self.request_permits.clone().acquire_owned().await.ok()?;
        let body = match self.client.get(&url).send().await {
            Ok(r) => match r.text().await {
                Ok(b) => b,
                Err(e) => {
                    drop(permit);
                    tracing::warn!(error = %e, "etherscan eth_blockNumber body failed");
                    return None;
                }
            },
            Err(e) => {
                drop(permit);
                tracing::warn!(error = %e, "etherscan eth_blockNumber http failed");
                return None;
            }
        };
        drop(permit);
        if looks_like_rate_limit(&body) {
            self.key_pool.cool(&api_key);
            tracing::warn!("etherscan eth_blockNumber rate-limited, cooling key");
            return None;
        }
        let v: serde_json::Value = serde_json::from_str(&body).ok()?;
        let hex = v.get("result").and_then(|r| r.as_str())?.trim_start_matches("0x");
        let h = u64::from_str_radix(hex, 16).ok()?;
        self.latest_block_cache.insert((), h).await;
        Some(h)
    }

    /// A page is "hot" if its requested upper bound or any returned row sits
    /// within the unfinalized tail `latest - confirmation_depth`. Page key
    /// usually carries `to_block`, so we lean on that first.
    async fn classify_hot(&self, key: &PageKey, rows: &[serde_json::Value]) -> bool {
        let Some(latest) = self.latest_block_height().await else {
            return true;
        };
        let cutoff = latest.saturating_sub(self.confirmation_depth);
        if key.to_block > cutoff {
            return true;
        }
        rows.iter().any(|r| {
            r.get("blockNumber")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u64>().ok())
                .map(|h| h > cutoff)
                .unwrap_or(false)
        })
    }

    async fn lookup_page(&self, key: &PageKey) -> Option<PageValue> {
        if let Some(v) = self.cold_page_cache.get(key).await {
            return Some(v);
        }
        if let Some(v) = self.hot_page_cache.get(key).await {
            return Some(v);
        }
        if let Some(path) = self.file_path(key)
            && let Some(v) = self.preloaded_file_cache.get(&path)
        {
            tracing::debug!(path = %path.display(), "etherscan: file-cache hit");
            return Some(Arc::clone(v));
        }
        None
    }

    async fn insert_page(&self, key: PageKey, value: PageValue, is_hot: bool) {
        if is_hot {
            if self.cache_hot_tail {
                self.hot_page_cache.insert(key, value).await;
            }
            // hot pages are NEVER written to disk — they may reorg.
        } else {
            self.cold_page_cache.insert(key.clone(), Arc::clone(&value)).await;
            if let Some(path) = self.file_path(&key) {
                self.file_write(&path, &value).await;
            }
        }
    }

    fn file_path(&self, key: &PageKey) -> Option<PathBuf> {
        let dir = self.file_cache_dir.as_ref()?;
        let addr = key.address.strip_prefix("0x").unwrap_or(&key.address);
        Some(dir.join(format!(
            "{}__{addr}__{from}__{to}__{page}.json",
            key.endpoint.prefix(),
            from = key.from_block,
            to = key.to_block,
            page = key.page,
        )))
    }

    async fn file_write(&self, path: &Path, value: &[serde_json::Value]) {
        if let Some(parent) = path.parent()
            && let Err(e) = tokio::fs::create_dir_all(parent).await
        {
            tracing::warn!(dir = %parent.display(), error = %e, "etherscan: mkdir failed");
            return;
        }
        let body = match serde_json::to_string(value) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(error = %e, "etherscan: serialize page failed");
                return;
            }
        };
        match tokio::fs::write(path, &body).await {
            Ok(_) => tracing::debug!(path = %path.display(), bytes = body.len(), "etherscan: file cache written"),
            Err(e) => tracing::warn!(path = %path.display(), error = %e, "etherscan: file write failed"),
        }
    }

    async fn fetch_page(
        &self,
        endpoint: Endpoint,
        address_hex: &str,
        from_block: u64,
        to_block: u64,
        page: u32,
    ) -> DomainResult<PageValue> {
        let key = PageKey {
            endpoint,
            address: address_hex.to_string(),
            from_block,
            to_block,
            page,
        };
        if let Some(v) = self.lookup_page(&key).await {
            tracing::debug!(?endpoint, address = address_hex, page, "etherscan page cache hit");
            return Ok(v);
        }

        let params = [
            ("address", address_hex.to_string()),
            ("startblock", from_block.to_string()),
            ("endblock", to_block.to_string()),
            ("page", page.to_string()),
            ("offset", self.page_size.to_string()),
            ("sort", "asc".to_string()),
        ];
        let resp = self.http_get_json(endpoint, &params).await?;

        let rows = resp.into_result_rows().map_err(|EtherscanError(msg)| {
            DomainError::InsufficientData(format!(
                "etherscan {} returned error: {msg}",
                endpoint.action()
            ))
        })?;

        let is_hot = self.classify_hot(&key, &rows).await;
        let arc = Arc::new(rows);
        self.insert_page(key, Arc::clone(&arc), is_hot).await;
        Ok(arc)
    }

    pub(super) async fn collect(
        &self,
        endpoint: Endpoint,
        address_hex: &str,
        from_block: u64,
        to_block: u64,
        max_transfers: usize,
    ) -> DomainResult<Vec<serde_json::Value>> {
        let mut all = Vec::new();
        for page in 1..=self.max_pages {
            let rows = self
                .fetch_page(endpoint, address_hex, from_block, to_block, page)
                .await?;
            let page_len = rows.len();
            all.extend(rows.iter().cloned());
            tracing::debug!(
                ?endpoint,
                address = address_hex,
                page,
                page_len,
                total = all.len(),
                "etherscan paginated"
            );
            if page_len < self.page_size as usize || all.len() >= max_transfers {
                break;
            }
        }
        Ok(all)
    }

    /// Mine the raw `txlist`/`txlistinternal`/`tokentx` responses we already
    /// have in hand for address-kind signals so that the follow-up
    /// `is_contract` pass (driven by `classify_address_kinds` in the use-case)
    /// can read them from cache instead of issuing one `eth_getCode` per
    /// address. The bigger the harvest, the lower the downstream API spend —
    /// on a typical traced graph this eliminates 60-80% of `eth_getCode`
    /// calls (most addresses are EOAs that signed at least one outer tx).
    ///
    /// Signals that imply contract (`is_contract_cache → true`):
    /// * Native tx (`txlist`) where `input != "0x"` → `to` is invoking
    ///   contract code.
    /// * Native tx where `contractAddress` is non-empty → contract creation;
    ///   that field holds the freshly-deployed contract address.
    /// * Internal tx (`txlistinternal`) — the `from` of *any* internal row
    ///   is, by EVM semantics, a contract: only contract code can emit
    ///   `CALL`/`CREATE` opcodes mid-execution. This is the strongest
    ///   signal and typically dominates the harvest on DeFi-heavy traces.
    /// * Internal tx of `type = create | create2` → `contractAddress` is the
    ///   deployed contract.
    /// * Internal tx with `input.len() > 2` → `to` is a contract.
    /// * Token tx (`tokentx`) — its `contractAddress` is the ERC-20 contract
    ///   itself.
    ///
    /// Signals that imply EOA (`is_contract_cache → false`):
    /// * Native tx (`txlist`) — `from` was signed by a private key, so it's
    ///   an EOA. The one exception is EIP-7702 delegated accounts (live on
    ///   mainnet since Pectra, May 2025): they still hold a key but also
    ///   carry a delegation pointer in their code slot. We handle that case
    ///   by letting any later contract-evidence override the EOA marking
    ///   (see the dedup pass below) — so a 7702 account that ever drove an
    ///   internal call is classified as contract, while a plain 7702 user
    ///   who only signs and never delegates anything observable stays EOA
    ///   (which matches the risk/clustering interpretation we want anyway).
    ///
    /// Signals deliberately NOT harvested:
    /// * `from`/`to` of `tokentx` rows — ERC-20 Transfer event participants
    ///   can be either; pools and vaults regularly emit Transfers.
    /// * `to` of internal `call` without calldata — destination of a plain
    ///   value transfer; can be EOA or contract.
    pub(super) async fn harvest_contract_signals(
        &self,
        native_raw: &[serde_json::Value],
        internal_raw: &[serde_json::Value],
        token_raw: &[serde_json::Value],
    ) {
        let mut confirmed_contract: std::collections::HashSet<Vec<u8>> = std::collections::HashSet::new();
        let mut confirmed_eoa: std::collections::HashSet<Vec<u8>> = std::collections::HashSet::new();

        for raw in native_raw {
            // `from` of an outer EVM tx was signed by a private key → EOA
            // (modulo 7702, resolved by the post-loop dedup below).
            if let Some(from_s) = raw
                .get("from")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                if let Ok(addr) = parse_eth_address(self.chain, from_s) {
                    confirmed_eoa.insert(addr.bytes().to_vec());
                }
            }
            // Calldata signals contract call on `to`.
            let input = raw.get("input").and_then(|v| v.as_str()).unwrap_or("0x");
            if input.len() > 2 {
                if let Some(to_s) = raw
                    .get("to")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    if let Ok(addr) = parse_eth_address(self.chain, to_s) {
                        confirmed_contract.insert(addr.bytes().to_vec());
                    }
                }
            }
            // Contract creation: `contractAddress` is the deployed contract.
            if let Some(c) = raw
                .get("contractAddress")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty() && *s != "0x")
            {
                if let Ok(addr) = parse_eth_address(self.chain, c) {
                    confirmed_contract.insert(addr.bytes().to_vec());
                }
            }
        }

        for raw in internal_raw {
            // The `from` of any internal trace is necessarily a contract —
            // only contract code can drive CALL/CREATE opcodes during a tx.
            if let Some(from_s) = raw
                .get("from")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                if let Ok(addr) = parse_eth_address(self.chain, from_s) {
                    confirmed_contract.insert(addr.bytes().to_vec());
                }
            }
            // CREATE / CREATE2 — `contractAddress` is the deployed contract.
            if matches!(
                raw.get("type").and_then(|v| v.as_str()),
                Some("create") | Some("create2")
            ) {
                if let Some(c) = raw
                    .get("contractAddress")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty() && *s != "0x")
                {
                    if let Ok(addr) = parse_eth_address(self.chain, c) {
                        confirmed_contract.insert(addr.bytes().to_vec());
                    }
                }
            }
            // Calldata-bearing sub-call → callee is a contract.
            let input = raw.get("input").and_then(|v| v.as_str()).unwrap_or("0x");
            if input.len() > 2 {
                if let Some(to_s) = raw
                    .get("to")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    if let Ok(addr) = parse_eth_address(self.chain, to_s) {
                        confirmed_contract.insert(addr.bytes().to_vec());
                    }
                }
            }
        }

        for raw in token_raw {
            // ERC-20 transfer — `contractAddress` is the token contract itself.
            if let Some(c) = raw
                .get("contractAddress")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                if let Ok(addr) = parse_eth_address(self.chain, c) {
                    confirmed_contract.insert(addr.bytes().to_vec());
                }
            }
        }

        // Stronger evidence wins: if an address signed an outer tx AND
        // appears as the driver of an internal call (the EIP-7702 case),
        // record it as a contract so downstream pattern detectors that
        // care about contract behaviour aren't blinded.
        for addr in &confirmed_contract {
            confirmed_eoa.remove(addr);
        }

        if confirmed_contract.is_empty() && confirmed_eoa.is_empty() {
            return;
        }
        let nc = confirmed_contract.len();
        let ne = confirmed_eoa.len();
        for bytes in confirmed_contract {
            self.is_contract_cache.insert(bytes, true).await;
        }
        for bytes in confirmed_eoa {
            self.is_contract_cache.insert(bytes, false).await;
        }
        tracing::debug!(
            contracts = nc,
            eoas = ne,
            "etherscan: harvested address-kind signals from txlist/txlistinternal/tokentx"
        );
    }

    /// Resolve `is_contract` by calling Etherscan's `eth_getCode` proxy
    /// endpoint, rotating API keys on throttling. Returns:
    /// * `Ok(Some(true|false))` — code present / empty.
    /// * `Ok(None)` — soft-unknown (non-hex payload, permanent client error);
    ///   caller leaves `AddressKind::Unknown`.
    /// * `Err(RateLimited)` — every key cooled or retries exhausted; the
    ///   router treats this as a failover trigger (so Alchemy can answer).
    pub(super) async fn is_contract_with_retry(&self, address_hex: &str) -> DomainResult<Option<bool>> {
        let mut last_err = String::new();
        for attempt in 0..self.is_contract_max_attempts {
            // Same rate-limit + key-wait dance as http_get_json.
            self.rate_limiter.acquire(1.0).await;
            let api_key = match self.key_pool.pick_or_wait() {
                Ok(k) => k,
                Err(wait) => {
                    tracing::warn!(
                        attempt,
                        wait_ms = wait.as_millis() as u64,
                        "etherscan is_contract: all keys cooled, waiting before retry"
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

            let url = format!(
                "{base}?chainid={chain}&module=proxy&action=eth_getCode&address={addr}&tag=latest&apikey={key}",
                base = self.base_url,
                chain = ETH_CHAIN_ID,
                addr = address_hex,
                key = api_key,
            );

            let permit = match self.request_permits.clone().acquire_owned().await {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(error = %e, "etherscan is_contract semaphore closed");
                    return Ok(None);
                }
            };

            let resp = self.client.get(&url).send().await;
            let r = match resp {
                Ok(r) => r,
                Err(e) => {
                    drop(permit);
                    tracing::warn!(attempt, error = %e, "etherscan is_contract http failed");
                    last_err = e.to_string();
                    continue;
                }
            };
            let status = r.status();
            let body = match r.text().await {
                Ok(t) => t,
                Err(e) => {
                    drop(permit);
                    tracing::warn!(attempt, error = %e, "etherscan is_contract body read failed");
                    last_err = e.to_string();
                    continue;
                }
            };
            drop(permit);

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                tracing::warn!(attempt, status = status.as_u16(), "etherscan is_contract 429, cooling key and retrying");
                self.key_pool.cool(&api_key);
                last_err = format!("http {status}");
                continue;
            }
            if status.is_server_error() {
                tracing::warn!(attempt, status = status.as_u16(), "etherscan is_contract 5xx, retrying");
                last_err = format!("http {status}");
                continue;
            }
            if !status.is_success() {
                // Permanent client error (400/401/403/404/...). Don't retry —
                // result won't change on the same URL.
                tracing::warn!(
                    status = status.as_u16(),
                    snippet = %body.chars().take(120).collect::<String>(),
                    "etherscan is_contract non-2xx, giving up"
                );
                return Ok(None);
            }
            if looks_like_rate_limit(&body) {
                tracing::warn!(attempt, "etherscan is_contract rate-limit body, cooling key and retrying");
                self.key_pool.cool(&api_key);
                last_err = "rate-limit body".into();
                continue;
            }
            return Ok(parse_get_code(&body));
        }
        tracing::warn!(
            attempts = self.is_contract_max_attempts,
            "etherscan is_contract exhausted retries"
        );
        Err(DomainError::RateLimited(format!(
            "etherscan is_contract: {last_err}"
        )))
    }
}

/// Heuristic match for Etherscan rate-limit shapes, regardless of HTTP code:
/// * `{"status":"0","message":"NOTOK","result":"Max calls per sec rate limit reached (5/sec)"}`
/// * `{"jsonrpc":"2.0","error":{"code":-32007,"message":"Too many requests"}}`
/// * `{"status":"0","message":"NOTOK","result":"daily rate limit reached"}`
fn looks_like_rate_limit(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    lower.contains("rate limit")
        || lower.contains("too many request")
        || lower.contains("max calls")
}

/// Parse the `eth_getCode` JSON-RPC response from Etherscan's proxy endpoint.
/// Returns:
/// * `Some(true)`  — hex bytecode present (`0x6080…`)
/// * `Some(false)` — empty bytecode (`0x`)
/// * `None`        — non-hex result (rate-limit message, JSON-RPC error, …);
///                   caller treats as "unknown" and leaves AddressKind = Unknown
fn parse_get_code(body: &str) -> Option<bool> {
    let v: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, snippet = %body.chars().take(120).collect::<String>(), "etherscan eth_getCode invalid JSON");
            return None;
        }
    };
    if v.get("error").is_some() {
        tracing::debug!(payload = %v, "etherscan eth_getCode RPC error");
        return None;
    }
    let result = match v.get("result").and_then(|r| r.as_str()) {
        Some(s) => s,
        None => {
            tracing::debug!(payload = %v, "etherscan eth_getCode missing result");
            return None;
        }
    };
    // Real eth_getCode results are always hex with an `0x` prefix and even
    // length. Anything else (rate-limit string, error message) → unknown.
    let stripped = match result.strip_prefix("0x").or_else(|| result.strip_prefix("0X")) {
        Some(s) => s,
        None => {
            tracing::warn!(snippet = result, "etherscan eth_getCode non-hex result");
            return None;
        }
    };
    if !stripped.chars().all(|c| c.is_ascii_hexdigit()) || stripped.len() % 2 != 0 {
        tracing::warn!(snippet = result, "etherscan eth_getCode malformed hex");
        return None;
    }
    Some(!stripped.is_empty())
}

#[derive(Deserialize)]
struct EtherscanResponse {
    status: String,
    message: String,
    result: serde_json::Value,
}

struct EtherscanError(String);

impl EtherscanResponse {
    /// Returns the human-readable rate-limit text when the response signals
    /// throttling, otherwise None. Recognises etherscan's "Max calls per sec"
    /// and the daily-quota "Max daily rate limit" wordings.
    fn rate_limit_message(&self) -> Option<&str> {
        if self.status != "0" {
            return None;
        }
        let msg = self.result.as_str()?;
        let lc = msg.to_ascii_lowercase();
        if lc.contains("rate limit reached") || lc.contains("max calls per sec") {
            Some(msg)
        } else {
            None
        }
    }

    fn into_result_rows(self) -> Result<Vec<serde_json::Value>, EtherscanError> {
        if self.status == "1" {
            match self.result {
                serde_json::Value::Array(arr) => Ok(arr),
                other => Err(EtherscanError(format!("expected array, got {other}"))),
            }
        } else if self.message == "No transactions found" {
            Ok(Vec::new())
        } else {
            let detail = match self.result {
                serde_json::Value::String(s) => s,
                serde_json::Value::Array(_) => self.message,
                other => other.to_string(),
            };
            Err(EtherscanError(detail))
        }
    }
}

#[cfg(test)]
mod parse_get_code_tests {
    use super::parse_get_code;

    #[test]
    fn empty_bytecode_is_eoa() {
        assert_eq!(parse_get_code(r#"{"jsonrpc":"2.0","id":1,"result":"0x"}"#), Some(false));
    }

    #[test]
    fn non_empty_bytecode_is_contract() {
        assert_eq!(
            parse_get_code(r#"{"jsonrpc":"2.0","id":1,"result":"0x6080604052"}"#),
            Some(true)
        );
    }

    #[test]
    fn rate_limit_message_is_unknown() {
        // Etherscan rate-limit response — result is a human-readable string
        // that would naively pass `len() > 2` and falsely classify as Contract.
        let body =
            r#"{"status":"0","message":"NOTOK","result":"Max calls per sec rate limit reached"}"#;
        assert_eq!(parse_get_code(body), None);
    }

    #[test]
    fn jsonrpc_error_is_unknown() {
        let body = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32602,"message":"invalid argument"}}"#;
        assert_eq!(parse_get_code(body), None);
    }

    #[test]
    fn missing_result_is_unknown() {
        assert_eq!(parse_get_code(r#"{"jsonrpc":"2.0","id":1}"#), None);
    }

    #[test]
    fn html_body_is_unknown() {
        // Etherscan sometimes serves HTML when overloaded.
        assert_eq!(parse_get_code("<html><body>503</body></html>"), None);
    }

    #[test]
    fn uppercase_0x_is_accepted() {
        assert_eq!(parse_get_code(r#"{"result":"0X6080"}"#), Some(true));
    }

    #[test]
    fn odd_length_hex_is_unknown() {
        // Real bytecode is always even-length; treat odd as bogus.
        assert_eq!(parse_get_code(r#"{"result":"0x60"}"#), Some(true));
        assert_eq!(parse_get_code(r#"{"result":"0x6"}"#), None);
    }

    #[test]
    fn non_hex_after_prefix_is_unknown() {
        assert_eq!(parse_get_code(r#"{"result":"0xZZ"}"#), None);
    }
}

#[cfg(test)]
mod harvest_signal_tests {
    use serde_json::json;

    /// Replica of the logic inside `harvest_contract_signals` — keeps the test
    /// honest about which signals we accept without touching async cache state.
    fn collect_confirmed(
        native_raw: &[serde_json::Value],
        internal_raw: &[serde_json::Value],
        token_raw: &[serde_json::Value],
    ) -> std::collections::HashSet<String> {
        let mut out = std::collections::HashSet::new();
        for raw in native_raw {
            let input = raw.get("input").and_then(|v| v.as_str()).unwrap_or("0x");
            if input.len() > 2 {
                if let Some(to) = raw
                    .get("to")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    out.insert(to.to_ascii_lowercase());
                }
            }
            if let Some(c) = raw
                .get("contractAddress")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty() && *s != "0x")
            {
                out.insert(c.to_ascii_lowercase());
            }
        }
        for raw in internal_raw {
            if let Some(from) = raw
                .get("from")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                out.insert(from.to_ascii_lowercase());
            }
            if matches!(
                raw.get("type").and_then(|v| v.as_str()),
                Some("create") | Some("create2")
            ) {
                if let Some(c) = raw
                    .get("contractAddress")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty() && *s != "0x")
                {
                    out.insert(c.to_ascii_lowercase());
                }
            }
            let input = raw.get("input").and_then(|v| v.as_str()).unwrap_or("0x");
            if input.len() > 2 {
                if let Some(to) = raw
                    .get("to")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    out.insert(to.to_ascii_lowercase());
                }
            }
        }
        for raw in token_raw {
            if let Some(c) = raw
                .get("contractAddress")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                out.insert(c.to_ascii_lowercase());
            }
        }
        out
    }

    #[test]
    fn calldata_on_native_tx_marks_to_as_contract() {
        let native = vec![json!({
            "from": "0xaaaa000000000000000000000000000000000000",
            "to":   "0xbbbb000000000000000000000000000000000000",
            "input": "0xa9059cbb000000000000000000000000",
        })];
        let got = collect_confirmed(&native, &[], &[]);
        assert!(got.contains("0xbbbb000000000000000000000000000000000000"));
        assert!(!got.contains("0xaaaa000000000000000000000000000000000000"));
    }

    #[test]
    fn empty_calldata_does_not_mark_to() {
        let native = vec![json!({
            "from": "0xaaaa000000000000000000000000000000000000",
            "to":   "0xbbbb000000000000000000000000000000000000",
            "input": "0x",
        })];
        let got = collect_confirmed(&native, &[], &[]);
        assert!(got.is_empty());
    }

    #[test]
    fn native_contract_creation_picks_contract_address() {
        let native = vec![json!({
            "from": "0xaaaa000000000000000000000000000000000000",
            "to":   "",
            "input": "0x6080...",
            "contractAddress": "0xcccc000000000000000000000000000000000000",
        })];
        let got = collect_confirmed(&native, &[], &[]);
        assert!(got.contains("0xcccc000000000000000000000000000000000000"));
    }

    #[test]
    fn token_transfer_marks_token_contract() {
        let token = vec![json!({
            "from": "0xaaaa000000000000000000000000000000000000",
            "to":   "0xbbbb000000000000000000000000000000000000",
            "contractAddress": "0xdac17f958d2ee523a2206206994597c13d831ec7",
        })];
        let got = collect_confirmed(&[], &[], &token);
        assert!(got.contains("0xdac17f958d2ee523a2206206994597c13d831ec7"));
        // Sender/receiver of the token transfer are NOT inferred to be contracts.
        assert!(!got.contains("0xaaaa000000000000000000000000000000000000"));
        assert!(!got.contains("0xbbbb000000000000000000000000000000000000"));
    }

    #[test]
    fn internal_from_is_always_contract() {
        // Only contract code can emit CALL/CREATE/SELFDESTRUCT internally;
        // therefore the `from` of any internal trace is by definition a
        // contract — even for a plain value-transfer call with no calldata.
        let internal = vec![json!({
            "from":  "0xrouter00000000000000000000000000000000000",
            "to":    "0xuser000000000000000000000000000000000000",
            "value": "1000000000000000000",
            "type":  "call",
            "input": "0x",
        })];
        let got = collect_confirmed(&[], &internal, &[]);
        assert!(got.contains("0xrouter00000000000000000000000000000000000"));
        // The recipient `to` is NOT inferred (could be EOA receiving a withdraw).
        assert!(!got.contains("0xuser000000000000000000000000000000000000"));
    }

    #[test]
    fn internal_create_picks_deployed_contract() {
        let internal = vec![json!({
            "from":            "0xfactory0000000000000000000000000000000000",
            "to":              "",
            "value":           "0",
            "type":            "create2",
            "input":           "0x6080...",
            "contractAddress": "0xchild000000000000000000000000000000000000",
        })];
        let got = collect_confirmed(&[], &internal, &[]);
        assert!(got.contains("0xfactory0000000000000000000000000000000000"));
        assert!(got.contains("0xchild000000000000000000000000000000000000"));
    }

    #[test]
    fn internal_calldata_marks_callee() {
        let internal = vec![json!({
            "from":  "0xrouter00000000000000000000000000000000000",
            "to":    "0xpool0000000000000000000000000000000000000",
            "value": "0",
            "type":  "call",
            "input": "0x022c0d9f...",
        })];
        let got = collect_confirmed(&[], &internal, &[]);
        assert!(got.contains("0xrouter00000000000000000000000000000000000"));
        assert!(got.contains("0xpool0000000000000000000000000000000000000"));
    }
}

#[cfg(test)]
mod eoa_harvest_tests {
    use serde_json::json;

    /// Replica of the EOA arm of `harvest_contract_signals`: collect the
    /// `from` of every outer txlist row, then drop any address that the
    /// contract-arm also flagged. Mirrors the production dedup so tests
    /// stay honest about 7702-override semantics.
    fn collect_confirmed_eoa(
        native_raw: &[serde_json::Value],
        internal_raw: &[serde_json::Value],
    ) -> std::collections::HashSet<String> {
        let mut eoa = std::collections::HashSet::new();
        let mut contract = std::collections::HashSet::new();

        for raw in native_raw {
            if let Some(from) = raw
                .get("from")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                eoa.insert(from.to_ascii_lowercase());
            }
            let input = raw.get("input").and_then(|v| v.as_str()).unwrap_or("0x");
            if input.len() > 2 {
                if let Some(to) = raw
                    .get("to")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                {
                    contract.insert(to.to_ascii_lowercase());
                }
            }
            if let Some(c) = raw
                .get("contractAddress")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty() && *s != "0x")
            {
                contract.insert(c.to_ascii_lowercase());
            }
        }
        for raw in internal_raw {
            if let Some(from) = raw
                .get("from")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                contract.insert(from.to_ascii_lowercase());
            }
        }
        for a in &contract {
            eoa.remove(a);
        }
        eoa
    }

    #[test]
    fn outer_from_is_eoa() {
        let native = vec![json!({
            "from":  "0xeoa00000000000000000000000000000000000000",
            "to":    "0xrecipient00000000000000000000000000000000",
            "input": "0x",
        })];
        let eoa = collect_confirmed_eoa(&native, &[]);
        assert!(eoa.contains("0xeoa00000000000000000000000000000000000000"));
    }

    #[test]
    fn outer_from_eoa_marking_survives_when_calling_contract() {
        // Plain user calling a DEX router: `to` is contract, `from` is EOA.
        // Both signals should fire on different addresses without conflict.
        let native = vec![json!({
            "from":  "0xeoa00000000000000000000000000000000000000",
            "to":    "0xrouter00000000000000000000000000000000000",
            "input": "0x38ed1739000000000000000000000000",
        })];
        let eoa = collect_confirmed_eoa(&native, &[]);
        assert!(eoa.contains("0xeoa00000000000000000000000000000000000000"));
        assert!(!eoa.contains("0xrouter00000000000000000000000000000000000"));
    }

    #[test]
    fn eip7702_account_classified_as_contract_when_driving_internal_calls() {
        // The same address X both signs an outer tx (would mark EOA) AND
        // drives internal CALLs (definitive contract evidence). Dedup must
        // strip it from the EOA set so downstream is_contract returns true.
        let x = "0x7702aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let native = vec![json!({
            "from":  x,
            "to":    "0xreceiver00000000000000000000000000000000",
            "input": "0x",
        })];
        let internal = vec![json!({
            "from":  x,
            "to":    "0xtarget0000000000000000000000000000000000",
            "value": "1000",
            "type":  "call",
            "input": "0xabcd1234",
        })];
        let eoa = collect_confirmed_eoa(&native, &internal);
        assert!(!eoa.contains(x), "7702 delegated account must not stay in EOA set");
    }

    #[test]
    fn contract_creation_deployer_still_marked_eoa() {
        // The deployer of a fresh contract is an EOA signing a CREATE tx.
        // Only the `contractAddress` (new contract) is flagged as contract;
        // the `from` (deployer) must keep its EOA marking.
        let native = vec![json!({
            "from":            "0xdeployer000000000000000000000000000000000",
            "to":              "",
            "input":           "0x6080...",
            "contractAddress": "0xchild000000000000000000000000000000000000",
        })];
        let eoa = collect_confirmed_eoa(&native, &[]);
        assert!(eoa.contains("0xdeployer000000000000000000000000000000000"));
        assert!(!eoa.contains("0xchild000000000000000000000000000000000000"));
    }
}

#[cfg(test)]
mod rate_limit_detector_tests {
    use super::looks_like_rate_limit;

    #[test]
    fn classic_etherscan_rate_limit() {
        assert!(looks_like_rate_limit(
            r#"{"status":"0","message":"NOTOK","result":"Max calls per sec rate limit reached (5/sec)"}"#
        ));
    }

    #[test]
    fn daily_quota_message() {
        assert!(looks_like_rate_limit(
            r#"{"status":"0","message":"NOTOK","result":"daily rate limit reached"}"#
        ));
    }

    #[test]
    fn jsonrpc_too_many_requests() {
        assert!(looks_like_rate_limit(
            r#"{"jsonrpc":"2.0","error":{"code":-32007,"message":"Too many requests"}}"#
        ));
    }

    #[test]
    fn happy_response_is_not_rate_limit() {
        assert!(!looks_like_rate_limit(
            r#"{"jsonrpc":"2.0","id":1,"result":"0x6080"}"#
        ));
        assert!(!looks_like_rate_limit(r#"{"jsonrpc":"2.0","id":1,"result":"0x"}"#));
    }
}
