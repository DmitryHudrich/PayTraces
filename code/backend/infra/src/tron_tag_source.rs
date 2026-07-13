//! `TagProvider` adapter over `TronGridSource::resolve` (the existing
//! `LabelProvider` impl that already fetches Tronscan's `addressTag` for
//! account lookups, but until now was never applied anywhere as an actual
//! label). Tron's API gives free text only — no category — so every hit
//! maps to `KnownService` at low confidence rather than guessing a finer
//! category from the tag text.

use std::sync::Arc;

use async_trait::async_trait;

use domain::error::DomainResult;
use domain::label_tag::{TagCategory, TagSource};
use domain::ports::{ExternalTagCandidate, LabelProvider, TagProvider};
use domain::primitives::{Address, Confidence};

use crate::tron_source::TronGridSource;

pub struct TronTagSource {
    inner: Arc<TronGridSource>,
}

impl TronTagSource {
    pub fn new(inner: Arc<TronGridSource>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl TagProvider for TronTagSource {
    async fn resolve_tags(&self, addr: &Address) -> DomainResult<Vec<ExternalTagCandidate>> {
        let Some(label) = self.inner.resolve(addr).await? else {
            return Ok(Vec::new());
        };
        Ok(vec![ExternalTagCandidate {
            category: TagCategory::KnownService,
            label_name: Some(label.name().to_string()),
            raw_label: label.name().to_string(),
            source: TagSource::ThirdParty("tron_addresstag".to_string()),
            confidence: Confidence::LOW,
            evidence_url: label.url().map(str::to_string),
        }])
    }
}
