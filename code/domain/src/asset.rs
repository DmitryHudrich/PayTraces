use crate::chain::ChainId;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetId {
    chain: ChainId,
    kind: AssetKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssetKind {
    Native,
    Contract(Vec<u8>),
}

impl AssetId {
    pub fn native(chain: ChainId) -> Self {
        Self {
            chain,
            kind: AssetKind::Native,
        }
    }

    pub fn contract(chain: ChainId, contract_bytes: Vec<u8>) -> Self {
        Self {
            chain,
            kind: AssetKind::Contract(contract_bytes),
        }
    }

    pub fn chain(&self) -> ChainId {
        self.chain
    }

    pub fn kind(&self) -> &AssetKind {
        &self.kind
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
    id: AssetId,
    symbol: String,
    name: String,
    decimals: u8,
    standard: Option<TokenStandard>,
    is_stablecoin: bool,
    coingecko_id: Option<String>,
}

impl AssetMeta {
    pub fn new(
        id: AssetId,
        symbol: String,
        name: String,
        decimals: u8,
        standard: Option<TokenStandard>,
        is_stablecoin: bool,
        coingecko_id: Option<String>,
    ) -> Self {
        Self {
            id,
            symbol,
            name,
            decimals,
            standard,
            is_stablecoin,
            coingecko_id,
        }
    }

    pub fn id(&self) -> &AssetId {
        &self.id
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn decimals(&self) -> u8 {
        self.decimals
    }

    pub fn standard(&self) -> Option<TokenStandard> {
        self.standard
    }

    pub fn is_stablecoin(&self) -> bool {
        self.is_stablecoin
    }

    pub fn coingecko_id(&self) -> Option<&str> {
        self.coingecko_id.as_deref()
    }
}

#[derive(Debug, Clone)]
pub struct AssetRegistry {
    entries: Vec<AssetMeta>,
}

impl AssetRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn register(&mut self, meta: AssetMeta) {
        self.entries.retain(|e| e.id != meta.id);
        self.entries.push(meta);
    }

    pub fn get(&self, id: &AssetId) -> Option<&AssetMeta> {
        self.entries.iter().find(|e| &e.id == id)
    }
}

impl Default for AssetRegistry {
    fn default() -> Self {
        Self::new()
    }
}
