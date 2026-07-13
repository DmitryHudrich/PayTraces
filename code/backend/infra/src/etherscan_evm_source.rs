use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use moka::future::Cache;
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

pub use config::EtherscanEvmConfig;

const LATEST_SENTINEL: u64 = 99_999_999;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Endpoint {
    TxList,
    TxListInternal,
    TokenTx,
}

impl Endpoint {
    fn action(self) -> &'static str {
        match self {
            Endpoint::TxList => "txlist",
            Endpoint::TxListInternal => "txlistinternal",
            Endpoint::TokenTx => "tokentx",
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Endpoint::TxList => "nat",
            Endpoint::TxListInternal => "int",
            Endpoint::TokenTx => "erc20",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PageKey {
    endpoint: Endpoint,
    address: String,
    from_block: u64,
    to_block: u64,
    page: u32,
}

type PageValue = Arc<Vec<serde_json::Value>>;

#[derive(Clone)]
pub struct EtherscanEvmSource {
    chain: ChainId,
    key_pool: KeyPool,
    base_url: String,
    page_size: u32,
    max_pages: u32,
    cache_hot_tail: bool,
    confirmation_depth: u64,
    http_max_attempts: u8,
    is_contract_max_attempts: u8,
    client: reqwest::Client,
    cold_page_cache: Cache<PageKey, PageValue>,
    hot_page_cache: Cache<PageKey, PageValue>,
    latest_block_cache: Cache<(), u64>,
    is_contract_cache: Cache<Vec<u8>, bool>,
    file_cache_dir: Option<PathBuf>,
    preloaded_file_cache: Arc<HashMap<PathBuf, PageValue>>,
    request_permits: Arc<Semaphore>,
    rate_limiter: Arc<RateLimiter>,
}

impl EtherscanEvmSource {
    pub async fn new(chain: ChainId, client: reqwest::Client, cfg: EtherscanEvmConfig) -> Self {
        assert!(
            cfg.has_keys(),
            "EtherscanEvmSource: at least one api key required — config validation must guard this"
        );
        let key_pool = KeyPool::new(cfg.api_keys.clone(), cfg.key_cooldown);
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

        let latest_block_cache = Cache::builder()
            .max_capacity(1)
            .time_to_live(cfg.latest_block_ttl)
            .build();

        // Contract code is immutable in practice (post-Cancun SELFDESTRUCT
        // basically removed); cache aggressively to avoid burning API quota.
        let is_contract_cache = Cache::builder()
            .max_capacity(100_000)
            .time_to_live(Duration::from_secs(7 * 24 * 3600))
            .build();

        let preloaded_file_cache = match &cfg.file_cache_dir {
            Some(dir) => Arc::new(Self::load_file_cache(dir).await),
            None => Arc::new(HashMap::new()),
        };

        let request_permits = Arc::new(Semaphore::new(cfg.max_concurrent_requests as usize));
        let rate_limiter = Arc::new(RateLimiter::new(
            cfg.requests_per_second,
            cfg.requests_per_second_burst,
        ));

        tracing::info!(
            base_url = %cfg.base_url,
            page_size = cfg.page_size,
            max_pages = cfg.max_pages,
            cold_ttl_secs = cfg.cold_ttl.as_secs(),
            hot_ttl_secs = cfg.hot_ttl.as_secs(),
            cache_hot_tail = cfg.cache_hot_tail,
            confirmation_depth = cfg.confirmation_depth,
            file_cache_dir = ?cfg.file_cache_dir,
            preloaded_pages = preloaded_file_cache.len(),
            max_concurrent_requests = cfg.max_concurrent_requests,
            api_keys = key_pool.len(),
            key_cooldown_secs = cfg.key_cooldown.as_secs(),
            requests_per_second = cfg.requests_per_second,
            requests_per_second_burst = cfg.requests_per_second_burst,
            http_max_attempts = cfg.http_max_attempts,
            is_contract_max_attempts = cfg.is_contract_max_attempts,
            chain_id = chain.value(),
            "Etherscan EVM source initialized"
        );

        Self {
            chain,
            key_pool,
            base_url: cfg.base_url,
            page_size: cfg.page_size,
            max_pages: cfg.max_pages,
            cache_hot_tail: cfg.cache_hot_tail,
            confirmation_depth: cfg.confirmation_depth,
            http_max_attempts: cfg.http_max_attempts,
            is_contract_max_attempts: cfg.is_contract_max_attempts,
            client,
            cold_page_cache,
            hot_page_cache,
            latest_block_cache,
            is_contract_cache,
            file_cache_dir: cfg.file_cache_dir,
            preloaded_file_cache,
            request_permits,
            rate_limiter,
        }
    }
}

#[async_trait]
impl ChainSource for EtherscanEvmSource {
    fn chain_id(&self) -> ChainId {
        self.chain
    }

    async fn latest_block(&self) -> DomainResult<BlockRef> {
        match self.latest_block_height().await {
            Some(h) => Ok(BlockRef::new(self.chain, h, [0u8; 32])),
            None => Err(DomainError::InsufficientData(
                "etherscan: eth_blockNumber failed".into(),
            )),
        }
    }

    async fn fetch_block(&self, height: u64) -> DomainResult<NormalizedBlock> {
        Err(DomainError::InsufficientData(format!(
            "etherscan: fetch_block by height ({height}) not supported; use transfers_for_address"
        )))
    }

    async fn transfers_for_address(
        &self,
        addr: &Address,
        range: BlockRange,
        max_transfers: usize,
    ) -> DomainResult<Vec<Transfer>> {
        if addr.chain() != self.chain {
            return Err(DomainError::InsufficientData(format!(
                "etherscan source (chain={}) called with foreign address chain: {}",
                self.chain,
                addr.chain()
            )));
        }
        let address_hex = format!("0x{}", hex::encode(addr.bytes()));
        let from_block = range.from_height();
        // etherscan accepts a sentinel for "latest"; modern blocks
        // are ~22M, so any large value works as long as it's a valid u32-ish.
        let to_block = if range.to_height() == u64::MAX {
            LATEST_SENTINEL
        } else {
            range.to_height()
        };

        tracing::info!(
            address = %address_hex,
            from_block,
            to_block,
            max_transfers,
            "etherscan: fetching ETH transfers"
        );

        // Fan-out all three account endpoints concurrently — they share the
        // semaphore, so this respects the upstream rate cap while cutting
        // wall-clock latency to the slowest endpoint instead of their sum.
        let (native_raw, internal_raw, token_raw) = tokio::try_join!(
            self.collect(Endpoint::TxList, &address_hex, from_block, to_block, max_transfers),
            self.collect(
                Endpoint::TxListInternal,
                &address_hex,
                from_block,
                to_block,
                max_transfers
            ),
            self.collect(Endpoint::TokenTx, &address_hex, from_block, to_block, max_transfers),
        )?;

        self.harvest_contract_signals(&native_raw, &internal_raw, &token_raw)
            .await;

        let mut native = Vec::with_capacity(native_raw.len());
        for raw in native_raw {
            match mapping::map_native(self.chain, &raw) {
                Ok(Some(t)) => native.push(t),
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "etherscan: skip malformed native row"),
            }
        }

        // Shared idx counter across internal + token rows so they never
        // collide on (chain, tx_hash, idx) — native always claims idx=0.
        let mut by_tx: HashMap<[u8; 32], u32> = HashMap::new();

        let mut internal = Vec::with_capacity(internal_raw.len());
        for raw in internal_raw {
            match mapping::map_internal(self.chain, &raw, &mut by_tx) {
                Ok(Some(t)) => internal.push(t),
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "etherscan: skip malformed internal row"),
            }
        }

        let mut token = Vec::with_capacity(token_raw.len());
        for raw in token_raw {
            match mapping::map_token(self.chain, &raw, &mut by_tx) {
                Ok(Some(t)) => token.push(t),
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "etherscan: skip malformed token row"),
            }
        }

