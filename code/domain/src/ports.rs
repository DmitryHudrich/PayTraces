use crate::asset::{AssetId, AssetMeta};
use crate::chain::ChainId;
use crate::entity::{ClusterEvidence, Entity, EntityCategory, EntityId};
use crate::error::DomainResult;
use crate::graph::{GraphRequest, TransferGraph};
use crate::primitives::{Address, BlockRef};
use crate::risk::{RiskReport, SanctionsCheckResult};
use crate::trace::{TraceRequest, TraceResult};
use crate::transfer::{NormalizedBlock, Transfer};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub struct BlockRange {
    from_height: u64,
    to_height: u64,
    time_window: Option<(DateTime<Utc>, DateTime<Utc>)>,
}

impl BlockRange {
    pub fn new(from_height: u64, to_height: u64) -> Self {
        assert!(from_height <= to_height);
        Self {
            from_height,
            to_height,
            time_window: None,
        }
    }

    pub fn single(height: u64) -> Self {
        Self::new(height, height)
    }

    pub fn full() -> Self {
        Self::new(0, u64::MAX)
    }

    pub fn with_time_window(mut self, from: DateTime<Utc>, to: DateTime<Utc>) -> Self {
        assert!(from <= to);
        self.time_window = Some((from, to));
        self
    }

    pub fn from_height(&self) -> u64 {
        self.from_height
    }

    pub fn to_height(&self) -> u64 {
        self.to_height
    }

    pub fn time_window(&self) -> Option<(DateTime<Utc>, DateTime<Utc>)> {
        self.time_window
    }

    pub fn is_unbounded(&self) -> bool {
        self.from_height == 0 && self.to_height == u64::MAX && self.time_window.is_none()
    }
}

impl Default for BlockRange {
    fn default() -> Self {
        Self::full()
    }
}

#[async_trait]
pub trait ChainSource: Send + Sync {
    fn chain_id(&self) -> ChainId;
    async fn latest_block(&self) -> DomainResult<BlockRef>;
    async fn fetch_block(&self, height: u64) -> DomainResult<NormalizedBlock>;
    async fn transfers_for_address(
        &self,
        addr: &Address,
        range: BlockRange,
        max_transfers: usize,
    ) -> DomainResult<Vec<Transfer>>;
}

#[async_trait]
pub trait ChainSourceRegistry: Send + Sync {
    fn source(&self, chain: ChainId) -> Option<Arc<dyn ChainSource>>;
    fn supported_chains(&self) -> Vec<ChainId>;
}

#[async_trait]
pub trait TransferRepository: Send + Sync {
    async fn save(&self, transfers: &[Transfer]) -> DomainResult<()>;
    async fn find_by_address(
        &self,
        addr: &Address,
        range: Option<BlockRange>,
    ) -> DomainResult<Vec<Transfer>>;
    async fn find_by_tx(&self, chain: ChainId, tx_hash: &[u8; 32]) -> DomainResult<Vec<Transfer>>;
    async fn find_outgoing(
        &self,
        addr: &Address,
        after: Option<DateTime<Utc>>,
    ) -> DomainResult<Vec<Transfer>>;
    async fn find_incoming(
        &self,
        addr: &Address,
        after: Option<DateTime<Utc>>,
    ) -> DomainResult<Vec<Transfer>>;
}

#[async_trait]
pub trait EntityRepository: Send + Sync {
    async fn find_by_id(&self, id: &EntityId) -> DomainResult<Option<Entity>>;
    async fn find_by_address(&self, addr: &Address) -> DomainResult<Option<Entity>>;
    async fn save(&self, entity: &Entity) -> DomainResult<()>;
    async fn list_sanctioned(&self) -> DomainResult<Vec<Entity>>;
}

#[async_trait]
pub trait AssetRepository: Send + Sync {
    async fn find(&self, id: &AssetId) -> DomainResult<Option<AssetMeta>>;
    async fn save(&self, meta: &AssetMeta) -> DomainResult<()>;
}

#[async_trait]
pub trait LabelProvider: Send + Sync {
    async fn resolve(&self, addr: &Address) -> DomainResult<Option<crate::entity::EntityLabel>>;
}

/// Application port covering data ingestion and transfer-graph construction.
#[async_trait]
pub trait IngestionPort: Send + Sync {
    async fn build_graph(
        &self,
        origin: &Address,
        req: GraphRequest,
    ) -> DomainResult<TransferGraph>;
}

/// Application port covering risk analysis: tracing, scoring, sanctions and clustering.
#[async_trait]
pub trait RiskPort: Send + Sync {
    async fn trace(&self, req: TraceRequest) -> DomainResult<TraceResult>;

    async fn score(&self, addr: &Address) -> DomainResult<RiskReport>;

    async fn check_sanctions(&self, addr: &Address) -> DomainResult<SanctionsCheckResult>;

    async fn check_sanctions_batch(
        &self,
        addrs: &[Address],
    ) -> DomainResult<Vec<SanctionsCheckResult>>;

    async fn deposit_reuse_cluster(
        &self,
        deposit_addr: &Address,
    ) -> DomainResult<Option<ClusterEvidence>>;

    async fn detect_peeling_chain(
        &self,
        addr: &Address,
    ) -> DomainResult<Option<ClusterEvidence>>;

    async fn save_cluster(
        &self,
        evidence: ClusterEvidence,
        category: EntityCategory,
    ) -> DomainResult<()>;
}
