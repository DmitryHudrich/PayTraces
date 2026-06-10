use crate::chain::ChainId;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetId {
    pub chain: ChainId,
    pub kind: AssetKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetKind {
    Native,
    Contract(Vec<u8>),
}

impl AssetId {
    pub fn native(chain: ChainId) -> Self {
        Self { chain, kind: AssetKind::Native }
    }

    pub fn contract(chain: ChainId, contract_bytes: Vec<u8>) -> Self {
        Self { chain, kind: AssetKind::Contract(contract_bytes) }
    }
}

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            AssetKind::Native => write!(f, "{}:native", self.chain),
            AssetKind::Contract(b) => write!(f, "{}:0x{}", self.chain, hex::encode(b)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenStandard {
    Erc20,
    Erc721,
    Erc1155,
    Trc20,
    Trc10,
    Spl,
}

#[derive(Debug, Clone)]
pub struct AssetMeta {
    pub id: AssetId,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
    pub standard: Option<TokenStandard>,
    pub is_stablecoin: bool,
    pub coingecko_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AssetRegistry {
    entries: Vec<AssetMeta>,
}

impl AssetRegistry {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn register(&mut self, meta: AssetMeta) {
        self.entries.retain(|e| e.id != meta.id);
        self.entries.push(meta);
    }

    pub fn get(&self, id: &AssetId) -> Option<&AssetMeta> {
        self.entries.iter().find(|e| &e.id == id)
    }
}

