use crate::chain::{AddressEncoding, ChainId};
use std::fmt;
use std::ops::{Add, Sub};
use thiserror::Error;
use uint::construct_uint;

construct_uint! {
    pub struct U256(4);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Address {
    chain: ChainId,
    bytes: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum AddressParseError {
    #[error("invalid hex: {0}")]
    Hex(String),
    #[error("invalid Base58Check: {0}")]
    Base58(String),
    #[error("invalid Bech32: {0}")]
    Bech32(String),
    #[error("invalid address length: expected {expected}, got {actual}")]
    Length { expected: usize, actual: usize },
    #[error("invalid Tron version byte: expected 0x41, got 0x{0:02x}")]
    TronVersion(u8),
    #[error("unsupported encoding for chain: {0}")]
    UnsupportedEncoding(ChainId),
    #[error("empty address")]
    Empty,
}

impl Address {
    pub fn new(chain: ChainId, bytes: Vec<u8>) -> Self {
        Self { chain, bytes }
    }

    pub fn chain(&self) -> ChainId {
        self.chain
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn hex(&self) -> String {
        hex::encode(&self.bytes)
    }

    pub fn parse(chain: ChainId, s: &str) -> Result<Self, AddressParseError> {
        if s.is_empty() {
            return Err(AddressParseError::Empty);
        }
        let encoding = chain_encoding(chain);
        match encoding {
            AddressEncoding::Hex20 => {
                let stripped = s.strip_prefix("0x").unwrap_or(s);
                let bytes = hex::decode(stripped)
                    .map_err(|e| AddressParseError::Hex(e.to_string()))?;
                if bytes.len() != 20 {
                    return Err(AddressParseError::Length {
                        expected: 20,
                        actual: bytes.len(),
                    });
                }
                Ok(Self { chain, bytes })
            }
            AddressEncoding::TronBase58Check => {
                let bytes = bs58::decode(s)
                    .with_check(None)
                    .into_vec()
                    .map_err(|e| AddressParseError::Base58(e.to_string()))?;
                if bytes.len() != 21 {
                    return Err(AddressParseError::Length {
                        expected: 21,
                        actual: bytes.len(),
                    });
                }
                if bytes[0] != 0x41 {
                    return Err(AddressParseError::TronVersion(bytes[0]));
                }
                Ok(Self { chain, bytes })
            }
            AddressEncoding::Base58 => {
                let bytes = bs58::decode(s)
                    .into_vec()
                    .map_err(|e| AddressParseError::Base58(e.to_string()))?;
                if bytes.len() != 32 {
                    return Err(AddressParseError::Length {
                        expected: 32,
                        actual: bytes.len(),
                    });
                }
                Ok(Self { chain, bytes })
            }
            AddressEncoding::Bech32 => Err(AddressParseError::UnsupportedEncoding(chain)),
        }
    }

    pub fn canonical(&self) -> String {
        match chain_encoding(self.chain) {
            AddressEncoding::Hex20 => format!("0x{}", hex::encode(&self.bytes)),
            AddressEncoding::TronBase58Check => {
                bs58::encode(&self.bytes).with_check().into_string()
            }
            AddressEncoding::Base58 => bs58::encode(&self.bytes).into_string(),
            AddressEncoding::Bech32 => format!("bech32?:0x{}", hex::encode(&self.bytes)),
        }
    }
}

fn chain_encoding(chain: ChainId) -> AddressEncoding {
    match chain {
        ChainId::ETH => AddressEncoding::Hex20,
        ChainId::TRON => AddressEncoding::TronBase58Check,
        ChainId::BTC => AddressEncoding::Bech32,
        ChainId::SOLANA => AddressEncoding::Base58,
        _ => AddressEncoding::Hex20,
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.chain, self.canonical())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Amount {
    raw: U256,
    decimals: u8,
}

impl Amount {
    pub fn new(raw: U256, decimals: u8) -> Self {
        Self { raw, decimals }
    }

    pub fn zero(decimals: u8) -> Self {
        Self {
            raw: U256::zero(),
            decimals,
        }
    }

    pub fn raw(self) -> U256 {
        self.raw
    }

    pub fn decimals(self) -> u8 {
        self.decimals
    }

    pub fn is_zero(&self) -> bool {
        self.raw.is_zero()
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        assert_eq!(self.decimals, other.decimals, "decimals mismatch");
        self.raw.checked_add(other.raw).map(|raw| Self {
            raw,
            decimals: self.decimals,
        })
    }

    pub fn ratio_of(&self, total: &Self) -> Ratio {
        assert_eq!(self.decimals, total.decimals);
        if total.raw.is_zero() {
            return Ratio::ZERO;
        }
        let scaled = self.raw * U256::from(1_000_000u64);
        let ratio = scaled / total.raw;
        Ratio(ratio.as_u64().min(1_000_000))
    }
}

impl Add for Amount {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        self.checked_add(rhs).expect("amount overflow")
    }
}

impl Sub for Amount {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        assert_eq!(self.decimals, rhs.decimals);
        Self {
            raw: self.raw - rhs.raw,
            decimals: self.decimals,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ratio(u64);

impl Ratio {
    pub const ZERO: Self = Self(0);
    pub const ONE: Self = Self(1_000_000);

    pub fn from_percent(pct: u8) -> Self {
        Self(pct as u64 * 10_000)
    }

    pub fn as_f64(self) -> f64 {
        self.0 as f64 / 1_000_000.0
    }

    pub fn apply_to(self, amount: Amount) -> Amount {
        let scaled = amount.raw * U256::from(self.0) / U256::from(1_000_000u64);
        Amount {
            raw: scaled,
            decimals: amount.decimals,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockRef {
    chain: ChainId,
    height: u64,
    hash: [u8; 32],
}

impl BlockRef {
    pub fn new(chain: ChainId, height: u64, hash: [u8; 32]) -> Self {
        Self {
            chain,
            height,
            hash,
        }
    }

    pub fn chain(self) -> ChainId {
        self.chain
    }

    pub fn height(self) -> u64 {
        self.height
    }

    pub fn hash(self) -> [u8; 32] {
        self.hash
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TxRef {
    chain: ChainId,
    hash: [u8; 32],
}

impl TxRef {
    pub fn new(chain: ChainId, hash: [u8; 32]) -> Self {
        Self { chain, hash }
    }

    pub fn chain(self) -> ChainId {
        self.chain
    }

    pub fn hash(self) -> [u8; 32] {
        self.hash
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Confidence(u8);

impl Confidence {
    pub const LOW: Self = Self(25);
    pub const MEDIUM: Self = Self(50);
    pub const HIGH: Self = Self(75);
    pub const CERTAIN: Self = Self(100);

    pub fn new(value: u8) -> Self {
        assert!(value <= 100);
        Self(value)
    }

    pub fn value(self) -> u8 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_evm_address() {
        let a = Address::parse(ChainId::ETH, "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045").unwrap();
        assert_eq!(a.chain(), ChainId::ETH);
        assert_eq!(a.bytes().len(), 20);
        assert!(a.canonical().starts_with("0x"));
    }

    #[test]
    fn parse_tron_address() {
        let a = Address::parse(ChainId::TRON, "TWhC1FvBoycGpu2bf5MSuGYva9oWcUD87A").unwrap();
        assert_eq!(a.chain(), ChainId::TRON);
        assert_eq!(a.bytes().len(), 21);
        assert_eq!(a.bytes()[0], 0x41);
        assert_eq!(a.canonical(), "TWhC1FvBoycGpu2bf5MSuGYva9oWcUD87A");
    }

    #[test]
    fn reject_bad_tron_version() {
        let bytes = [0x42u8; 21];
        let s = bs58::encode(&bytes).with_check().into_string();
        let err = Address::parse(ChainId::TRON, &s).unwrap_err();
        assert!(matches!(err, AddressParseError::TronVersion(0x42)));
    }
}
