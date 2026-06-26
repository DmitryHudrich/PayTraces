use anyhow::{Context, anyhow};
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use async_trait::async_trait;
use domain::{
    asset::{AssetId, TokenStandard},
    chain::ChainId,
    error::{DomainError, DomainResult},
    ports::{BlockRange, ChainSource},
    primitives::{Address, Amount, BlockRef, TxRef, U256},
    transfer::{Finality, NormalizedBlock, Transfer, TransferId, TransferKind},
};
use moka::future::Cache;

pub mod side_api {
    pub mod moralis {
        pub mod dto;
        pub mod endpoints;
    }
    pub mod tron {
        pub mod dto;
        pub mod endpoints;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Endpoint {
    NativeTransactions,
    Erc20Transfers,
}

impl Endpoint {
    fn prefix(&self) -> &'static str {
        match self {
            Endpoint::NativeTransactions => "nat",
            Endpoint::Erc20Transfers => "erc20",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PageKey {
    endpoint: Endpoint,
    address_hex: String,
    cursor: Option<String>,
    from_block: Option<u64>,
    to_block: Option<u64>,
}

type PageValue = Arc<(Vec<Transfer>, Option<String>)>;

#[derive(Debug, Clone)]
pub struct CacheConfig {
    page_cache_max_capacity: u64,
    cold_ttl: Duration,
    hot_ttl: Duration,
    cache_hot_tail: bool,
    confirmation_depth: u64,
    latest_block_cache_ttl: Duration,
    file_cache_dir: Option<PathBuf>,
}

impl CacheConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        page_cache_max_capacity: u64,
        cold_ttl: Duration,
        hot_ttl: Duration,
        cache_hot_tail: bool,
        confirmation_depth: u64,
        latest_block_cache_ttl: Duration,
        file_cache_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            page_cache_max_capacity,
            cold_ttl,
            hot_ttl,
            cache_hot_tail,
            confirmation_depth,
            latest_block_cache_ttl,
            file_cache_dir,
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            page_cache_max_capacity: 10_000,
            cold_ttl: Duration::from_secs(60 * 60 * 24),
            hot_ttl: Duration::from_secs(15),
            cache_hot_tail: true,
            confirmation_depth: 12,
            latest_block_cache_ttl: Duration::from_secs(15),
            file_cache_dir: None,
        }
    }
}

#[derive(Clone)]
pub struct MoralisEthSource {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
    cold_page_cache: Cache<PageKey, PageValue>,
    hot_page_cache: Cache<PageKey, PageValue>,
    cache_hot_tail: bool,
    confirmation_depth: u64,
    latest_block_cache: Cache<(), u64>,
    file_cache_dir: Option<PathBuf>,
    hot_cache: Arc<HashMap<PathBuf, PageValue>>,
}

impl MoralisEthSource {
    pub async fn new(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        client: reqwest::Client,
        cache: CacheConfig,
    ) -> Self {
        let cold_page_cache = Cache::builder()
            .max_capacity(cache.page_cache_max_capacity)
            .weigher(|_k: &PageKey, v: &PageValue| v.0.len().max(1) as u32)
            .time_to_live(cache.cold_ttl)
            .build();

        let hot_page_cache = Cache::builder()
            .max_capacity(cache.page_cache_max_capacity)
            .weigher(|_k: &PageKey, v: &PageValue| v.0.len().max(1) as u32)
            .time_to_live(cache.hot_ttl)
            .build();

        let latest_block_cache = Cache::builder()
            .max_capacity(1)
            .time_to_live(cache.latest_block_cache_ttl)
            .build();

        let hot_cache = match &cache.file_cache_dir {
            Some(dir) => Arc::new(Self::load_hot_cache(dir).await),
            None => Arc::new(HashMap::new()),
        };

        Self {
            api_key: api_key.into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            client,
            cold_page_cache,
            hot_page_cache,
            cache_hot_tail: cache.cache_hot_tail,
            confirmation_depth: cache.confirmation_depth,
            latest_block_cache,
            file_cache_dir: cache.file_cache_dir,
            hot_cache,
        }
    }

