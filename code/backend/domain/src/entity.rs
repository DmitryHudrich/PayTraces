use crate::primitives::{Address, Confidence};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntityId(uuid::Uuid);

impl EntityId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    pub fn from_uuid(id: uuid::Uuid) -> Self {
        Self(id)
    }

    pub fn value(&self) -> uuid::Uuid {
        self.0
    }
}

impl Default for EntityId {
    fn default() -> Self {
        Self::new()
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
    name: String,
    url: Option<String>,
    source: LabelSource,
}

impl EntityLabel {
    pub fn new(name: String, url: Option<String>, source: LabelSource) -> Self {
        Self { name, url, source }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    pub fn source(&self) -> &LabelSource {
        &self.source
    }
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
    id: EntityId,
    label: Option<EntityLabel>,
    category: EntityCategory,
    addresses: HashSet<Address>,
    risk_score: RiskScore,
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

    pub fn from_parts(
        id: EntityId,
        label: Option<EntityLabel>,
        category: EntityCategory,
        addresses: HashSet<Address>,
        risk_score: RiskScore,
    ) -> Self {
        Self {
            id,
            label,
            category,
            addresses,
            risk_score,
        }
    }

    pub fn id(&self) -> &EntityId {
        &self.id
    }

    pub fn label(&self) -> Option<&EntityLabel> {
        self.label.as_ref()
    }

    pub fn set_label(&mut self, label: EntityLabel) {
        self.label = Some(label);
    }

    pub fn category(&self) -> &EntityCategory {
        &self.category
    }

    pub fn addresses(&self) -> &HashSet<Address> {
        &self.addresses
    }

    pub fn risk_score(&self) -> RiskScore {
        self.risk_score
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
    addresses: Vec<Address>,
    heuristic: ClusteringHeuristic,
    confidence: Confidence,
    notes: Option<String>,
}

impl ClusterEvidence {
    pub fn new(
        addresses: Vec<Address>,
        heuristic: ClusteringHeuristic,
        confidence: Confidence,
        notes: Option<String>,
    ) -> Self {
        Self {
            addresses,
            heuristic,
            confidence,
            notes,
        }
    }

    pub fn addresses(&self) -> &[Address] {
        &self.addresses
    }

    pub fn heuristic(&self) -> &ClusteringHeuristic {
        &self.heuristic
    }

    pub fn confidence(&self) -> Confidence {
        self.confidence
    }

    pub fn notes(&self) -> Option<&str> {
        self.notes.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClusteringHeuristic {
    CoSpend,
    DepositAddressReuse,
    PeelingChain,
    FanOut,
    FanIn,
    SmurfingCycle,
    BehavioralPattern(String),
    Manual,
}
