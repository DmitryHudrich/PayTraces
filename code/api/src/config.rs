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
    pub directives: Vec<String>,
}

impl LogConfig {
    pub fn build_filter(&self) -> tracing_subscriber::EnvFilter {
        self.directives.iter().fold(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
            |f, d| f.add_directive(d.parse().unwrap()),
        )
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MoralisConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    #[serde(default)]
    pub cache: CacheConfigFile,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TronConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    #[serde(default)]
    pub cache: CacheConfigFile,
}

#[derive(Deserialize, Debug, Clone)]
pub struct FileCacheConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "FileCacheConfig::default_dir")]
    pub dir: String,
}

impl Default for FileCacheConfig {
    fn default() -> Self {
        Self { enabled: false, dir: Self::default_dir() }
    }
}

impl FileCacheConfig {
    fn default_dir() -> String {
        "./moralis_cache".into()
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct CacheConfigFile {
    #[serde(default = "CacheConfigFile::default_page_ttl_secs")]
    pub page_ttl_secs: u64,
    #[serde(default = "CacheConfigFile::default_page_max_capacity")]
    pub page_max_capacity: u64,
    #[serde(default = "CacheConfigFile::default_latest_block_ttl_secs")]
    pub latest_block_ttl_secs: u64,
    #[serde(default = "CacheConfigFile::default_block_cache_max_capacity")]
    pub block_cache_max_capacity: u64,
    #[serde(default = "CacheConfigFile::default_block_cache_ttl_secs")]
    pub block_cache_ttl_secs: u64,
    #[serde(default)]
    pub file_cache: FileCacheConfig,
}

impl Default for CacheConfigFile {
    fn default() -> Self {
        Self {
            page_ttl_secs: Self::default_page_ttl_secs(),
            page_max_capacity: Self::default_page_max_capacity(),
            latest_block_ttl_secs: Self::default_latest_block_ttl_secs(),
            block_cache_max_capacity: Self::default_block_cache_max_capacity(),
            block_cache_ttl_secs: Self::default_block_cache_ttl_secs(),
            file_cache: FileCacheConfig::default(),
        }
    }
}

impl CacheConfigFile {
    fn default_page_ttl_secs() -> u64 {
        86_400
    }
    fn default_page_max_capacity() -> u64 {
        10_000
    }
    fn default_latest_block_ttl_secs() -> u64 {
        15
    }
    fn default_block_cache_max_capacity() -> u64 {
        5_000
    }
    fn default_block_cache_ttl_secs() -> u64 {
        86_400 * 7
    }

    pub fn into_domain(self) -> infra::fetch_wallet_api::CacheConfig {
        infra::fetch_wallet_api::CacheConfig {
            page_cache_max_capacity: self.page_max_capacity,
            page_cache_ttl: Duration::from_secs(self.page_ttl_secs),
            latest_block_cache_ttl: Duration::from_secs(self.latest_block_ttl_secs),
            file_cache_dir: if self.file_cache.enabled {
                Some(std::path::PathBuf::from(self.file_cache.dir))
            } else {
                None
            },
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ProxyConfig {
    pub socks_url: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub moralis: MoralisConfig,
    pub tron: TronConfig,
    pub database: DatabaseConfig,
    pub proxy: ProxyConfig,
    pub log: LogConfig,
}

impl AppConfig {
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
            ("tron.api_key", &|c| c.trongrid_api_key.clone()),
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

