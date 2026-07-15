use std::path::PathBuf;
use std::time::Duration;

const DEFAULT_BASE_URL: &str = "https://api.etherscan.io/v2/api";
const DEFAULT_PAGE_SIZE: u32 = 1000;
const DEFAULT_MAX_PAGES: u32 = 20;
const DEFAULT_HTTP_MAX_ATTEMPTS: u8 = 6;
const DEFAULT_IS_CONTRACT_MAX_ATTEMPTS: u8 = 4;
const DEFAULT_KEY_COOLDOWN_SECS: u64 = 5;
const DEFAULT_REQUESTS_PER_SECOND: f64 = 5.0;
const DEFAULT_REQUESTS_PER_SECOND_BURST: f64 = 5.0;

#[derive(Debug, Clone)]
pub struct EtherscanEvmConfig {
    pub(super) api_keys: Vec<String>,
    pub(super) base_url: String,
    pub(super) page_size: u32,
    pub(super) max_pages: u32,
    pub(super) cold_ttl: Duration,
    pub(super) hot_ttl: Duration,
    pub(super) cache_hot_tail: bool,
    pub(super) confirmation_depth: u64,
    pub(super) latest_block_ttl: Duration,
    pub(super) page_max_capacity: u64,
    pub(super) file_cache_dir: Option<PathBuf>,
    pub(super) max_concurrent_requests: u32,
    pub(super) key_cooldown: Duration,
    pub(super) requests_per_second: f64,
    pub(super) requests_per_second_burst: f64,
    pub(super) http_max_attempts: u8,
    pub(super) is_contract_max_attempts: u8,
}

impl EtherscanEvmConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        api_keys: Vec<String>,
        base_url: Option<String>,
        page_size: Option<u32>,
        max_pages: Option<u32>,
        cold_ttl: Duration,
        hot_ttl: Duration,
        cache_hot_tail: bool,
        confirmation_depth: u64,
        latest_block_ttl: Duration,
        page_max_capacity: u64,
        file_cache_dir: Option<PathBuf>,
        max_concurrent_requests: u32,
        key_cooldown: Option<Duration>,
        requests_per_second: Option<f64>,
        requests_per_second_burst: Option<f64>,
        http_max_attempts: Option<u8>,
        is_contract_max_attempts: Option<u8>,
    ) -> Self {
        let api_keys: Vec<String> = api_keys
            .into_iter()
            .filter(|k| !k.trim().is_empty())
            .collect();
        Self {
            api_keys,
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.into()),
            page_size: page_size.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, 10_000),
            max_pages: max_pages.unwrap_or(DEFAULT_MAX_PAGES).max(1),
            cold_ttl,
            hot_ttl,
            cache_hot_tail,
            confirmation_depth,
            latest_block_ttl,
            page_max_capacity: page_max_capacity.max(1),
            file_cache_dir,
            max_concurrent_requests: max_concurrent_requests.max(1),
            key_cooldown: key_cooldown
                .unwrap_or_else(|| Duration::from_secs(DEFAULT_KEY_COOLDOWN_SECS)),
            requests_per_second: requests_per_second.unwrap_or(DEFAULT_REQUESTS_PER_SECOND),
            requests_per_second_burst: requests_per_second_burst
                .unwrap_or(DEFAULT_REQUESTS_PER_SECOND_BURST),
            http_max_attempts: http_max_attempts.unwrap_or(DEFAULT_HTTP_MAX_ATTEMPTS).max(1),
            is_contract_max_attempts: is_contract_max_attempts
                .unwrap_or(DEFAULT_IS_CONTRACT_MAX_ATTEMPTS)
                .max(1),
        }
    }

    pub fn has_keys(&self) -> bool {
        !self.api_keys.is_empty()
    }

    /// Chain-specific override for the API base URL. Etherscan V2 uses one
    /// canonical URL and a `chainid=` query param, so most deployments never
    /// need this; kept for private/proxied etherscan-compatible endpoints.
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }
}