    async fn latest_block_height(&self) -> Option<u64> {
        if let Some(h) = self.latest_block_cache.get(&()).await {
            return Some(h);
        }
        let url = format!(
            "{}/dateToBlock?chain=eth&date={}",
            self.base_url,
            chrono::Utc::now().timestamp()
        );
        let body = match self.http_get_text(&url).await {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(url, error = %e, "moralis dateToBlock HTTP failed");
                return None;
            }
        };
        let resp = match serde_json::from_str::<side_api::moralis::dto::DateToBlockResponse>(&body)
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    url,
                    error = %e,
                    body_preview = %body.chars().take(200).collect::<String>(),
                    "moralis dateToBlock body parse failed"
                );
                return None;
            }
        };
        let h = resp.block();
        self.latest_block_cache.insert((), h).await;
        Some(h)
    }

    async fn classify_page(&self, transfers: &[Transfer], requested_to: Option<u64>) -> bool {
        let Some(latest) = self.latest_block_height().await else {
            return true;
        };
        let cutoff = latest.saturating_sub(self.confirmation_depth);
        if transfers.is_empty() {
            match requested_to {
                None => true,
                Some(to) => to > cutoff,
            }
        } else {
            transfers.iter().any(|t| t.block().height() > cutoff)
        }
    }

    async fn insert_page(&self, key: PageKey, value: PageValue, is_hot: bool) {
        if is_hot {
            if self.cache_hot_tail {
                self.hot_page_cache.insert(key, value).await;
            }
        } else {
            self.cold_page_cache.insert(key, value).await;
        }
    }

    async fn lookup_page(&self, key: &PageKey) -> Option<PageValue> {
        if let Some(v) = self.cold_page_cache.get(key).await {
            return Some(v);
        }
        self.hot_page_cache.get(key).await
    }

    async fn load_hot_cache(dir: &PathBuf) -> HashMap<PathBuf, PageValue> {
        let mut map = HashMap::new();
        let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
            return map;
        };
        let mut files: usize = 0;
        let mut transfers: usize = 0;

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();

            let parsed = if name.starts_with("nat__") {
                Self::file_read(&path)
                    .await
                    .and_then(|body| parse_native_response(&body).ok())
            } else if name.starts_with("erc20__") {
                Self::file_read(&path)
                    .await
                    .and_then(|body| parse_erc20_response(&body).ok())
            } else {
                None
            };

            if let Some(value) = parsed {
                transfers += value.0.len();
                map.insert(path, Arc::new(value));
                files += 1;
            }
        }

        tracing::info!(files, transfers, "moralis file cache pre-loaded into memory");
        map
    }

    fn get_req(&self, url: &str) -> reqwest::RequestBuilder {
        self.client.get(url).header("X-API-Key", &self.api_key)
    }

    async fn http_get_text(&self, url: &str) -> DomainResult<String> {
        const MAX_ATTEMPTS: u8 = 3;
        let mut last_err = String::new();

        for attempt in 0..MAX_ATTEMPTS {
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(u64::from(attempt) * 2)).await;
            }

            tracing::debug!(url, attempt, "moralis GET");

            let resp = match self.get_req(url).send().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(url, attempt, error = %e, "request failed, retrying");
                    last_err = e.to_string();
                    continue;
                }
            };

            let status = resp.status();

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                tracing::warn!(url, "moralis rate limited");
                return Err(DomainError::InsufficientData("rate limited".into()));
            }

            let bytes = match resp.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(url, attempt, error = %e, "failed to read body, retrying");
                    last_err = e.to_string();
                    continue;
                }
            };

            let body = String::from_utf8_lossy(&bytes).into_owned();

            if !status.is_success() {
                return Err(DomainError::InsufficientData(format!(
                    "http {status}: {body}"
                )));
            }

            if body.trim_end().ends_with('}') || body.trim_end().ends_with(']') {
                tracing::debug!(
                    url,
                    bytes = bytes.len(),
                    status = status.as_u16(),
                    "response ok"
                );
                return Ok(body);
            }

            last_err = format!("truncated response ({} bytes)", bytes.len());
            tracing::warn!(
                url,
                attempt,
                bytes = bytes.len(),
                "truncated response, retrying"
            );
        }

        Err(DomainError::InsufficientData(format!(
            "after {MAX_ATTEMPTS} attempts: {last_err}"
        )))
    }

    fn file_path(
        &self,
        endpoint: &Endpoint,
        address: &str,
        cursor: Option<&str>,
        from_block: Option<u64>,
        to_block: Option<u64>,
    ) -> Option<PathBuf> {
        let dir = self.file_cache_dir.as_ref()?;
        let addr = address.strip_prefix("0x").unwrap_or(address);
        let from = from_block
            .map(|b| b.to_string())
            .unwrap_or_else(|| "nil".into());
        let to = to_block
            .map(|b| b.to_string())
            .unwrap_or_else(|| "nil".into());

        let cur = match cursor {
            None => "nil".to_string(),
            Some(c) => {
                use sha2::{Digest, Sha256};
                hex::encode(Sha256::digest(c.as_bytes()))
            }
        };
        Some(dir.join(format!(
            "{}__{addr}__{from}__{to}__{cur}.json",
            endpoint.prefix()
        )))
    }

    async fn file_read(path: &std::path::Path) -> Option<String> {
        tokio::fs::read_to_string(path).await.ok()
    }

    async fn file_write(path: &std::path::Path, body: &str) {
        if let Some(parent) = path.parent()
            && let Err(e) = tokio::fs::create_dir_all(parent).await
        {
            tracing::warn!(dir = %parent.display(), error = %e, "failed to create cache dir");
            return;
        }
        match tokio::fs::write(path, body).await {
            Ok(_) => {
                tracing::debug!(path = %path.display(), bytes = body.len(), "cache file written")
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to write cache file")
            }
        }
    }

    async fn fetch_native_page(
        &self,
        address_hex: &str,
        cursor: Option<&str>,
        from_block: Option<u64>,
        to_block: Option<u64>,
    ) -> DomainResult<PageValue> {
        let key = PageKey {
            endpoint: Endpoint::NativeTransactions,
            address_hex: address_hex.to_string(),
            cursor: cursor.map(str::to_string),
            from_block,
            to_block,
        };

        if let Some(v) = self.lookup_page(&key).await {
            tracing::debug!(address = address_hex, endpoint = "native", "moka cache hit");
            return Ok(v);
        }

        let file_path = self.file_path(
            &Endpoint::NativeTransactions,
            address_hex,
            cursor,
            from_block,
            to_block,
        );

        if let Some(ref path) = file_path
            && let Some(v) = self.hot_cache.get(path)
        {
            let is_hot = self.classify_page(&v.0, to_block).await;
            if !is_hot {
                tracing::debug!(address = address_hex, "hot cache hit (native, cold)");
                self.insert_page(key, Arc::clone(v), false).await;
                return Ok(Arc::clone(v));
            }
            tracing::warn!(
                address = address_hex,
                path = %path.display(),
                "preloaded page classifies as hot; dropping stale disk cache"
            );
            let _ = tokio::fs::remove_file(path).await;
        }

        let url = self.build_native_url(address_hex, cursor, from_block, to_block);
        let (mut body, mut from_disk) =
            self.read_or_fetch(file_path.as_deref(), &url, address_hex).await?;
        let mut value = Arc::new(parse_native_response(&body)?);
        let mut is_hot = self.classify_page(&value.0, to_block).await;

        if from_disk && is_hot {
            tracing::warn!(
                address = address_hex,
                "disk cache hit on a hot-classified page; dropping and refetching"
            );
            if let Some(path) = file_path.as_deref() {
                let _ = tokio::fs::remove_file(path).await;
            }
            body = self.http_get_text(&url).await?;
            value = Arc::new(parse_native_response(&body)?);
            is_hot = self.classify_page(&value.0, to_block).await;
            from_disk = false;
        }

        tracing::debug!(
            address = address_hex,
            endpoint = "native",
            transfers = value.0.len(),
            has_next = value.1.is_some(),
            is_hot,
            "page parsed"
        );

        if !is_hot && !from_disk
            && let Some(path) = file_path.as_deref()
        {
            Self::file_write(path, &body).await;
        }

        self.insert_page(key, Arc::clone(&value), is_hot).await;
        Ok(value)
    }

    async fn fetch_erc20_page(
        &self,
        address_hex: &str,
        cursor: Option<&str>,
        from_block: Option<u64>,
        to_block: Option<u64>,
    ) -> DomainResult<PageValue> {
        let key = PageKey {
            endpoint: Endpoint::Erc20Transfers,
            address_hex: address_hex.to_string(),
            cursor: cursor.map(str::to_string),
            from_block,
            to_block,
        };

        if let Some(v) = self.lookup_page(&key).await {
            tracing::debug!(address = address_hex, endpoint = "erc20", "moka cache hit");
            return Ok(v);
        }

        let file_path = self.file_path(
            &Endpoint::Erc20Transfers,
            address_hex,
            cursor,
            from_block,
            to_block,
        );

        if let Some(ref path) = file_path
            && let Some(v) = self.hot_cache.get(path)
        {
            let is_hot = self.classify_page(&v.0, to_block).await;
            if !is_hot {
                tracing::debug!(address = address_hex, "hot cache hit (erc20, cold)");
                self.insert_page(key, Arc::clone(v), false).await;
                return Ok(Arc::clone(v));
            }
            tracing::warn!(
                address = address_hex,
                path = %path.display(),
                "preloaded erc20 page classifies as hot; dropping stale disk cache"
            );
            let _ = tokio::fs::remove_file(path).await;
        }

        let url = self.build_erc20_url(address_hex, cursor, from_block, to_block);
        let (mut body, mut from_disk) =
            self.read_or_fetch(file_path.as_deref(), &url, address_hex).await?;
        let mut value = Arc::new(parse_erc20_response(&body)?);
        let mut is_hot = self.classify_page(&value.0, to_block).await;

        if from_disk && is_hot {
            tracing::warn!(
                address = address_hex,
                "disk cache hit on a hot-classified erc20 page; dropping and refetching"
            );
            if let Some(path) = file_path.as_deref() {
                let _ = tokio::fs::remove_file(path).await;
            }
            body = self.http_get_text(&url).await?;
            value = Arc::new(parse_erc20_response(&body)?);
            is_hot = self.classify_page(&value.0, to_block).await;
            from_disk = false;
        }

        tracing::debug!(
            address = address_hex,
            endpoint = "erc20",
            transfers = value.0.len(),
            has_next = value.1.is_some(),
            is_hot,
            "page parsed"
        );

        if !is_hot && !from_disk
            && let Some(path) = file_path.as_deref()
        {
            Self::file_write(path, &body).await;
        }

        self.insert_page(key, Arc::clone(&value), is_hot).await;
        Ok(value)
    }

    async fn read_or_fetch(
        &self,
        file_path: Option<&std::path::Path>,
        url: &str,
        address_hex: &str,
    ) -> DomainResult<(String, bool)> {
        if let Some(path) = file_path
            && let Some(cached) = Self::file_read(path).await
        {
            tracing::debug!(address = address_hex, path = %path.display(), "file cache hit (disk)");
            return Ok((cached, true));
        }
        let body = self.http_get_text(url).await?;
        Ok((body, false))
    }

    fn build_native_url(
        &self,
        address: &str,
        cursor: Option<&str>,
        from_block: Option<u64>,
        to_block: Option<u64>,
    ) -> String {
        let mut url = format!("{}/{}?chain=eth", self.base_url, address);
        if let Some(c) = cursor {
            url.push_str("&cursor=");
            url.push_str(c);
        }
        url.push_str(&block_range_params(from_block, to_block));
        url
    }

    fn build_erc20_url(
        &self,
        address: &str,
        cursor: Option<&str>,
        from_block: Option<u64>,
        to_block: Option<u64>,
    ) -> String {
        let mut url = format!("{}/{}/erc20/transfers?chain=eth", self.base_url, address);
        if let Some(c) = cursor {
            url.push_str("&cursor=");
            url.push_str(c);
        }
        url.push_str(&block_range_params(from_block, to_block));
        url
    }

    async fn collect_native(
        &self,
        address_hex: &str,
        from_block: Option<u64>,
        to_block: Option<u64>,
        max_transfers: usize,
    ) -> DomainResult<Vec<Transfer>> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        let mut page_n: u32 = 0;
        loop {
            let page = self
                .fetch_native_page(address_hex, cursor.as_deref(), from_block, to_block)
                .await?;
            page_n += 1;
            let done = early_stop(&page.0, from_block);
            all.extend(page.0.iter().cloned());
            tracing::debug!(
                address = address_hex,
                page = page_n,
                page_transfers = page.0.len(),
                total = all.len(),
                endpoint = "native",
                "paginating"
            );
            if done || all.len() >= max_transfers {
                break;
            }
            match page.1.clone() {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        tracing::debug!(
            address = address_hex,
            total = all.len(),
            pages = page_n,
            endpoint = "native",
            "pagination done"
        );
        Ok(all)
    }

    async fn collect_erc20(
        &self,
        address_hex: &str,
        from_block: Option<u64>,
        to_block: Option<u64>,
        max_transfers: usize,
    ) -> DomainResult<Vec<Transfer>> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        let mut page_n: u32 = 0;
        loop {
            let page = self
                .fetch_erc20_page(address_hex, cursor.as_deref(), from_block, to_block)
                .await?;
            page_n += 1;
            let done = early_stop(&page.0, from_block);
            all.extend(page.0.iter().cloned());
            tracing::debug!(
                address = address_hex,
                page = page_n,
                page_transfers = page.0.len(),
                total = all.len(),
                endpoint = "erc20",
                "paginating"
            );
            if done || all.len() >= max_transfers {
                break;
            }
            match page.1.clone() {
                Some(c) => cursor = Some(c),
                None => break,
            }
        }
        tracing::debug!(
            address = address_hex,
            total = all.len(),
            pages = page_n,
            endpoint = "erc20",
            "pagination done"
        );
        Ok(all)
    }
}

