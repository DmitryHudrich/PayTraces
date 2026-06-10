use crate::asset::AssetId;
use crate::chain::ChainId;
use crate::primitives::{Address, Amount, BlockRef, TxRef};
use chrono::{DateTime, Utc};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TransferId {
    pub chain: ChainId,
    pub tx_hash: [u8; 32],
    pub index: u32,
}

impl TransferId {
    pub fn new(chain: ChainId, tx_hash: [u8; 32], index: u32) -> Self {
        Self { chain, tx_hash, index }
    }
}

impl fmt::Display for TransferId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:0x{}:{}", self.chain, hex::encode(self.tx_hash), self.index)
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
    pub id: TransferId,
    pub chain: ChainId,
    pub tx_ref: TxRef,
    pub from: Address,
    pub to: Address,
    pub asset: AssetId,
    pub amount: Amount,
    pub block: BlockRef,
    pub timestamp: DateTime<Utc>,
    pub kind: TransferKind,
    pub finality: Finality,
}

impl Transfer {
    pub fn is_confirmed(&self) -> bool {
        matches!(self.finality, Finality::Confirmed)
    }

    pub fn involves(&self, addr: &Address) -> bool {
        &self.from == addr || &self.to == addr
    }
}

#[derive(Debug, Clone)]
pub struct NormalizedBlock {
    pub block_ref: BlockRef,
    pub timestamp: DateTime<Utc>,
    pub transfers: Vec<Transfer>,
}

