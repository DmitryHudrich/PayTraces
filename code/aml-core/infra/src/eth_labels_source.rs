//! `TagProvider` backed by the public, unauthenticated eth-labels.com
//! dataset (https://github.com/dawsbot/eth-labels). `GET /labels/{address}`
//! returns every `{address, chainId, label, nameTag}` row known for that
//! address across all EVM chains it covers — `label` is a free-form
//! taxonomy slug (896+ distinct values observed: `exchange`, `mixer`,
//! `ofac-sanctioned`, `scam`, ...), so one address can yield several rows /
//! several tag candidates. We filter to the caller's `chain_id` and map
//! each row to an `ExternalTagCandidate` via a small keyword table —
//! anything not recognized still comes through as `KnownService` at low
//! confidence so nothing is silently dropped.

use std::time::Duration;

use async_trait::async_trait;
use moka::future::Cache;
use serde::Deserialize;

use domain::error::{DomainError, DomainResult};
use domain::label_tag::{TagCategory, TagSource};
use domain::ports::{ExternalTagCandidate, TagProvider};
use domain::primitives::{Address, Confidence};

use crate::rate_limiter::RateLimiter;

const DEFAULT_BASE_URL: &str = "https://eth-labels.com";
const DEFAULT_REQUESTS_PER_SECOND: f64 = 5.0;
const DEFAULT_CACHE_TTL_SECS: u64 = 86_400;
const DEFAULT_CACHE_MAX_CAPACITY: u64 = 50_000;

#[derive(Debug, Clone)]
pub struct EthLabelsConfig {
    pub base_url: String,
    pub requests_per_second: f64,
    pub cache_ttl_secs: u64,
    pub cache_max_capacity: u64,
}

impl Default for EthLabelsConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            requests_per_second: DEFAULT_REQUESTS_PER_SECOND,
            cache_ttl_secs: DEFAULT_CACHE_TTL_SECS,
            cache_max_capacity: DEFAULT_CACHE_MAX_CAPACITY,
        }
    }
}

impl EthLabelsConfig {
    pub fn new(
        base_url: Option<String>,
        requests_per_second: Option<f64>,
        cache_ttl_secs: Option<u64>,
        cache_max_capacity: Option<u64>,
    ) -> Self {
        Self {
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
            requests_per_second: requests_per_second.unwrap_or(DEFAULT_REQUESTS_PER_SECOND),
            cache_ttl_secs: cache_ttl_secs.unwrap_or(DEFAULT_CACHE_TTL_SECS),
            cache_max_capacity: cache_max_capacity.unwrap_or(DEFAULT_CACHE_MAX_CAPACITY),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
struct LabelRow {
    #[serde(rename = "chainId")]
    chain_id: u32,
    label: String,
    #[serde(rename = "nameTag")]
    name_tag: Option<String>,
}

pub struct EthLabelsSource {
    http: reqwest::Client,
    config: EthLabelsConfig,
    limiter: RateLimiter,
    // Cached per (chain_id, lowercase-hex address) — the upstream call
    // itself is cross-chain, but callers ask per-chain, and different
    // chains for the same address hex are legitimately different cache
    // entries (label sets differ per chain).
    cache: Cache<(u32, String), Vec<ExternalTagCandidate>>,
}

impl EthLabelsSource {
    pub fn new(http: reqwest::Client, config: EthLabelsConfig) -> Self {
        let limiter = RateLimiter::new(config.requests_per_second, config.requests_per_second);
        let cache = Cache::builder()
            .max_capacity(config.cache_max_capacity)
            .time_to_live(Duration::from_secs(config.cache_ttl_secs))
            .build();
        Self {
            http,
            config,
            limiter,
            cache,
        }
    }
}

#[async_trait]
impl TagProvider for EthLabelsSource {
    async fn resolve_tags(&self, addr: &Address) -> DomainResult<Vec<ExternalTagCandidate>> {
        let chain = addr.chain().value();
        let addr_hex = format!("0x{}", hex::encode(addr.bytes()));
        let cache_key = (chain, addr_hex.clone());

        if let Some(cached) = self.cache.get(&cache_key).await {
            return Ok(cached);
        }

        self.limiter.acquire(1.0).await;

        let url = format!("{}/labels/{}", self.config.base_url, addr_hex);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| DomainError::InsufficientData(format!("eth-labels.com request failed: {e}")))?;

        if !resp.status().is_success() {
            tracing::warn!(
                status = %resp.status(),
                address = %addr_hex,
                "eth-labels.com: non-success response, treating as no labels"
            );
            self.cache.insert(cache_key, Vec::new()).await;
            return Ok(Vec::new());
        }

        let rows: Vec<LabelRow> = resp.json().await.map_err(|e| {
            DomainError::InsufficientData(format!("eth-labels.com: invalid JSON response: {e}"))
        })?;

        let candidates: Vec<ExternalTagCandidate> = rows
            .into_iter()
            .filter(|r| r.chain_id == chain)
            .map(|r| map_row(r, &addr_hex, chain))
            .collect();

        self.cache.insert(cache_key, candidates.clone()).await;
        Ok(candidates)
    }
}

fn map_row(row: LabelRow, addr_hex: &str, chain: u32) -> ExternalTagCandidate {
    let (category, confidence) = classify_slug(&row.label);
    ExternalTagCandidate {
        category,
        label_name: row.name_tag.filter(|s| !s.is_empty()),
        raw_label: row.label,
        source: TagSource::ThirdParty("eth_labels".to_string()),
        confidence,
        evidence_url: Some(format!("https://eth-labels.com/accounts?chainId={chain}&address={addr_hex}")),
    }
}

/// Maps an eth-labels.com taxonomy slug to a `TagCategory` + confidence.
/// The taxonomy has 896+ free-form slugs (protocol names, sectors,
/// incidents, ...) — only the risk-relevant ones get a specific category;
/// everything else falls through to `KnownService` at low confidence so no
/// data is dropped, just left unclassified.
fn classify_slug(slug: &str) -> (TagCategory, Confidence) {
    if slug.contains("ofac") || slug.contains("sanction") {
        return (TagCategory::Sanctioned, Confidence::HIGH);
    }
    if slug.contains("mixer") || slug == "tornado-cash" {
        return (TagCategory::Mixer, Confidence::MEDIUM);
    }
    if slug == "scam" || slug == "phish-hack" || slug.contains("exploit") || slug.contains("hack") {
        return (TagCategory::Scam, Confidence::MEDIUM);
    }
    if slug.contains("darknet") || slug.contains("darkweb") {
        return (TagCategory::Darknet, Confidence::MEDIUM);
    }
    if slug == "exchange" {
        return (TagCategory::Exchange, Confidence::LOW);
    }
    if slug.contains("bridge") {
        return (TagCategory::Bridge, Confidence::LOW);
    }
    if slug.contains("defi") || slug.contains("dex") || slug.contains("amm") {
        return (TagCategory::DefiProtocol, Confidence::LOW);
    }
    if slug.contains("gambling") || slug.contains("casino") || slug.contains("bet") {
        return (TagCategory::Gambling, Confidence::LOW);
    }
    (TagCategory::KnownService, Confidence::LOW)
}