        tracing::info!(
            address = %address_hex,
            native = native.len(),
            internal = internal.len(),
            erc20 = token.len(),
            total = native.len() + internal.len() + token.len(),
            "etherscan: transfers fetched"
        );

        let mut out = native;
        out.extend(internal);
        out.extend(token);
        if out.len() > max_transfers {
            out.truncate(max_transfers);
        }
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
        match self.is_contract_with_retry(&address_hex).await {
            Ok(Some(is_c)) => {
                self.is_contract_cache.insert(bytes, is_c).await;
                Ok(Some(is_c))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Etherscan has no JSON-RPC batch, so each cache miss is its own
    /// `eth_getCode`. But the in-memory harvest cache typically covers
    /// 60-80% of any traced address graph (every contract that was the
    /// `from` of an internal trace or the `to` of a calldata-bearing call,
    /// plus every ERC-20 contract that emitted a Transfer). For those
    /// addresses the batch resolves with zero network traffic — only the
    /// stragglers fan out as parallel single-address calls.
    async fn is_contract_batch(
        &self,
        addrs: &[Address],
    ) -> DomainResult<Vec<Option<bool>>> {
        if addrs.is_empty() {
            return Ok(Vec::new());
        }
        let mut out: Vec<Option<bool>> = vec![None; addrs.len()];
        let mut to_fetch: Vec<(usize, &Address)> = Vec::new();
        for (i, addr) in addrs.iter().enumerate() {
            if addr.chain() != self.chain {
                continue;
            }
            let bytes = addr.bytes().to_vec();
            if let Some(cached) = self.is_contract_cache.get(&bytes).await {
                out[i] = Some(cached);
            } else {
                to_fetch.push((i, addr));
            }
        }
        if to_fetch.is_empty() {
            return Ok(out);
        }

        use futures::future::join_all;
        let futs = to_fetch.iter().map(|(_, addr)| async move {
            let bytes = addr.bytes().to_vec();
            let hex = format!("0x{}", hex::encode(&bytes));
            (self.is_contract_with_retry(&hex).await, bytes)
        });
        let results = join_all(futs).await;
        for ((i, _), (res, bytes)) in to_fetch.iter().zip(results.into_iter()) {
            match res {
                Ok(Some(v)) => {
                    self.is_contract_cache.insert(bytes, v).await;
                    out[*i] = Some(v);
                }
                Ok(None) => {}
                // Single rate-limited address shouldn't sink the whole
                // batch — record as soft-unknown and keep going.
                Err(_) => {}
            }
        }
        Ok(out)
    }
}
