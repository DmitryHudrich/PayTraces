use std::sync::atomic::{AtomicBool, Ordering};
use std::{collections::HashMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use chrono::TimeZone;
use moka::future::Cache;
use serde_json::json;
use tokio::sync::Semaphore;

use domain::{
    chain::ChainId,
    error::{DomainError, DomainResult},
    ports::{BlockRange, ChainSource},
    primitives::{Address, BlockRef},
    transfer::{NormalizedBlock, Transfer},
};

use crate::key_pool::KeyPool;
use crate::rate_limiter::RateLimiter;

mod config;
mod http;
mod mapping;
mod parse;

pub use config::{AlchemyEvmConfig, DEFAULT_BASE_URL};

const TRANSFER_TOPIC: &str =
    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

/// Structured cache key for the two paginated upstream endpoints
/// (`trace_filter` per address-side, `eth_getLogs` per topic-slot side).
/// Block range is part of the key so disjoint windows don't collide, and
/// `after` (trace_filter pagination cursor) keeps successive pages
/// independent. ERC-20 logs don't paginate via a cursor — each chunk
/// resolved by the bisection loop is keyed by its `(lo, hi)` window.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PageKey {
    TraceFrom {
        address: String,
        from_block: u64,
        to_block: u64,
        after: u32,
    },
    TraceTo {
        address: String,
        from_block: u64,
        to_block: u64,
        after: u32,
    },
    LogsFrom {
        topic: String,
        from_block: u64,
        to_block: u64,
    },
    LogsTo {
        topic: String,
        from_block: u64,
        to_block: u64,
    },
}

impl PageKey {
    /// Upper block bound — used by `classify_hot` to decide whether the
    /// page can still reorg.
    fn to_block(&self) -> u64 {
        match self {
            PageKey::TraceFrom { to_block, .. }
            | PageKey::TraceTo { to_block, .. }
            | PageKey::LogsFrom { to_block, .. }
            | PageKey::LogsTo { to_block, .. } => *to_block,
        }
    }
}

type PageValue = Arc<Vec<serde_json::Value>>;

#[derive(Clone)]
pub struct AlchemyEvmSource {
    chain: ChainId,
    key_pool: KeyPool,
    base_url: String,
    enable_transfers: bool,
    log_chunk_blocks: u64,
    min_log_chunk_blocks: u64,
    trace_page_size: u32,
    trace_max_pages: u32,
    cache_hot_tail: bool,
    confirmation_depth: u64,
    http_max_attempts: u8,
    client: reqwest::Client,
    latest_block_cache: Cache<(), u64>,
    is_contract_cache: Cache<Vec<u8>, bool>,
    cold_page_cache: Cache<PageKey, PageValue>,
    hot_page_cache: Cache<PageKey, PageValue>,
    request_permits: Arc<Semaphore>,
    rate_limiter: Arc<RateLimiter>,
    /// Once a JSON-RPC `eth_getCode` batch resolves zero of N entries we
    /// flip this and stop sending batches for the rest of the process'
    /// lifetime — individual calls with rate-limit + cache still work and
    /// don't burn round-trips that demonstrably return nothing useful.
    batch_eth_get_code_disabled: Arc<AtomicBool>,
}

