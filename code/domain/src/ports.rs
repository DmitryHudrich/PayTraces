use crate::asset::{AssetId, AssetMeta};
use crate::chain::ChainId;
use crate::entity::{Entity, EntityId};
use crate::error::DomainResult;
use crate::primitives::{Address, BlockRef};
use crate::transfer::{NormalizedBlock, Transfer};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy)]
pub struct BlockRange {
    pub from_height: u64,
    pub to_height: u64,
}

impl BlockRange {
    pub fn new(from_height: u64, to_height: u64) -> Self {
        assert!(from_height <= to_height);
        Self {
            from_height,
            to_height,
        }
    }

    pub fn single(height: u64) -> Self {
        Self {
            from_height: height,
            to_height: height,
        }
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
    ) -> DomainResult<Vec<Transfer>>;
}

#[async_trait]
pub trait TransferRepository: Send + Sync {
    async fn save(&self, transfers: &[Transfer]) -> DomainResult<()>;
    async fn find_by_address(
        &self,
        addr: &Address,
        range: Option<BlockRange>,
    ) -> DomainResult<Vec<Transfer>>;
    async fn find_by_tx(
        &self,
        chain: ChainId,
        tx_hash: &[u8; 32],
    ) -> DomainResult<Vec<Transfer>>;
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
    async fn resolve(
        &self,
        addr: &Address,
    ) -> DomainResult<Option<crate::entity::EntityLabel>>;
}

