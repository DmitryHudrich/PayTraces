use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use chrono::TimeZone;
use moka::future::Cache;
use serde::Deserialize;
use tokio::sync::Semaphore;

use domain::{
    asset::{AssetId, TokenStandard},
    chain::ChainId,
    error::{DomainError, DomainResult},
    ports::{BlockRange, ChainSource},
    primitives::{Address, Amount, BlockRef, TxRef, U256},
    transfer::{Finality, NormalizedBlock, Transfer, TransferId, TransferKind},
};

const DEFAULT_BASE_URL: &str = "https://api.etherscan.io/v2/api";
const DEFAULT_PAGE_SIZE: u32 = 1000;
const DEFAULT_MAX_PAGES: u32 = 20;
const ETH_CHAIN_ID: u32 = 1;
const HTTP_MAX_ATTEMPTS: u8 = 6;
const LATEST_SENTINEL: u64 = 99_999_999;
const RATE_LIMIT_BACKOFF_BASE_MS: u64 = 500;

#[derive(Debug, Clone)]
pub struct EtherscanEthConfig {
    api_key: String,
    base_url: String,
    page_size: u32,
    max_pages: u32,
    cold_ttl: Duration,
    hot_ttl: Duration,
    cache_hot_tail: bool,
    confirmation_depth: u64,
    latest_block_ttl: Duration,
    page_max_capacity: u64,
    file_cache_dir: Option<PathBuf>,
    max_concurrent_requests: u32,
}

