use std::time::Duration;

pub const DEFAULT_BASE_URL: &str = "https://eth-mainnet.g.alchemy.com/v2";
const DEFAULT_KEY_COOLDOWN_SECS: u64 = 5;
const DEFAULT_LATEST_BLOCK_TTL_SECS: u64 = 15;
const DEFAULT_LOG_CHUNK_BLOCKS: u64 = 2_000;
const DEFAULT_MIN_LOG_CHUNK_BLOCKS: u64 = 16;
const DEFAULT_MAX_CONCURRENT_REQUESTS: u32 = 8;
const DEFAULT_TRACE_PAGE_SIZE: u32 = 1_000;
const DEFAULT_TRACE_MAX_PAGES: u32 = 50;
const DEFAULT_COLD_TTL_SECS: u64 = 86_400;
const DEFAULT_HOT_TTL_SECS: u64 = 15;
const DEFAULT_CONFIRMATION_DEPTH: u64 = 12;
const DEFAULT_PAGE_MAX_CAPACITY: u64 = 100_000;
const DEFAULT_HTTP_MAX_ATTEMPTS: u8 = 5;
/// Free-tier Alchemy throttles by Compute Units / second (500 CU/s with a
/// 10s burst window of 5000 CU). The rate limiter measures cost in CUs;
/// each call passes its method's CU cost. See `cu_for_method` for the
/// per-method table.
const DEFAULT_REQUESTS_PER_SECOND: f64 = 500.0;
const DEFAULT_REQUESTS_PER_SECOND_BURST: f64 = 5_000.0;

#[derive(Debug, Clone)]
pub struct AlchemyEvmConfig {
    pub(super) api_keys: Vec<String>,
    pub(super) base_url: String,
    pub(super) key_cooldown: Duration,
    pub(super) latest_block_ttl: Duration,
    pub(super) max_concurrent_requests: u32,
    pub(super) enable_transfers: bool,
    pub(super) log_chunk_blocks: u64,
    pub(super) min_log_chunk_blocks: u64,
    pub(super) trace_page_size: u32,
    pub(super) trace_max_pages: u32,
    pub(super) cold_ttl: Duration,
    pub(super) hot_ttl: Duration,
    pub(super) cache_hot_tail: bool,
    pub(super) confirmation_depth: u64,
    pub(super) page_max_capacity: u64,
    pub(super) requests_per_second: f64,
    pub(super) requests_per_second_burst: f64,
    pub(super) http_max_attempts: u8,
}

impl AlchemyEvmConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        api_keys: Vec<String>,
        base_url: Option<String>,
        key_cooldown: Option<Duration>,
        latest_block_ttl: Option<Duration>,
        max_concurrent_requests: Option<u32>,
        enable_transfers: bool,
        log_chunk_blocks: Option<u64>,
        min_log_chunk_blocks: Option<u64>,
        trace_page_size: Option<u32>,
        trace_max_pages: Option<u32>,
        cold_ttl: Option<Duration>,
        hot_ttl: Option<Duration>,
        cache_hot_tail: Option<bool>,
        confirmation_depth: Option<u64>,
        page_max_capacity: Option<u64>,
        requests_per_second: Option<f64>,
        requests_per_second_burst: Option<f64>,
        http_max_attempts: Option<u8>,
    ) -> Self {
        let api_keys: Vec<String> = api_keys
            .into_iter()
            .filter(|k| !k.trim().is_empty())
            .collect();
        Self {
            api_keys,
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.into()),
            key_cooldown: key_cooldown
                .unwrap_or_else(|| Duration::from_secs(DEFAULT_KEY_COOLDOWN_SECS)),
            latest_block_ttl: latest_block_ttl
                .unwrap_or_else(|| Duration::from_secs(DEFAULT_LATEST_BLOCK_TTL_SECS)),
            max_concurrent_requests: max_concurrent_requests
                .unwrap_or(DEFAULT_MAX_CONCURRENT_REQUESTS)
                .max(1),
            enable_transfers,
            log_chunk_blocks: log_chunk_blocks.unwrap_or(DEFAULT_LOG_CHUNK_BLOCKS).max(1),
            min_log_chunk_blocks: min_log_chunk_blocks
                .unwrap_or(DEFAULT_MIN_LOG_CHUNK_BLOCKS)
                .max(1),
            trace_page_size: trace_page_size
                .unwrap_or(DEFAULT_TRACE_PAGE_SIZE)
                .clamp(1, 10_000),
            trace_max_pages: trace_max_pages.unwrap_or(DEFAULT_TRACE_MAX_PAGES).max(1),
            cold_ttl: cold_ttl.unwrap_or_else(|| Duration::from_secs(DEFAULT_COLD_TTL_SECS)),
            hot_ttl: hot_ttl.unwrap_or_else(|| Duration::from_secs(DEFAULT_HOT_TTL_SECS)),
            cache_hot_tail: cache_hot_tail.unwrap_or(true),
            confirmation_depth: confirmation_depth.unwrap_or(DEFAULT_CONFIRMATION_DEPTH),
            page_max_capacity: page_max_capacity.unwrap_or(DEFAULT_PAGE_MAX_CAPACITY).max(1),
            requests_per_second: requests_per_second.unwrap_or(DEFAULT_REQUESTS_PER_SECOND),
            requests_per_second_burst: requests_per_second_burst
                .unwrap_or(DEFAULT_REQUESTS_PER_SECOND_BURST),
            http_max_attempts: http_max_attempts.unwrap_or(DEFAULT_HTTP_MAX_ATTEMPTS).max(1),
        }
    }

    pub fn has_keys(&self) -> bool {
        !self.api_keys.is_empty()
    }

    /// Per-chain Alchemy endpoint override. Alchemy uses chain-specific
    /// hostnames (eth-mainnet / polygon-mainnet / base-mainnet / ...) that
    /// share the same API-key pool. Multi-chain deployments MUST override
    /// this when reusing one `alchemy:` section across chains.
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }
}
