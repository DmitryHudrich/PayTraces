use clap::Parser;
use serde::Deserialize;
use std::time::Duration;

#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Cli {
    #[arg(short, long, default_value = "config.yaml")]
    config: String,
    #[arg(long)]
    port: Option<u16>,
    #[arg(long)]
    host: Option<String>,
    #[arg(long)]
    moralis_api_key: Option<String>,
    #[arg(long)]
    trongrid_api_key: Option<String>,
    #[arg(long)]
    etherscan_api_key: Option<String>,
    #[arg(long)]
    socks_proxy: Option<String>,
    #[arg(long)]
    database_url: Option<String>,
    #[arg(long, help = "ETH chain source: moralis | bigquery | etherscan")]
    eth_source: Option<String>,
    #[arg(long)]
    bigquery_project_id: Option<String>,
    #[arg(long)]
    bigquery_credentials_path: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct LogConfig {
    directives: Vec<String>,
}

impl LogConfig {
    pub fn directives(&self) -> &[String] {
        &self.directives
    }

    pub fn build_filter(&self) -> tracing_subscriber::EnvFilter {
        let directives = self.directives.join(",");
        tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&directives))
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ServerConfig {
    host: String,
    port: u16,
    #[serde(default, rename = "api_key")]
    api_key: Option<String>,
}

impl ServerConfig {
    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref().filter(|s| !s.is_empty())
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct MoralisConfig {
    base_url: String,
    api_key: Option<String>,
    #[serde(default)]
    cache: CacheConfigFile,
}

impl MoralisConfig {
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    pub fn cache(&self) -> &CacheConfigFile {
        &self.cache
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct TronGridConfigFile {
    #[serde(default = "TronGridConfigFile::default_base_url")]
    base_url: String,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default = "TronGridConfigFile::default_page_ttl_secs")]
    page_ttl_secs: u64,
    #[serde(default = "TronGridConfigFile::default_page_max_capacity")]
    page_max_capacity: u64,
    #[serde(default = "TronGridConfigFile::default_max_pages")]
    max_pages_per_endpoint: u32,
    #[serde(default)]
    file_cache: FileCacheConfig,
    #[serde(default = "TronGridConfigFile::default_enabled")]
    enabled: bool,
}

impl Default for TronGridConfigFile {
    fn default() -> Self {
        Self {
            base_url: Self::default_base_url(),
            api_key: None,
            page_ttl_secs: Self::default_page_ttl_secs(),
            page_max_capacity: Self::default_page_max_capacity(),
            max_pages_per_endpoint: Self::default_max_pages(),
            file_cache: FileCacheConfig::default(),
            enabled: Self::default_enabled(),
        }
    }
}

impl TronGridConfigFile {
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    fn default_base_url() -> String {
        "https://api.trongrid.io".into()
    }
    fn default_page_ttl_secs() -> u64 {
        3_600
    }
    fn default_page_max_capacity() -> u64 {
        10_000
    }
    fn default_max_pages() -> u32 {
        50
    }
    fn default_enabled() -> bool {
        true
    }

    pub fn into_domain(self) -> infra::TronGridConfig {
        infra::TronGridConfig::new(
            self.base_url,
            self.api_key,
            self.page_max_capacity,
            Duration::from_secs(self.page_ttl_secs),
            if self.file_cache.enabled {
                Some(std::path::PathBuf::from(self.file_cache.dir))
            } else {
                None
            },
            self.max_pages_per_endpoint,
        )
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct FileCacheConfig {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "FileCacheConfig::default_dir")]
    dir: String,
}

impl Default for FileCacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            dir: Self::default_dir(),
        }
    }
}

impl FileCacheConfig {
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn dir(&self) -> &str {
        &self.dir
    }

    fn default_dir() -> String {
        "./moralis_cache".into()
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct CacheConfigFile {
    #[serde(
        default = "CacheConfigFile::default_cold_ttl_secs",
        alias = "page_ttl_secs"
    )]
    cold_ttl_secs: u64,
    #[serde(default = "CacheConfigFile::default_hot_ttl_secs")]
    hot_ttl_secs: u64,
    #[serde(default = "CacheConfigFile::default_cache_hot_tail")]
    cache_hot_tail: bool,
    #[serde(default = "CacheConfigFile::default_confirmation_depth")]
    confirmation_depth: u64,
    #[serde(default = "CacheConfigFile::default_page_max_capacity")]
    page_max_capacity: u64,
    #[serde(default = "CacheConfigFile::default_latest_block_ttl_secs")]
    latest_block_ttl_secs: u64,
    #[serde(default)]
    file_cache: FileCacheConfig,
}

