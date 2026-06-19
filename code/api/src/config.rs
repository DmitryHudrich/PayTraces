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
    socks_proxy: Option<String>,
    #[arg(long)]
    database_url: Option<String>,
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
}

impl ServerConfig {
    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
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
    #[serde(default = "CacheConfigFile::default_page_ttl_secs")]
    page_ttl_secs: u64,
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
            page_ttl_secs: Self::default_page_ttl_secs(),
            page_max_capacity: Self::default_page_max_capacity(),
            latest_block_ttl_secs: Self::default_latest_block_ttl_secs(),
            file_cache: FileCacheConfig::default(),
        }
    }
}

impl CacheConfigFile {
    fn default_page_ttl_secs() -> u64 {
        86_400
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
            Duration::from_secs(self.page_ttl_secs),
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

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    server: ServerConfig,
    moralis: MoralisConfig,
    #[serde(default)]
    trongrid: TronGridConfigFile,
    database: DatabaseConfig,
    proxy: ProxyConfig,
    log: LogConfig,
    #[serde(default)]
    telemetry: TelemetryConfig,
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
            ("proxy.socks_url", &|c| c.socks_proxy.clone()),
            ("database.url", &|c| c.database_url.clone()),
        ];

        for (key, extract) in overrides {
            if let Some(value) = extract(cli) {
                builder = builder.set_override(key, value)?;
            }
        }

        Ok(builder.build()?.try_deserialize()?)
    }
}