fn early_stop(transfers: &[Transfer], from_block: Option<u64>) -> bool {
    from_block.is_some_and(|from| {
        !transfers.is_empty() && transfers.iter().all(|t| t.block().height() < from)
    })
}

#[async_trait]
impl ChainSource for MoralisEthSource {
    fn chain_id(&self) -> ChainId {
        ChainId::ETH
    }

    async fn latest_block(&self) -> DomainResult<BlockRef> {
        let h = self
            .latest_block_height()
            .await
            .ok_or_else(|| DomainError::InsufficientData("moralis latest_block fetch failed".into()))?;
        Ok(BlockRef::new(ChainId::ETH, h, [0u8; 32]))
    }

    async fn fetch_block(&self, height: u64) -> DomainResult<NormalizedBlock> {
        Err(DomainError::InsufficientData(format!(
            "fetch_block by height ({height}) not supported; use transfers_for_address"
        )))
    }

    async fn transfers_for_address(
        &self,
        addr: &Address,
        range: BlockRange,
        max_transfers: usize,
    ) -> DomainResult<Vec<Transfer>> {
        let address_hex = format!("0x{}", hex::encode(addr.bytes()));
        let from_block = if range.from_height() > 0 {
            Some(range.from_height())
        } else {
            None
        };
        let to_block = if range.to_height() < u64::MAX {
            Some(range.to_height())
        } else {
            None
        };

        tracing::info!(
            address = %address_hex,
            from_block,
            to_block,
            max_transfers,
            "fetching transfers from moralis"
        );

        let (native, erc20) = tokio::try_join!(
            self.collect_native(&address_hex, from_block, to_block, max_transfers),
            self.collect_erc20(&address_hex, from_block, to_block, max_transfers),
        )?;

        let total = native.len() + erc20.len();
        tracing::info!(
            address = %address_hex,
            native = native.len(),
            erc20 = erc20.len(),
            total,
            "transfers fetched"
        );

        let mut all = native;
        all.extend(erc20);
        Ok(all)
    }
}

