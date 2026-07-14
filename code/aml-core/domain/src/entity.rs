use crate::label_tag::{LabelTag, TagAggregationStrategy, aggregate_risk_score};
use crate::primitives::{Address, Confidence};
use chrono::{DateTime, Utc};

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

/// A single free-text label resolved by an external, best-effort source
/// (e.g. Tronscan's `addressTag`). Distinct from `LabelTag` — this is the
/// raw "name + url + who said so" shape returned by `ports::LabelProvider`,
/// not an entity-attached, lifecycle-managed tag.
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

/// Semantic role of an address — used by trace and heuristics to skip
/// infrastructure (contracts, exchanges) as cash-outs and to weight risk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressKind {
    Eoa,
    Contract,
    KnownService(String),
    Unknown,
}

impl AddressKind {
    pub fn as_str(&self) -> &str {
        match self {
            AddressKind::Eoa => "eoa",
            AddressKind::Contract => "contract",
            AddressKind::KnownService(_) => "known_service",
            AddressKind::Unknown => "unknown",
        }
    }
}

/// One address attached to an `Entity`, with when it was attached.
#[derive(Debug, Clone)]
pub struct AddressRef {
    address: Address,
    attached_at: DateTime<Utc>,
}

impl AddressRef {
    pub fn new(address: Address, attached_at: DateTime<Utc>) -> Self {
        Self { address, attached_at }
    }

    pub fn address(&self) -> &Address {
        &self.address
    }

    pub fn attached_at(&self) -> DateTime<Utc> {
        self.attached_at
    }
}

#[derive(Debug, Clone)]
pub struct Entity {
    id: EntityId,
    addresses: Vec<AddressRef>,
    tags: Vec<LabelTag>,
    created_at: DateTime<Utc>,
}

impl Entity {
    pub fn new() -> Self {
        Self {
            id: EntityId::new(),
            addresses: Vec::new(),
            tags: Vec::new(),
            created_at: Utc::now(),
        }
    }

    pub fn from_parts(
        id: EntityId,
        addresses: Vec<AddressRef>,
        tags: Vec<LabelTag>,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            addresses,
            tags,
            created_at,
        }
    }

    pub fn id(&self) -> &EntityId {
        &self.id
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn addresses(&self) -> &[AddressRef] {
        &self.addresses
    }

    pub fn add_address(&mut self, addr: Address) {
        if !self.addresses.iter().any(|a| a.address() == &addr) {
            self.addresses.push(AddressRef::new(addr, Utc::now()));
        }
    }

    pub fn remove_address(&mut self, addr: &Address) {
        self.addresses.retain(|a| a.address() != addr);
    }

    pub fn contains(&self, addr: &Address) -> bool {
        self.addresses.iter().any(|a| a.address() == addr)
    }

    pub fn tags(&self) -> &[LabelTag] {
        &self.tags
    }

    pub fn tags_mut(&mut self) -> &mut Vec<LabelTag> {
        &mut self.tags
    }

    pub fn active_tags(&self) -> impl Iterator<Item = &LabelTag> + '_ {
        let now = Utc::now();
        self.tags.iter().filter(move |t| t.is_active_at(now))
    }

    pub fn add_tag(&mut self, tag: LabelTag) {
        self.tags.push(tag);
    }

    pub fn aggregate_risk_score(&self, strategy: TagAggregationStrategy) -> RiskScore {
        aggregate_risk_score(&self.tags, strategy, Utc::now())
    }
}

impl Default for Entity {
    fn default() -> Self {
        Self::new()
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
    TemporalBurst,
    FixedAmountClustering,
    DwellTimePassThrough,
    BehavioralPattern(String),
    Manual,
}