impl AlchemyEvmSource {
    pub fn new(chain: ChainId, client: reqwest::Client, cfg: AlchemyEvmConfig) -> Self {
        assert!(
            cfg.has_keys(),
            "AlchemyEvmSource: at least one api key required — config validation must guard this"
        );
        let key_pool = KeyPool::new(cfg.api_keys.clone(), cfg.key_cooldown);

        let latest_block_cache = Cache::builder()
            .max_capacity(1)
            .time_to_live(cfg.latest_block_ttl)
            .build();

        // Contract code is immutable in practice (post-Cancun SELFDESTRUCT
        // is a no-op for code removal); cache aggressively. 7d mirrors the
        // Etherscan source so the routed pipeline behaves consistently.
        let is_contract_cache = Cache::builder()
            .max_capacity(100_000)
            .time_to_live(Duration::from_secs(7 * 24 * 3600))
            .build();

        // Cold pages: finalized data, long TTL. Hot pages: still-reorgable
        // tail, short TTL (or disabled via cache_hot_tail). Capacity is
        // weighed by the number of rows in the page so large traces don't
        // crowd small ones out of the cache.
        let cold_page_cache = Cache::builder()
            .max_capacity(cfg.page_max_capacity)
            .weigher(|_k: &PageKey, v: &PageValue| v.len().max(1) as u32)
            .time_to_live(cfg.cold_ttl)
            .build();
        let hot_page_cache = Cache::builder()
            .max_capacity(cfg.page_max_capacity)
            .weigher(|_k: &PageKey, v: &PageValue| v.len().max(1) as u32)
            .time_to_live(cfg.hot_ttl)
            .build();

        let request_permits = Arc::new(Semaphore::new(cfg.max_concurrent_requests as usize));
        let rate_limiter = Arc::new(RateLimiter::new(
            cfg.requests_per_second,
            cfg.requests_per_second_burst,
        ));

        tracing::info!(
            base_url = %cfg.base_url,
            api_keys = key_pool.len(),
            key_cooldown_secs = cfg.key_cooldown.as_secs(),
            latest_block_ttl_secs = cfg.latest_block_ttl.as_secs(),
            max_concurrent_requests = cfg.max_concurrent_requests,
            enable_transfers = cfg.enable_transfers,
            log_chunk_blocks = cfg.log_chunk_blocks,
            min_log_chunk_blocks = cfg.min_log_chunk_blocks,
            trace_page_size = cfg.trace_page_size,
            cold_ttl_secs = cfg.cold_ttl.as_secs(),
            hot_ttl_secs = cfg.hot_ttl.as_secs(),
            cache_hot_tail = cfg.cache_hot_tail,
            confirmation_depth = cfg.confirmation_depth,
            page_max_capacity = cfg.page_max_capacity,
            requests_per_second = cfg.requests_per_second,
            requests_per_second_burst = cfg.requests_per_second_burst,
            http_max_attempts = cfg.http_max_attempts,
            chain_id = chain.value(),
            "Alchemy EVM source initialized"
        );

        Self {
            chain,
            key_pool,
            base_url: cfg.base_url,
            enable_transfers: cfg.enable_transfers,
            log_chunk_blocks: cfg.log_chunk_blocks,
            min_log_chunk_blocks: cfg.min_log_chunk_blocks,
            trace_page_size: cfg.trace_page_size,
            trace_max_pages: cfg.trace_max_pages,
            cache_hot_tail: cfg.cache_hot_tail,
            confirmation_depth: cfg.confirmation_depth,
            http_max_attempts: cfg.http_max_attempts,
            client,
            latest_block_cache,
            is_contract_cache,
            cold_page_cache,
            hot_page_cache,
            request_permits,
            rate_limiter,
            batch_eth_get_code_disabled: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl ChainSource for AlchemyEvmSource {
    fn chain_id(&self) -> ChainId {
        self.chain
    }

    async fn latest_block(&self) -> DomainResult<BlockRef> {
        let h = self.eth_block_number().await?;
        Ok(BlockRef::new(self.chain, h, [0u8; 32]))
    }

    async fn fetch_block(&self, height: u64) -> DomainResult<NormalizedBlock> {
        // Native value transfers come from the block body; ERC-20 from
        // Transfer-topic logs filtered to this single block. Fan both
        // requests out in parallel — they're independent.
        let block_call = self.jsonrpc_call(
            "eth_getBlockByNumber",
            json!([format!("0x{height:x}"), true]),
        );
        let logs_filter = json!({
            "fromBlock": format!("0x{height:x}"),
            "toBlock":   format!("0x{height:x}"),
            "topics":    [TRANSFER_TOPIC, null, null],
        });
        let logs_call = self.jsonrpc_call("eth_getLogs", json!([logs_filter]));
        let (block_v, logs_v) = tokio::try_join!(block_call, logs_call)?;
        let log_rows = logs_v.as_array().cloned().unwrap_or_default();

        // Block envelope: hash + timestamp + the transaction list.
        let block_hash_s = block_v
            .get("hash")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DomainError::InsufficientData("alchemy fetch_block: missing block hash".into()))?;
        let ts_hex = block_v
            .get("timestamp")
            .and_then(|v| v.as_str())
            .ok_or_else(|| DomainError::InsufficientData("alchemy fetch_block: missing timestamp".into()))?;
        let block_hash = parse::parse_hash32(block_hash_s)
            .map_err(|e| DomainError::InsufficientData(format!("alchemy fetch_block: block hash: {e}")))?;
        let ts_secs = parse::parse_hex_u64(ts_hex)
            .map_err(|e| DomainError::InsufficientData(format!("alchemy fetch_block: timestamp: {e}")))?
            as i64;
        let timestamp = chrono::Utc
            .timestamp_opt(ts_secs, 0)
            .single()
            .ok_or_else(|| DomainError::InsufficientData(format!("alchemy fetch_block: bad timestamp {ts_secs}")))?;

        let block_ref = BlockRef::new(self.chain, height, block_hash);
        let mut block_ts: HashMap<u64, chrono::DateTime<chrono::Utc>> = HashMap::new();
        block_ts.insert(height, timestamp);
        let mut by_tx: HashMap<[u8; 32], u32> = HashMap::new();
        let mut transfers: Vec<Transfer> = Vec::new();

        // Native value transfers from the body. The outer tx claims idx=0
        // per the existing convention (matches Etherscan source); idx 1+
        // is reserved for inner traces and ERC-20 events in the same tx.
        if let Some(txs) = block_v.get("transactions").and_then(|v| v.as_array()) {
            for raw in txs {
                match mapping::map_native_tx_to_transfer(self.chain, raw, height, block_hash, timestamp, &mut by_tx) {
                    Ok(Some(t)) => transfers.push(t),
                    Ok(None) => {}
                    Err(e) => tracing::warn!(error = %e, "alchemy fetch_block: skip malformed tx"),
                }
            }
        }

        // ERC-20 Transfer events in the same block. Each log claims the
        // next free idx within its tx so we never collide on the PK.
        for raw in log_rows {
            match mapping::map_log_to_transfer(self.chain, &raw, &block_ts, &mut by_tx) {
                Ok(Some(t)) => transfers.push(t),
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "alchemy fetch_block: skip malformed log"),
            }
        }

        transfers.sort_by_key(|t| t.id().index());
        Ok(NormalizedBlock::new(block_ref, timestamp, transfers))
    }

    async fn transfers_for_address(
        &self,
        addr: &Address,
        range: BlockRange,
        max_transfers: usize,
    ) -> DomainResult<Vec<Transfer>> {
        if !self.enable_transfers {
            return Err(DomainError::InsufficientData(
                "alchemy: transfers disabled (set alchemy.enable_transfers=true to enable)".into(),
            ));
        }
        if addr.chain() != self.chain {
            return Err(DomainError::InsufficientData(format!(
                "alchemy source (chain={}) called with foreign address chain: {}",
                self.chain,
                addr.chain()
            )));
        }
        let address_hex = format!("0x{}", hex::encode(addr.bytes()));
        let padded = format!("0x{:0>64}", hex::encode(addr.bytes()));

        let from_block = range.from_height();
        let to_block = if range.to_height() == u64::MAX {
            self.eth_block_number().await?
        } else {
            range.to_height()
        };

        tracing::info!(
            address = %address_hex,
            from_block,
            to_block,
            max_transfers,
            "alchemy: fetching ETH transfers"
        );

        let (traces_from, traces_to, logs_from, logs_to) = tokio::try_join!(
            self.trace_filter_by_address(from_block, to_block, "fromAddress", &address_hex, max_transfers),
            self.trace_filter_by_address(from_block, to_block, "toAddress", &address_hex, max_transfers),
            self.get_logs_chunked(from_block, to_block, Some(&padded), None, max_transfers),
            self.get_logs_chunked(from_block, to_block, None, Some(&padded), max_transfers),
        )?;

        // Map block timestamps from blockNumber → we'd normally need a
        // per-block lookup but trace_filter returns blockNumber and logs do
        // too; we cache timestamps in-flight via a per-call map populated
        // lazily through eth_getBlockByNumber. This adds at most one call
        // per distinct block touched in the result — acceptable in MVP.
        let mut block_ts: HashMap<u64, chrono::DateTime<chrono::Utc>> = HashMap::new();

        // Native: merge traces dedupe by (tx_hash, traceAddress).
        let mut traces: HashMap<(Vec<u8>, String), serde_json::Value> = HashMap::new();
        for raw in traces_from.into_iter().chain(traces_to.into_iter()) {
            let tx_hash_s = match raw.get("transactionHash").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => continue,
            };
            let tx_hash_bytes = match hex::decode(tx_hash_s.trim_start_matches("0x")) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let trace_addr = raw
                .get("traceAddress")
                .map(|v| v.to_string())
                .unwrap_or_default();
            traces.entry((tx_hash_bytes, trace_addr)).or_insert(raw);
        }

        // ERC-20 logs: dedupe by (tx_hash, logIndex).
        let mut logs: HashMap<(Vec<u8>, u64), serde_json::Value> = HashMap::new();
        for raw in logs_from.into_iter().chain(logs_to.into_iter()) {
            let tx_hash_s = match raw.get("transactionHash").and_then(|v| v.as_str()) {
                Some(s) => s,
                None => continue,
            };
            let tx_hash_bytes = match hex::decode(tx_hash_s.trim_start_matches("0x")) {
                Ok(b) => b,
                Err(_) => continue,
            };
            let log_idx_s = raw.get("logIndex").and_then(|v| v.as_str()).unwrap_or("0x0");
            let log_idx = parse::parse_hex_u64(log_idx_s).unwrap_or(0);
            logs.entry((tx_hash_bytes, log_idx)).or_insert(raw);
        }

        let unique_blocks: std::collections::HashSet<u64> = traces
            .values()
            .filter_map(|t| t.get("blockNumber").and_then(|v| v.as_u64()).or_else(|| {
                t.get("blockNumber").and_then(|v| v.as_str()).and_then(|s| parse::parse_hex_u64(s).ok())
            }))
            .chain(logs.values().filter_map(|l| {
                l.get("blockNumber").and_then(|v| v.as_str()).and_then(|s| parse::parse_hex_u64(s).ok())
            }))
            .collect();

        for height in unique_blocks {
            if let Ok(ts) = self.block_timestamp(height).await {
                block_ts.insert(height, ts);
            }
        }

        let mut by_tx: HashMap<[u8; 32], u32> = HashMap::new();
        let mut out: Vec<Transfer> = Vec::with_capacity(traces.len() + logs.len());

        for (_, raw) in traces {
            match mapping::map_trace_to_transfer(self.chain, &raw, &block_ts, &mut by_tx) {
                Ok(Some(t)) => out.push(t),
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "alchemy: skip malformed trace"),
            }
        }
        for (_, raw) in logs {
            match mapping::map_log_to_transfer(self.chain, &raw, &block_ts, &mut by_tx) {
                Ok(Some(t)) => out.push(t),
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "alchemy: skip malformed log"),
            }
        }

        // Stable ordering by (block_height, idx) so downstream BFS state is
        // deterministic across runs.
        out.sort_by_key(|t| (t.block().height(), t.id().index()));
        if out.len() > max_transfers {
            out.truncate(max_transfers);
        }

        tracing::info!(
            address = %address_hex,
            total = out.len(),
            "alchemy: transfers fetched"
        );
        Ok(out)
    }

    async fn is_contract(&self, addr: &Address) -> DomainResult<Option<bool>> {
        if addr.chain() != self.chain {
            return Ok(None);
        }
        let bytes = addr.bytes().to_vec();
        if let Some(cached) = self.is_contract_cache.get(&bytes).await {
            return Ok(Some(cached));
        }
        let address_hex = format!("0x{}", hex::encode(&bytes));
        match self.eth_get_code(&address_hex).await {
            Ok(Some(v)) => {
                self.is_contract_cache.insert(bytes, v).await;
                Ok(Some(v))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Real bulk path: one POST with a JSON array of `eth_getCode` calls
    /// keyed by their array index. We skip cached entries entirely (no
    /// token consumed), batch the rest into a single round-trip, then
    /// stitch results back into the input order. With ~100 addresses per
    /// batch this turns 100 × RTT into 1 × RTT.
    ///
    /// Defensive: if a batch returns Ok with zero resolved entries (either
    /// Alchemy doesn't support batches on this plan, the response shape
    /// changed, or the request was silently malformed), we flip a sticky
    /// flag and fall back to parallel single-shot calls. So classification
    /// keeps working even when batching is broken — just at single-call
    /// throughput.
    async fn is_contract_batch(
        &self,
        addrs: &[Address],
    ) -> DomainResult<Vec<Option<bool>>> {
        if addrs.is_empty() {
            return Ok(Vec::new());
        }
        // Cached entries shortcut.
        let mut out: Vec<Option<bool>> = vec![None; addrs.len()];
        let mut needs_fetch_idx: Vec<usize> = Vec::with_capacity(addrs.len());
        let mut needs_fetch_addr: Vec<String> = Vec::with_capacity(addrs.len());
        for (i, addr) in addrs.iter().enumerate() {
            if addr.chain() != self.chain {
                // Stay consistent with the single-shot path: foreign chains
                // get Ok(None) (unknown), not an error.
                continue;
            }
            let bytes = addr.bytes().to_vec();
            if let Some(cached) = self.is_contract_cache.get(&bytes).await {
                out[i] = Some(cached);
                continue;
            }
            needs_fetch_idx.push(i);
            needs_fetch_addr.push(format!("0x{}", hex::encode(&bytes)));
        }
        if needs_fetch_idx.is_empty() {
            return Ok(out);
        }

        // Sticky disable — once we've proven the batch endpoint is busted
        // on this account/plan, skip it for the rest of the process.
        if self.batch_eth_get_code_disabled.load(Ordering::Relaxed) {
            return self.fan_out_individual(addrs, out, &needs_fetch_idx).await;
        }

        let results = self.eth_get_code_batch(&needs_fetch_addr).await?;
        let resolved = results.iter().filter(|r| r.is_some()).count();
        // Sticky fallback only fires when the batch path produced
        // *literally nothing* across ≥4 entries — that's a shape bug, not
        // transient throttling (inner 429s are now retried within
        // eth_get_code_batch and surface here as resolved entries). Be
        // strict so we don't disable batching the first time CU/s spikes.
        if resolved == 0 && results.len() >= 4 {
            self.batch_eth_get_code_disabled.store(true, Ordering::Relaxed);
            tracing::warn!(
                batch_size = results.len(),
                "alchemy batch resolved 0 entries even after inner retries — disabling batched eth_getCode for this process, falling back to single-shot"
            );
            return self.fan_out_individual(addrs, out, &needs_fetch_idx).await;
        }

        for (slot, res) in needs_fetch_idx.iter().zip(results.into_iter()) {
            if let Some(v) = res {
                let bytes = addrs[*slot].bytes().to_vec();
                self.is_contract_cache.insert(bytes, v).await;
                out[*slot] = Some(v);
            }
        }
        Ok(out)
    }
}