fn parse_native_response(body: &str) -> DomainResult<(Vec<Transfer>, Option<String>)> {
    let resp = serde_json::from_str::<side_api::moralis::dto::WalletTransactionsResponse>(body)
        .map_err(|e| DomainError::InsufficientData(format!("parse native: {e}\n{body}")))?;

    let next_cursor = resp.cursor().map(str::to_string);
    let mut transfers = Vec::new();
    for tx in resp.into_result() {
        let mapped = map_native_transaction(tx)
            .map_err(|e| DomainError::InsufficientData(format!("map native tx: {e}")))?;
        transfers.extend(mapped);
    }
    Ok((transfers, next_cursor))
}

fn parse_erc20_response(body: &str) -> DomainResult<(Vec<Transfer>, Option<String>)> {
    let resp = serde_json::from_str::<side_api::moralis::dto::Erc20TransfersResponse>(body)
        .map_err(|e| DomainError::InsufficientData(format!("parse erc20: {e}\n{body}")))?;

    let next_cursor = resp.cursor().map(str::to_string);
    let mut transfers = Vec::new();
    for rec in resp.into_result() {
        if let Some(t) = map_erc20_record(rec)
            .map_err(|e| DomainError::InsufficientData(format!("map erc20 rec: {e}")))?
        {
            transfers.push(t);
        }
    }
    Ok((transfers, next_cursor))
}

