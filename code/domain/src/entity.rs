use crate::primitives::{Address, Confidence};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityId(pub uuid::Uuid);

impl EntityId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityCategory {
    Exchange,
    Mixer,
    Bridge,
    DefiProtocol,
    Sanctioned { sanction_list: SanctionList },
    Scam,
    Gambling,
    Darknet,
    Mining,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SanctionList {
    Ofac,
    Eu,
    Un,
    Other(String),
}

#[derive(Debug, Clone)]
pub struct EntityLabel {
    pub name: String,
    pub url: Option<String>,
    pub source: LabelSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LabelSource {
    Manual,
    Chainalysis,
    Internal,
    Community,
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub id: EntityId,
    pub label: Option<EntityLabel>,
    pub category: EntityCategory,
    pub addresses: HashSet<Address>,
    pub risk_score: RiskScore,
}

impl Entity {
    pub fn new(category: EntityCategory, risk_score: RiskScore) -> Self {
        Self {
            id: EntityId::new(),
            label: None,
            category,
            addresses: HashSet::new(),
            risk_score,
        }
    }

    pub fn add_address(&mut self, addr: Address) {
        self.addresses.insert(addr);
    }

    pub fn contains(&self, addr: &Address) -> bool {
        self.addresses.contains(addr)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RiskScore(u8);

impl RiskScore {
    pub const CLEAN: Self = Self(0);
    pub const LOW: Self = Self(25);
    pub const MEDIUM: Self = Self(50);
    pub const HIGH: Self = Self(75);
    pub const CRITICAL: Self = Self(100);

    pub fn new(value: u8) -> Self {
        assert!(value <= 100);
        Self(value)
    }

    pub fn value(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct ClusterEvidence {
    pub addresses: Vec<Address>,
    pub heuristic: ClusteringHeuristic,
    pub confidence: Confidence,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusteringHeuristic {
    CoSpend,
    DepositAddressReuse,
    PeelingChain,
    BehavioralPattern(String),
    Manual,
}

