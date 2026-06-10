use crate::entity::{EntityCategory, RiskScore};
use crate::primitives::Address;
use crate::trace::Sink;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct RiskReport {
    pub subject: Address,
    pub overall_score: RiskScore,
    pub signals: Vec<RiskSignal>,
    pub generated_at: DateTime<Utc>,
}

impl RiskReport {
    pub fn new(subject: Address, signals: Vec<RiskSignal>) -> Self {
        let overall_score = Self::aggregate_score(&signals);
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
        let max = signals.iter().map(|s| s.severity.value()).max().unwrap_or(0);
        RiskScore::new(max)
    }

    pub fn is_high_risk(&self) -> bool {
        self.overall_score >= RiskScore::HIGH
    }
}

#[derive(Debug, Clone)]
pub struct RiskSignal {
    pub kind: RiskSignalKind,
    pub severity: RiskScore,
    pub description: String,
    pub evidence: RiskEvidence,
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
    EntityCategory(EntityCategory),
    TransactionPattern(String),
    Manual(String),
}