fn block_range_params(from_block: Option<u64>, to_block: Option<u64>) -> String {
    let mut s = String::new();
    if let Some(from) = from_block {
        s.push_str(&format!("&from_block={from}"));
    }
    if let Some(to) = to_block {
        s.push_str(&format!("&to_block={to}"));
    }
    s
}

fn parse_hash32(s: &str) -> anyhow::Result<[u8; 32]> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).context("hex decode")?;
    bytes
        .try_into()
        .map_err(|_| anyhow!("expected 32 bytes, got {}", s.len() / 2))
}

fn parse_address(s: &str) -> anyhow::Result<Address> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    Ok(Address::new(
        ChainId::ETH,
        hex::decode(s).context("hex decode address")?,
    ))
}

fn parse_u256(s: &str) -> anyhow::Result<U256> {
    U256::from_dec_str(s).map_err(|e| anyhow!("U256 parse '{s}': {e}"))
}

fn map_native_transaction(
    tx: side_api::moralis::dto::WalletTransaction,
) -> anyhow::Result<Vec<Transfer>> {
    let tx_hash_bytes = parse_hash32(tx.hash()).context("tx hash")?;
    let block_number: u64 = tx.block_number().parse().context("block_number")?;

    let timestamp = chrono::DateTime::parse_from_rfc3339(tx.block_timestamp())
        .context("block_timestamp")?
        .with_timezone(&chrono::Utc);

    let block_hash = tx
        .block_hash()
        .map(parse_hash32)
        .transpose()
        .context("block_hash")?
        .unwrap_or(tx_hash_bytes);

    let block_ref = BlockRef::new(ChainId::ETH, block_number, block_hash);
    let tx_ref = TxRef::new(ChainId::ETH, tx_hash_bytes);

    let finality = match tx.receipt_status() {
        Some("1") => Finality::Confirmed,
        Some(_) => Finality::Reorged,
        None => Finality::Unconfirmed,
    };

    let to_str = match tx.to_address() {
        Some(s) if !s.is_empty() => s,
        _ => return Ok(vec![]),
    };

    let raw = parse_u256(tx.value()).context("native value")?;
    if raw.is_zero() {
        return Ok(vec![]);
    }

    let from = parse_address(tx.from_address()).context("native.from")?;
    let to = parse_address(to_str).context("native.to")?;

    Ok(vec![Transfer::new(
        TransferId::new(ChainId::ETH, tx_hash_bytes, 0),
        ChainId::ETH,
        tx_ref,
        from,
        to,
        AssetId::native(ChainId::ETH),
        Amount::new(raw, 18),
        block_ref,
        timestamp,
        TransferKind::Native,
        finality,
    )])
}