impl EtherscanEthConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        api_key: String,
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
    ) -> Self {
        Self {
            api_key,
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
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Endpoint {
    TxList,
    TokenTx,
}

impl Endpoint {
    fn action(self) -> &'static str {
        match self {
            Endpoint::TxList => "txlist",
            Endpoint::TokenTx => "tokentx",
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Endpoint::TxList => "nat",
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
pub struct EtherscanEthSource {
    api_key: String,
    base_url: String,
    page_size: u32,
    max_pages: u32,
    cache_hot_tail: bool,
    confirmation_depth: u64,
    client: reqwest::Client,
    cold_page_cache: Cache<PageKey, PageValue>,
    hot_page_cache: Cache<PageKey, PageValue>,
    latest_block_cache: Cache<(), u64>,
    file_cache_dir: Option<PathBuf>,
    preloaded_file_cache: Arc<HashMap<PathBuf, PageValue>>,
    request_permits: Arc<Semaphore>,
}

impl EtherscanEthSource {
    pub async fn new(client: reqwest::Client, cfg: EtherscanEthConfig) -> Self {
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

        let preloaded_file_cache = match &cfg.file_cache_dir {
            Some(dir) => Arc::new(Self::load_file_cache(dir).await),
            None => Arc::new(HashMap::new()),
        };

        let request_permits = Arc::new(Semaphore::new(cfg.max_concurrent_requests as usize));

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
            "Etherscan ETH source initialized"
        );

        Self {
            api_key: cfg.api_key,
            base_url: cfg.base_url,
            page_size: cfg.page_size,
            max_pages: cfg.max_pages,
            cache_hot_tail: cfg.cache_hot_tail,
            confirmation_depth: cfg.confirmation_depth,
            client,
            cold_page_cache,
            hot_page_cache,
            latest_block_cache,
            file_cache_dir: cfg.file_cache_dir,
            preloaded_file_cache,
            request_permits,
        }
    }

    async fn load_file_cache(dir: &Path) -> HashMap<PathBuf, PageValue> {
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

    fn build_url(&self, endpoint: Endpoint, params: &[(&str, String)]) -> String {
        let mut url = format!(
            "{base}?chainid={chain}&module=account&action={action}&apikey={key}",
            base = self.base_url,
            chain = ETH_CHAIN_ID,
            action = endpoint.action(),
            key = self.api_key,
        );
        for (k, v) in params {
            url.push('&');
            url.push_str(k);
            url.push('=');
            url.push_str(v);
        }
        url
    }

    async fn http_get_json(&self, url: &str) -> DomainResult<EtherscanResponse> {
        let mut last_err = String::new();
        for attempt in 0..HTTP_MAX_ATTEMPTS {
            if attempt > 0 {
                // Exponential backoff: 500ms, 1s, 2s, 4s, 8s, capped by HTTP_MAX_ATTEMPTS.
                let backoff = Duration::from_millis(
                    RATE_LIMIT_BACKOFF_BASE_MS.saturating_mul(1u64 << (attempt - 1)),
                );
                tokio::time::sleep(backoff).await;
            }
            tracing::debug!(url, attempt, "etherscan GET");

            // Hold the semaphore permit only across the network call so the
            // backoff sleep above does not consume an in-flight slot.
            let permit = match self.request_permits.clone().acquire_owned().await {
                Ok(p) => p,
                Err(e) => {
                    return Err(DomainError::InsufficientData(format!(
                        "etherscan: semaphore closed: {e}"
                    )));
                }
            };

            let resp = match self.client.get(url).send().await {
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
                tracing::warn!(url, attempt, "etherscan HTTP 429, backing off");
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

            // The free tier (and even the keyed tier under burst) returns HTTP 200
            // with `status:"0"` + a string `result` of "Max calls per sec rate
            // limit reached (N/sec)" when throttled. Treat that as a transient
            // error and retry with exponential backoff instead of bubbling up.
            if let Some(msg) = parsed.rate_limit_message() {
                tracing::warn!(url, attempt, message = %msg, "etherscan rate-limited, backing off");
                last_err = msg.to_string();
                continue;
            }

            return Ok(parsed);
        }
        Err(DomainError::InsufficientData(format!(
            "etherscan: after {HTTP_MAX_ATTEMPTS} attempts: {last_err}"
        )))
    }

    async fn latest_block_height(&self) -> Option<u64> {
        if let Some(h) = self.latest_block_cache.get(&()).await {
            return Some(h);
        }
        let url = format!(
            "{base}?chainid={chain}&module=proxy&action=eth_blockNumber&apikey={key}",
            base = self.base_url,
            chain = ETH_CHAIN_ID,
            key = self.api_key,
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
        let url = self.build_url(endpoint, &params);
        let resp = self.http_get_json(&url).await?;

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

    async fn collect(
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
}

#[async_trait]
impl ChainSource for EtherscanEthSource {
    fn chain_id(&self) -> ChainId {
        ChainId::ETH
    }

    async fn latest_block(&self) -> DomainResult<BlockRef> {
        match self.latest_block_height().await {
            Some(h) => Ok(BlockRef::new(ChainId::ETH, h, [0u8; 32])),
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
        if addr.chain() != ChainId::ETH {
            return Err(DomainError::InsufficientData(format!(
                "etherscan source called with non-eth chain: {}",
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

        let (native_raw, token_raw) = tokio::try_join!(
            self.collect(Endpoint::TxList, &address_hex, from_block, to_block, max_transfers),
            self.collect(Endpoint::TokenTx, &address_hex, from_block, to_block, max_transfers),
        )?;

        let mut native = Vec::with_capacity(native_raw.len());
        for raw in native_raw {
            match map_native(&raw) {
                Ok(Some(t)) => native.push(t),
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "etherscan: skip malformed native row"),
            }
        }

        let mut token = Vec::with_capacity(token_raw.len());
        let mut by_tx: HashMap<[u8; 32], u32> = HashMap::new();
        for raw in token_raw {
            match map_token(&raw, &mut by_tx) {
                Ok(Some(t)) => token.push(t),
                Ok(None) => {}
                Err(e) => tracing::warn!(error = %e, "etherscan: skip malformed token row"),
            }
        }

        tracing::info!(
            address = %address_hex,
            native = native.len(),
            erc20 = token.len(),
            total = native.len() + token.len(),
            "etherscan: transfers fetched"
        );

        let mut out = native;
        out.extend(token);
        if out.len() > max_transfers {
            out.truncate(max_transfers);
        }
        Ok(out)
    }
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

fn map_native(raw: &serde_json::Value) -> anyhow::Result<Option<Transfer>> {
    use anyhow::{Context, anyhow};

    if raw.get("isError").and_then(|v| v.as_str()) == Some("1") {
        return Ok(None);
    }
    let value_s = raw
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("native: missing value"))?;
    let raw_val = U256::from_dec_str(value_s).context("native: value")?;
    if raw_val.is_zero() {
        return Ok(None);
    }

    let to_s = raw
        .get("to")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let Some(to_s) = to_s else {
        return Ok(None);
    };
    let from_s = raw
        .get("from")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("native: missing from"))?;
    let tx_hash_s = raw
        .get("hash")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("native: missing hash"))?;
    let block_num_s = raw
        .get("blockNumber")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("native: missing blockNumber"))?;
    let ts_s = raw
        .get("timeStamp")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("native: missing timeStamp"))?;
    let block_hash_s = raw.get("blockHash").and_then(|v| v.as_str());

    let tx_hash = parse_hash32(tx_hash_s).context("native: tx hash")?;
    let block_hash = block_hash_s
        .map(parse_hash32)
        .transpose()
        .context("native: block_hash")?
        .unwrap_or(tx_hash);
    let block_number: u64 = block_num_s.parse().context("native: blockNumber")?;
    let ts_secs: i64 = ts_s.parse().context("native: timeStamp")?;
    let timestamp = chrono::Utc
        .timestamp_opt(ts_secs, 0)
        .single()
        .ok_or_else(|| anyhow!("native: bad timestamp {ts_secs}"))?;

    let from = parse_eth_address(from_s).context("native: from")?;
    let to = parse_eth_address(to_s).context("native: to")?;

    let finality = match raw.get("txreceipt_status").and_then(|v| v.as_str()) {
        Some("0") => Finality::Reorged,
        _ => Finality::Confirmed,
    };

    Ok(Some(Transfer::new(
        TransferId::new(ChainId::ETH, tx_hash, 0),
        ChainId::ETH,
        TxRef::new(ChainId::ETH, tx_hash),
        from,
        to,
        AssetId::native(ChainId::ETH),
        Amount::new(raw_val, 18),
        BlockRef::new(ChainId::ETH, block_number, block_hash),
        timestamp,
        TransferKind::Native,
        finality,
    )))
}

fn map_token(
    raw: &serde_json::Value,
    by_tx: &mut HashMap<[u8; 32], u32>,
) -> anyhow::Result<Option<Transfer>> {
    use anyhow::{Context, anyhow};

    let from_s = raw
        .get("from")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("token: missing from"))?;
    let to_s = raw
        .get("to")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let Some(to_s) = to_s else {
        return Ok(None);
    };
    let contract_s = raw
        .get("contractAddress")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("token: missing contractAddress"))?;
    let value_s = raw
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("token: missing value"))?;
    let tx_hash_s = raw
        .get("hash")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("token: missing hash"))?;
    let block_num_s = raw
        .get("blockNumber")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("token: missing blockNumber"))?;
    let ts_s = raw
        .get("timeStamp")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("token: missing timeStamp"))?;
    let block_hash_s = raw.get("blockHash").and_then(|v| v.as_str());
    let decimals_s = raw.get("tokenDecimal").and_then(|v| v.as_str());
    let symbol = raw
        .get("tokenSymbol")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let tx_hash = parse_hash32(tx_hash_s).context("token: tx hash")?;
    let block_hash = block_hash_s
        .map(parse_hash32)
        .transpose()
        .context("token: block_hash")?
        .unwrap_or(tx_hash);
    let block_number: u64 = block_num_s.parse().context("token: blockNumber")?;
    let ts_secs: i64 = ts_s.parse().context("token: timeStamp")?;
    let timestamp = chrono::Utc
        .timestamp_opt(ts_secs, 0)
        .single()
        .ok_or_else(|| anyhow!("token: bad timestamp {ts_secs}"))?;
    let decimals: u8 = decimals_s
        .map(|s| s.parse::<u8>())
        .transpose()
        .context("token: tokenDecimal")?
        .unwrap_or(18);
    let raw_val = U256::from_dec_str(value_s).context("token: value")?;

    let from = parse_eth_address(from_s).context("token: from")?;
    let to = parse_eth_address(to_s).context("token: to")?;
    let contract = parse_eth_address(contract_s).context("token: contractAddress")?;

    // idx=0 is reserved for the (single) native transfer in this tx; shift
    // token rows by +1 so they never collide on the (chain, tx_hash, idx) PK
    // when a tx has both a native value transfer and an ERC-20 Transfer event.
    let position = by_tx.entry(tx_hash).or_insert(0);
    let idx = position.saturating_add(1);
    *position += 1;

    Ok(Some(Transfer::new(
        TransferId::new(ChainId::ETH, tx_hash, idx),
        ChainId::ETH,
        TxRef::new(ChainId::ETH, tx_hash),
        from,
        to,
        AssetId::contract(ChainId::ETH, contract.bytes().to_vec()),
        Amount::new(raw_val, decimals),
        BlockRef::new(ChainId::ETH, block_number, block_hash),
        timestamp,
        TransferKind::Token {
            contract,
            standard: TokenStandard::Erc20,
            symbol,
        },
        Finality::Confirmed,
    )))
}

fn parse_hash32(s: &str) -> anyhow::Result<[u8; 32]> {
    use anyhow::{Context, anyhow};
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).context("hex decode")?;
    bytes
        .try_into()
        .map_err(|v: Vec<u8>| anyhow!("expected 32 bytes, got {}", v.len()))
}

fn parse_eth_address(s: &str) -> anyhow::Result<Address> {
    use anyhow::Context;
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).context("hex decode eth address")?;
    if bytes.len() != 20 {
        anyhow::bail!("eth address expected 20 bytes, got {}", bytes.len());
    }
    Ok(Address::new(ChainId::ETH, bytes))
}
