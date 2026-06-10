use crate::chain::ChainId;
use std::fmt;
use std::ops::{Add, Sub};
use uint::construct_uint;

construct_uint! {
    pub struct U256(4);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Address {
    pub chain: ChainId,
    pub bytes: Vec<u8>,
}

impl Address {
    pub fn new(chain: ChainId, bytes: Vec<u8>) -> Self {
        Self { chain, bytes }
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:0x{}", self.chain, hex::encode(&self.bytes))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Amount {
    pub raw: U256,
    pub decimals: u8,
}

impl Amount {
    pub fn new(raw: U256, decimals: u8) -> Self {
        Self { raw, decimals }
    }

    pub fn zero(decimals: u8) -> Self {
        Self { raw: U256::zero(), decimals }
    }

    pub fn is_zero(&self) -> bool {
        self.raw.is_zero()
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        assert_eq!(self.decimals, other.decimals, "decimals mismatch");
        self.raw.checked_add(other.raw).map(|raw| Self { raw, decimals: self.decimals })
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
        Self { raw: self.raw - rhs.raw, decimals: self.decimals }
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
        Amount { raw: scaled, decimals: amount.decimals }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockRef {
    pub chain: ChainId,
    pub height: u64,
    pub hash: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TxRef {
    pub chain: ChainId,
    pub hash: [u8; 32],
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