impl Default for CacheConfigFile {
    fn default() -> Self {
        Self {
            cold_ttl_secs: Self::default_cold_ttl_secs(),
            hot_ttl_secs: Self::default_hot_ttl_secs(),
            cache_hot_tail: Self::default_cache_hot_tail(),
            confirmation_depth: Self::default_confirmation_depth(),
            page_max_capacity: Self::default_page_max_capacity(),
            latest_block_ttl_secs: Self::default_latest_block_ttl_secs(),
            file_cache: FileCacheConfig::default(),
        }
    }
}

impl CacheConfigFile {
    fn default_cold_ttl_secs() -> u64 {
        86_400
    }
    fn default_hot_ttl_secs() -> u64 {
        15
    }
    fn default_cache_hot_tail() -> bool {
        true
    }
    fn default_confirmation_depth() -> u64 {
        12
    }
    fn default_page_max_capacity() -> u64 {
        100_000
    }
    fn default_latest_block_ttl_secs() -> u64 {
        15
    }

    pub fn into_domain(self) -> infra::fetch_wallet_api::CacheConfig {
        infra::fetch_wallet_api::CacheConfig::new(
            self.page_max_capacity,
            Duration::from_secs(self.cold_ttl_secs),
            Duration::from_secs(self.hot_ttl_secs),
            self.cache_hot_tail,
            self.confirmation_depth,
            Duration::from_secs(self.latest_block_ttl_secs),
            if self.file_cache.enabled {
                Some(std::path::PathBuf::from(self.file_cache.dir))
            } else {
                None
            },
        )
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct RiskCacheConfigFile {
    #[serde(default = "RiskCacheConfigFile::default_score_ttl_secs")]
    score_ttl_secs: u64,
    #[serde(default = "RiskCacheConfigFile::default_max_entries")]
    score_max_entries: u64,
    #[serde(default = "RiskCacheConfigFile::default_sanctions_ttl_secs")]
    sanctions_ttl_secs: u64,
    #[serde(default = "RiskCacheConfigFile::default_max_entries")]
    sanctions_max_entries: u64,
}

impl Default for RiskCacheConfigFile {
    fn default() -> Self {
        Self {
            score_ttl_secs: Self::default_score_ttl_secs(),
            score_max_entries: Self::default_max_entries(),
            sanctions_ttl_secs: Self::default_sanctions_ttl_secs(),
            sanctions_max_entries: Self::default_max_entries(),
        }
    }
}

impl RiskCacheConfigFile {
    fn default_score_ttl_secs() -> u64 {
        300
    }
    fn default_sanctions_ttl_secs() -> u64 {
        900
    }
    fn default_max_entries() -> u64 {
        10_000
    }

    pub fn into_domain(self) -> usecase::risk::RiskCacheConfig {
        usecase::risk::RiskCacheConfig {
            score_ttl: Duration::from_secs(self.score_ttl_secs),
            score_max_entries: self.score_max_entries,
            sanctions_ttl: Duration::from_secs(self.sanctions_ttl_secs),
            sanctions_max_entries: self.sanctions_max_entries,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct HeuristicsConfigFile {
    #[serde(default = "HeuristicsConfigFile::default_min_fanout")]
    min_fanout: usize,
    #[serde(default = "HeuristicsConfigFile::default_min_fanin")]
    min_fanin: usize,
    #[serde(default = "HeuristicsConfigFile::default_fan_window_secs")]
    fan_window_secs: u64,
    #[serde(default = "HeuristicsConfigFile::default_smurf_window_secs")]
    smurf_window_secs: u64,
    #[serde(default = "HeuristicsConfigFile::default_smurf_max_depth")]
    smurf_max_depth: u32,
}

impl Default for HeuristicsConfigFile {
    fn default() -> Self {
        Self {
            min_fanout: Self::default_min_fanout(),
            min_fanin: Self::default_min_fanin(),
            fan_window_secs: Self::default_fan_window_secs(),
            smurf_window_secs: Self::default_smurf_window_secs(),
            smurf_max_depth: Self::default_smurf_max_depth(),
        }
    }
}

impl HeuristicsConfigFile {
    fn default_min_fanout() -> usize {
        5
    }
    fn default_min_fanin() -> usize {
        5
    }
    fn default_fan_window_secs() -> u64 {
        86_400
    }
    fn default_smurf_window_secs() -> u64 {
        86_400
    }
    fn default_smurf_max_depth() -> u32 {
        2
    }

    pub fn into_domain(self) -> usecase::risk::HeuristicsConfig {
        usecase::risk::HeuristicsConfig {
            min_fanout: self.min_fanout,
            min_fanin: self.min_fanin,
            fan_window: Duration::from_secs(self.fan_window_secs),
            smurf_window: Duration::from_secs(self.smurf_window_secs),
            smurf_max_depth: self.smurf_max_depth,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct DatabaseConfig {
    url: String,
}

impl DatabaseConfig {
    pub fn url(&self) -> &str {
        &self.url
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ProxyConfig {
    socks_url: Option<String>,
}

impl ProxyConfig {
    pub fn socks_url(&self) -> Option<&str> {
        self.socks_url.as_deref()
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct TelemetryConfig {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "TelemetryConfig::default_endpoint")]
    otlp_endpoint: String,
    #[serde(default = "TelemetryConfig::default_service_name")]
    service_name: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            otlp_endpoint: Self::default_endpoint(),
            service_name: Self::default_service_name(),
        }
    }
}

impl TelemetryConfig {
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn otlp_endpoint(&self) -> &str {
        &self.otlp_endpoint
    }

    pub fn service_name(&self) -> &str {
        &self.service_name
    }

    fn default_endpoint() -> String {
        "http://localhost:4317".into()
    }
    fn default_service_name() -> String {
        "paytraces-api".into()
    }
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EthSourceKind {
    Moralis,
    Bigquery,
    Etherscan,
}

impl Default for EthSourceKind {
    fn default() -> Self {
        Self::Moralis
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct EthereumConfig {
    #[serde(default)]
    source: EthSourceKind,
}

impl EthereumConfig {
    pub fn source(&self) -> EthSourceKind {
        self.source
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct BigQueryConfig {
    project_id: String,
    credentials_path: String,
    #[serde(default = "BigQueryConfig::default_transactions_table")]
    transactions_table: String,
    #[serde(default)]
    token_transfers_table: Option<String>,
    #[serde(default = "BigQueryConfig::default_max_rows")]
    max_rows_per_query: u64,
    #[serde(default = "BigQueryConfig::default_query_timeout_secs")]
    query_timeout_secs: u64,
}

impl BigQueryConfig {
    fn default_transactions_table() -> String {
        "bigquery-public-data.goog_blockchain_ethereum_mainnet_us.transactions".into()
    }
    fn default_max_rows() -> u64 {
        10_000
    }
    fn default_query_timeout_secs() -> u64 {
        60
    }

    pub fn project_id(&self) -> &str {
        &self.project_id
    }

    pub fn credentials_path(&self) -> &str {
        &self.credentials_path
    }

    pub fn into_domain(self) -> infra::BigQueryEthConfig {
        infra::BigQueryEthConfig::new(
            self.project_id,
            std::path::PathBuf::from(self.credentials_path),
            self.transactions_table,
            self.token_transfers_table,
            self.max_rows_per_query,
            Duration::from_secs(self.query_timeout_secs),
        )
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct EtherscanConfigFile {
    api_key: Option<String>,
    #[serde(default = "EtherscanConfigFile::default_base_url")]
    base_url: String,
    #[serde(default)]
    page_size: Option<u32>,
    #[serde(default)]
    max_pages: Option<u32>,
    #[serde(default = "EtherscanConfigFile::default_cold_ttl_secs")]
    cold_ttl_secs: u64,
    #[serde(default = "EtherscanConfigFile::default_hot_ttl_secs")]
    hot_ttl_secs: u64,
    #[serde(default = "EtherscanConfigFile::default_cache_hot_tail")]
    cache_hot_tail: bool,
    #[serde(default = "EtherscanConfigFile::default_confirmation_depth")]
    confirmation_depth: u64,
    #[serde(default = "EtherscanConfigFile::default_latest_block_ttl_secs")]
    latest_block_ttl_secs: u64,
    #[serde(default = "EtherscanConfigFile::default_page_max_capacity")]
    page_max_capacity: u64,
    #[serde(default = "EtherscanConfigFile::default_max_concurrent_requests")]
    max_concurrent_requests: u32,
    #[serde(default)]
    file_cache: EtherscanFileCacheConfig,
}

impl EtherscanConfigFile {
    fn default_base_url() -> String {
        "https://api.etherscan.io/v2/api".into()
    }
    fn default_cold_ttl_secs() -> u64 {
        86_400
    }
    fn default_hot_ttl_secs() -> u64 {
        15
    }
    fn default_cache_hot_tail() -> bool {
        true
    }
    fn default_confirmation_depth() -> u64 {
        12
    }
    fn default_latest_block_ttl_secs() -> u64 {
        15
    }
    fn default_page_max_capacity() -> u64 {
        10_000
    }
    fn default_max_concurrent_requests() -> u32 {
        // Etherscan free tier caps at 5 req/sec; leave headroom so bursts of
        // tokio::try_join! native+token calls across BFS nodes do not trip the
        // throttle. The HTTP layer also retries on rate-limit responses.
        4
    }

    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref().filter(|s| !s.is_empty())
    }

    pub fn into_domain(self) -> infra::EtherscanEthConfig {
        infra::EtherscanEthConfig::new(
            self.api_key.unwrap_or_default(),
            Some(self.base_url),
            self.page_size,
            self.max_pages,
            Duration::from_secs(self.cold_ttl_secs),
            Duration::from_secs(self.hot_ttl_secs),
            self.cache_hot_tail,
            self.confirmation_depth,
            Duration::from_secs(self.latest_block_ttl_secs),
            self.page_max_capacity,
            if self.file_cache.enabled {
                Some(std::path::PathBuf::from(self.file_cache.dir))
            } else {
                None
            },
            self.max_concurrent_requests,
        )
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct EtherscanFileCacheConfig {
    #[serde(default)]
    enabled: bool,
    #[serde(default = "EtherscanFileCacheConfig::default_dir")]
    dir: String,
}

impl Default for EtherscanFileCacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            dir: Self::default_dir(),
        }
    }
}

impl EtherscanFileCacheConfig {
    fn default_dir() -> String {
        "./etherscan_cache".into()
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    server: ServerConfig,
    moralis: MoralisConfig,
    #[serde(default)]
    trongrid: TronGridConfigFile,
    #[serde(default)]
    bigquery: Option<BigQueryConfig>,
    #[serde(default)]
    etherscan: Option<EtherscanConfigFile>,
    #[serde(default)]
    ethereum: EthereumConfig,
    database: DatabaseConfig,
    proxy: ProxyConfig,
    log: LogConfig,
    #[serde(default)]
    telemetry: TelemetryConfig,
    #[serde(default)]
    risk_cache: RiskCacheConfigFile,
    #[serde(default)]
    heuristics: HeuristicsConfigFile,
}

impl AppConfig {
    pub fn server(&self) -> &ServerConfig {
        &self.server
    }

    pub fn moralis(&self) -> &MoralisConfig {
        &self.moralis
    }

    pub fn trongrid(&self) -> &TronGridConfigFile {
        &self.trongrid
    }

    pub fn database(&self) -> &DatabaseConfig {
        &self.database
    }

    pub fn proxy(&self) -> &ProxyConfig {
        &self.proxy
    }

    pub fn log(&self) -> &LogConfig {
        &self.log
    }

    pub fn telemetry(&self) -> &TelemetryConfig {
        &self.telemetry
    }

    pub fn risk_cache(&self) -> &RiskCacheConfigFile {
        &self.risk_cache
    }

    pub fn heuristics(&self) -> &HeuristicsConfigFile {
        &self.heuristics
    }

    pub fn ethereum(&self) -> &EthereumConfig {
        &self.ethereum
    }

    pub fn bigquery(&self) -> Option<&BigQueryConfig> {
        self.bigquery.as_ref()
    }

    pub fn etherscan(&self) -> Option<&EtherscanConfigFile> {
        self.etherscan.as_ref()
    }

    pub fn load(cli: &Cli) -> anyhow::Result<Self> {
        let mut builder = config::Config::builder()
            .add_source(config::File::with_name(&cli.config).required(false))
            .add_source(
                config::Environment::default()
                    .separator("__")
                    .try_parsing(true),
            );

        let overrides: &[(&str, &dyn Fn(&Cli) -> Option<String>)] = &[
            ("server.port", &|c| c.port.map(|p| p.to_string())),
            ("server.host", &|c| c.host.clone()),
            ("moralis.api_key", &|c| c.moralis_api_key.clone()),
            ("trongrid.api_key", &|c| c.trongrid_api_key.clone()),
            ("etherscan.api_key", &|c| c.etherscan_api_key.clone()),
            ("proxy.socks_url", &|c| c.socks_proxy.clone()),
            ("database.url", &|c| c.database_url.clone()),
            ("ethereum.source", &|c| c.eth_source.clone()),
            ("bigquery.project_id", &|c| c.bigquery_project_id.clone()),
            ("bigquery.credentials_path", &|c| {
                c.bigquery_credentials_path.clone()
            }),
        ];

        for (key, extract) in overrides {
            if let Some(value) = extract(cli) {
                builder = builder.set_override(key, value)?;
            }
        }

        Ok(builder.build()?.try_deserialize()?)
    }
}
