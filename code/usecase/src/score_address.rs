use super::trace_funds::TraceFundsUseCase;
use domain::entity::{EntityCategory, RiskScore};
use domain::error::DomainResult;
use domain::ports::{EntityRepository, TransferRepository};
use domain::primitives::{Address, Ratio};
use domain::risk::{RiskEvidence, RiskReport, RiskSignal, RiskSignalKind};
use domain::trace::{TaintStrategy, TraceDirection, TraceLimits, TraceOrigin, TraceRequest};

pub struct ScoreAddressUseCase<R, E> {
    trace: TraceFundsUseCase<R, E>,
    entities: E,
}

impl<R: TransferRepository, E: EntityRepository + Clone> ScoreAddressUseCase<R, E> {
    pub fn new(transfers: R, entities: E) -> Self {
        Self {
            trace: TraceFundsUseCase::new(transfers, entities.clone()),
            entities,
        }
    }

    pub async fn execute(&self, addr: &Address) -> DomainResult<RiskReport> {
        tracing::info!(address = %super::addr_hex(addr), "score started");
        let mut signals: Vec<RiskSignal> = Vec::new();

        if let Some(entity) = self.entities.find_by_address(addr).await? {
            tracing::debug!(address = %super::addr_hex(addr), category = ?entity.category(), "direct entity label found");
            let (severity, kind) = match entity.category() {
                EntityCategory::Sanctioned { .. } => {
                    (RiskScore::CRITICAL, RiskSignalKind::SanctionedCounterparty)
                }
                EntityCategory::Mixer => (RiskScore::HIGH, RiskSignalKind::MixerInteraction),
                EntityCategory::Darknet => (RiskScore::CRITICAL, RiskSignalKind::DarknetMarket),
                EntityCategory::Scam => (RiskScore::HIGH, RiskSignalKind::DirectExposure),
                _ => (RiskScore::LOW, RiskSignalKind::DirectExposure),
            };
            signals.push(RiskSignal::new(
                kind,
                severity,
                format!(
                    "Address is labelled: {}",
                    entity.label().map(|l| l.name()).unwrap_or("unknown")
                ),
                RiskEvidence::EntityCategory(entity.category().clone()),
            ));
        }

        let backward = self
            .trace
            .execute(TraceRequest::new(
                TraceOrigin::Address(addr.clone()),
                TraceDirection::Backward,
                TaintStrategy::Haircut,
                TraceLimits::new(5, 200, 100, Some(Ratio::from_percent(5))),
                false,
            ))
            .await?;

        for sink in backward.terminal_sinks() {
            if sink.risk_score() >= RiskScore::HIGH.value() {
                let hops = backward
                    .paths()
                    .iter()
                    .filter(|p| p.destination() == Some(sink.address()))
                    .map(|p| p.depth())
                    .min()
                    .unwrap_or(0);

                let kind = if hops == 0 {
                    RiskSignalKind::DirectExposure
                } else {
                    RiskSignalKind::IndirectExposure { hops }
                };

                signals.push(RiskSignal::new(
                    kind,
                    RiskScore::new(sink.risk_score()),
                    format!(
                        "Funds traceable to high-risk sink ({})",
                        sink_label(sink.kind())
                    ),
                    RiskEvidence::SinkExposure(vec![sink.clone()]),
                ));
            }
        }

        let forward = self
            .trace
            .execute(TraceRequest::new(
                TraceOrigin::Address(addr.clone()),
                TraceDirection::Forward,
                TaintStrategy::Haircut,
                TraceLimits::new(5, 200, 100, Some(Ratio::from_percent(5))),
                false,
            ))
            .await?;

        for sink in forward.terminal_sinks() {
            if sink.risk_score() >= RiskScore::HIGH.value() {
                signals.push(RiskSignal::new(
                    RiskSignalKind::DirectExposure,
                    RiskScore::new(sink.risk_score()),
                    format!(
                        "Funds sent to high-risk destination ({})",
                        sink_label(sink.kind())
                    ),
                    RiskEvidence::SinkExposure(vec![sink.clone()]),
                ));
            }
        }

        let report = RiskReport::new(addr.clone(), signals);
        tracing::info!(
            address = %super::addr_hex(addr),
            score = report.overall_score().value(),
            signals = report.signals().len(),
            is_high_risk = report.is_high_risk(),
            "score complete"
        );
        Ok(report)
    }
}

fn sink_label(kind: &domain::trace::SinkKind) -> &'static str {
    use domain::trace::SinkKind;
    match kind {
        SinkKind::Exchange { .. } => "exchange",
        SinkKind::Bridge { .. } => "bridge",
        SinkKind::Mixer => "mixer",
        SinkKind::Sanctioned => "sanctioned",
        SinkKind::Darknet => "darknet",
        SinkKind::Unresolved => "unresolved",
    }
}

