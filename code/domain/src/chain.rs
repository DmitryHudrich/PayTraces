use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChainId(u32);

impl ChainId {
    pub const ETH: Self = Self(1);
    pub const TRON: Self = Self(195);
    pub const BTC: Self = Self(0);
    pub const SOLANA: Self = Self(501);

    pub fn new(value: u32) -> Self {
        Self(value)
    }

    pub fn value(self) -> u32 {
        self.0
    }
}

impl fmt::Display for ChainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ChainId::ETH => write!(f, "eth"),
            ChainId::TRON => write!(f, "tron"),
            ChainId::BTC => write!(f, "btc"),
            ChainId::SOLANA => write!(f, "sol"),
            ChainId(id) => write!(f, "chain:{id}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressModel {
    Account,
    Utxo,
}

#[derive(Debug, Clone)]
pub struct ChainMeta {
    pub id: ChainId,
    pub name: &'static str,
    pub address_model: AddressModel,
    pub confirmation_depth: u64,
    pub native_asset_symbol: &'static str,
    pub native_asset_decimals: u8,
}

pub struct ChainRegistry {
    entries: Vec<ChainMeta>,
}

impl ChainRegistry {
    pub fn default_registry() -> Self {
        Self {
            entries: vec![
                ChainMeta {
                    id: ChainId::ETH,
                    name: "Ethereum",
                    address_model: AddressModel::Account,
                    confirmation_depth: 12,
                    native_asset_symbol: "ETH",
                    native_asset_decimals: 18,
                },
                ChainMeta {
                    id: ChainId::TRON,
                    name: "Tron",
                    address_model: AddressModel::Account,
                    confirmation_depth: 20,
                    native_asset_symbol: "TRX",
                    native_asset_decimals: 6,
                },
                ChainMeta {
                    id: ChainId::BTC,
                    name: "Bitcoin",
                    address_model: AddressModel::Utxo,
                    confirmation_depth: 6,
                    native_asset_symbol: "BTC",
                    native_asset_decimals: 8,
                },
            ],
        }
    }

    pub fn get(&self, id: ChainId) -> Option<&ChainMeta> {
        self.entries.iter().find(|m| m.id == id)
    }

    pub fn register(&mut self, meta: ChainMeta) {
        self.entries.retain(|m| m.id != meta.id);
        self.entries.push(meta);
    }
}