fn map_erc20_record(
    rec: side_api::moralis::dto::Erc20TransferRecord,
) -> anyhow::Result<Option<Transfer>> {
    let tx_hash_bytes = parse_hash32(rec.transaction_hash()).context("tx hash")?;
    let block_number: u64 = rec.block_number().parse().context("block_number")?;

    let timestamp = chrono::DateTime::parse_from_rfc3339(rec.block_timestamp())
        .context("block_timestamp")?
        .with_timezone(&chrono::Utc);

    let block_hash = rec
        .block_hash()
        .map(parse_hash32)
        .transpose()
        .context("block_hash")?
        .unwrap_or(tx_hash_bytes);

    let block_ref = BlockRef::new(ChainId::ETH, block_number, block_hash);
    let tx_ref = TxRef::new(ChainId::ETH, tx_hash_bytes);

    let raw = parse_u256(rec.value()).context("erc20 value")?;
    let decimals: u8 = rec
        .token_decimals()
        .unwrap_or("18")
        .parse()
        .context("erc20 decimals")?;

    let from = parse_address(rec.from_address()).context("erc20.from")?;
    let to = parse_address(rec.to_address()).context("erc20.to")?;
    let contract = parse_address(rec.address()).context("erc20.contract")?;
    let log_index = rec.log_index().unwrap_or(0) as u32;
    let symbol = rec
        .token_symbol()
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    Ok(Some(Transfer::new(
        TransferId::new(ChainId::ETH, tx_hash_bytes, log_index),
        ChainId::ETH,
        tx_ref,
        from,
        to,
        AssetId::contract(ChainId::ETH, contract.bytes().to_vec()),
        Amount::new(raw, decimals),
        block_ref,
        timestamp,
        TransferKind::Token {
            contract,
            standard: TokenStandard::Erc20,
            symbol,
        },
        Finality::Confirmed,
    )))
}
