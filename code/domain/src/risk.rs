use crate::entity::{EntityCategory, RiskScore};
use crate::primitives::Address;
use crate::trace::Sink;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct RiskReport {
    subject: Address,
    overall_score: RiskScore,
    signals: Vec<RiskSignal>,
    generated_at: DateTime<Utc>,
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
    EntityCategory(EntityCategory),
    TransactionPattern(String),
    Manual(String),
}
