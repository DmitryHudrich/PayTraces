use crate::entity::RiskScore;
use crate::label_tag::{LabelTag, TagCategory, TagId};
use crate::primitives::Address;
use crate::trace::Sink;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct SanctionsCheckResult {
    address: Address,
    /// Every currently-active `Sanctioned` tag on the address's entity — an
    /// address can be on OFAC *and* EU lists simultaneously, so this is a
    /// list rather than a single `Option<SanctionList>`.
    sanction_tags: Vec<LabelTag>,
}

impl SanctionsCheckResult {
    pub fn new(address: Address, sanction_tags: Vec<LabelTag>) -> Self {
        Self { address, sanction_tags }
    }

    pub fn address(&self) -> &Address {
        &self.address
    }

    pub fn is_sanctioned(&self) -> bool {
        !self.sanction_tags.is_empty()
    }

    pub fn sanction_tags(&self) -> &[LabelTag] {
        &self.sanction_tags
    }
}

#[derive(Debug, Clone)]
pub struct RiskReport {
    subject: Address,
    overall_score: RiskScore,
    signals: Vec<RiskSignal>,
    generated_at: DateTime<Utc>,
}

impl RiskReport {
    /// Construct a report and auto-aggregate with the legacy "max severity"
    /// strategy. Kept for callers that don't have a `ScoreConfig` handy.
    pub fn new(subject: Address, signals: Vec<RiskSignal>) -> Self {
        let overall_score = Self::aggregate_score(&signals);
        Self {
            subject,
            overall_score,
            signals,
            generated_at: Utc::now(),
        }
    }

    /// Construct a report with a pre-computed score — used by callers that
    /// run their own aggregation (e.g. `RiskService` with config-driven
    /// dedup + weighted-count rules).
    pub fn with_score(subject: Address, signals: Vec<RiskSignal>, overall_score: RiskScore) -> Self {
        Self {
            subject,
            overall_score,
            signals,
            generated_at: Utc::now(),
        }
    }

    fn aggregate_score(signals: &[RiskSignal]) -> RiskScore {
        if signals.is_empty() {
            return RiskScore::CLEAN;
        }
        let max = signals
            .iter()
            .map(|s| s.severity.value())
            .max()
            .unwrap_or(0);
        RiskScore::new(max)
    }

    pub fn subject(&self) -> &Address {
        &self.subject
    }

    pub fn overall_score(&self) -> RiskScore {
        self.overall_score
    }

    pub fn signals(&self) -> &[RiskSignal] {
        &self.signals
    }

    pub fn generated_at(&self) -> DateTime<Utc> {
        self.generated_at
    }

    pub fn is_high_risk(&self) -> bool {
        self.overall_score >= RiskScore::HIGH
    }
}

#[derive(Debug, Clone)]
pub struct RiskSignal {
    kind: RiskSignalKind,
    severity: RiskScore,
    description: String,
    evidence: RiskEvidence,
}

impl RiskSignal {
    pub fn new(
        kind: RiskSignalKind,
        severity: RiskScore,
        description: String,
        evidence: RiskEvidence,
    ) -> Self {
        Self {
            kind,
            severity,
            description,
            evidence,
        }
    }

    pub fn kind(&self) -> &RiskSignalKind {
        &self.kind
    }

    pub fn severity(&self) -> RiskScore {
        self.severity
    }

    pub fn description(&self) -> &str {
        &self.description
    }

    pub fn evidence(&self) -> &RiskEvidence {
        &self.evidence
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskSignalKind {
    DirectExposure,
    IndirectExposure { hops: u32 },
    SanctionedCounterparty,
    MixerInteraction,
    DarknetMarket,
    RapidLayering,
    HighVelocity,
    NewAddress,
    NoKyc,
}

#[derive(Debug, Clone)]
pub enum RiskEvidence {
    SinkExposure(Vec<Sink>),
    /// A specific active `LabelTag` on the address's entity drove this
    /// signal — `tag_id` lets an investigator jump straight to the tag
    /// that produced it (ТЗ §7).
    Tag { tag_id: TagId, category: TagCategory },
    TransactionPattern(String),
    Manual(String),
}
