use crate::asset::AssetId;
use crate::chain::ChainId;
use crate::primitives::{Address, Amount, BlockRef, TxRef};
use chrono::{DateTime, Utc};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransferId {
    chain: ChainId,
    tx_hash: [u8; 32],
    index: u32,
}

impl TransferId {
    pub fn new(chain: ChainId, tx_hash: [u8; 32], index: u32) -> Self {
        Self {
            chain,
            tx_hash,
            index,
        }
    }

    pub fn chain(&self) -> ChainId {
        self.chain
    }

    pub fn tx_hash(&self) -> &[u8; 32] {
        &self.tx_hash
    }

    pub fn index(&self) -> u32 {
        self.index
    }
}

impl fmt::Display for TransferId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:0x{}:{}",
            self.chain,
            hex::encode(self.tx_hash),
            self.index
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferKind {
    Native,
    Token {
        contract: Address,
        standard: crate::asset::TokenStandard,
    },
    Internal,
    Fee,
    UtxoEdge {
        vin_index: u32,
        vout_index: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Finality {
    Unconfirmed,
    Pending { confirmations: u64 },
    Confirmed,
    Reorged,
}

#[derive(Debug, Clone)]
pub struct Transfer {
    id: TransferId,
    chain: ChainId,
    tx_ref: TxRef,
    from: Address,
    to: Address,
    asset: AssetId,
    amount: Amount,
    block: BlockRef,
    timestamp: DateTime<Utc>,
    kind: TransferKind,
    finality: Finality,
}

impl Transfer {
    pub fn new(
        id: TransferId,
        chain: ChainId,
        tx_ref: TxRef,
        from: Address,
        to: Address,
        asset: AssetId,
        amount: Amount,
        block: BlockRef,
        timestamp: DateTime<Utc>,
        kind: TransferKind,
        finality: Finality,
    ) -> Self {
        Self {
            id,
            chain,
            tx_ref,
            from,
            to,
            asset,
            amount,
            block,
            timestamp,
            kind,
            finality,
        }
    }

    pub fn id(&self) -> &TransferId {
        &self.id
    }

    pub fn chain(&self) -> ChainId {
        self.chain
    }

    pub fn tx_ref(&self) -> TxRef {
        self.tx_ref
    }

    pub fn from(&self) -> &Address {
        &self.from
    }

    pub fn to(&self) -> &Address {
        &self.to
    }

    pub fn asset(&self) -> &AssetId {
        &self.asset
    }

    pub fn amount(&self) -> Amount {
        self.amount
    }

    pub fn block(&self) -> BlockRef {
        self.block
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    pub fn kind(&self) -> &TransferKind {
        &self.kind
    }

    pub fn finality(&self) -> Finality {
        self.finality
    }

    pub fn is_confirmed(&self) -> bool {
        matches!(self.finality, Finality::Confirmed)
    }

    pub fn involves(&self, addr: &Address) -> bool {
        &self.from == addr || &self.to == addr
    }
}

#[derive(Debug, Clone)]
pub struct NormalizedBlock {
    block_ref: BlockRef,
    timestamp: DateTime<Utc>,
    transfers: Vec<Transfer>,
}

impl NormalizedBlock {
    pub fn new(block_ref: BlockRef, timestamp: DateTime<Utc>, transfers: Vec<Transfer>) -> Self {
        Self {
            block_ref,
            timestamp,
            transfers,
        }
    }

    pub fn block_ref(&self) -> BlockRef {
        self.block_ref
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    pub fn transfers(&self) -> &[Transfer] {
        &self.transfers
    }
}
