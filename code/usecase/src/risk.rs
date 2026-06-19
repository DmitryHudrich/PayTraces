use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use domain::entity::{
    ClusterEvidence, ClusteringHeuristic, Entity, EntityCategory, RiskScore, SanctionList,
};
use domain::error::DomainResult;
use domain::ports::{EntityRepository, RiskPort, TransferRepository};
use domain::primitives::{Address, Amount, Confidence, Ratio};
use domain::risk::{
    RiskEvidence, RiskReport, RiskSignal, RiskSignalKind, SanctionsCheckResult,
};
use domain::trace::{
    FlowPath, Sink, SinkKind, TaintStrategy, TraceDirection, TraceLimits, TraceOrigin, TraceRequest,
    TraceResult, TraceStats,
};
use domain::transfer::Transfer;

pub struct RiskService<R, E> {
    transfers: R,
    entities: E,
}

impl<R, E> RiskService<R, E> {
    pub fn new(transfers: R, entities: E) -> Self {
        Self {
            transfers,
            entities,
        }
    }
}

#[async_trait]
impl<R, E> RiskPort for RiskService<R, E>
where
    R: TransferRepository,
    E: EntityRepository,
{
    #[tracing::instrument(skip(self, req), fields(
        direction = ?req.direction(),
        strategy = ?req.strategy(),
        max_hops = req.limits().max_hops(),
        max_addresses = req.limits().max_addresses(),
    ))]
    async fn trace(&self, req: TraceRequest) -> DomainResult<TraceResult> {
        tracing::info!(
            direction = ?req.direction(),
            strategy = ?req.strategy(),
            max_hops = req.limits().max_hops(),
            max_addresses = req.limits().max_addresses(),
            "trace started"
        );

        let seeds = self.resolve_seeds(req.origin(), req.direction()).await?;
        tracing::debug!(seeds = seeds.len(), "trace seeds resolved");

        let mut paths: Vec<FlowPath> = Vec::new();
        let mut sinks: Vec<Sink> = Vec::new();
        let mut addresses_visited: HashSet<Address> = HashSet::new();
        let mut transfers_evaluated: usize = 0;
        let mut truncated = false;

        let mut out_cache: HashMap<Address, Arc<Vec<Arc<Transfer>>>> = HashMap::new();
        let mut in_cache: HashMap<Address, Arc<Vec<Arc<Transfer>>>> = HashMap::new();

        let mut queue: Vec<(Vec<Arc<Transfer>>, Address, Amount, HashSet<Address>)> = seeds
            .into_iter()
            .map(|t| {
                let (addr, origin_side) = match req.direction() {
                    TraceDirection::Forward | TraceDirection::Both => {
                        (t.to().clone(), t.from().clone())
                    }
                    TraceDirection::Backward => (t.from().clone(), t.to().clone()),
                };
                let amount = t.amount();
                let mut path_visited = HashSet::new();
                path_visited.insert(addr.clone());
                path_visited.insert(origin_side);
                (vec![Arc::new(t)], addr, amount, path_visited)
            })
            .collect();

        while let Some((path, addr, tainted, path_visited)) = queue.pop() {
            if path.len() as u32 > req.limits().max_hops() {
                tracing::debug!(
                    hops = path.len(),
                    max_hops = req.limits().max_hops(),
                    "path truncated: max hops"
                );
                truncated = true;
                continue;
            }
            if addresses_visited.len() >= req.limits().max_addresses() {
                tracing::warn!(
                    addresses = addresses_visited.len(),
                    "trace truncated: max_addresses reached"
                );
                truncated = true;
                break;
            }
            if paths.len() >= req.limits().max_paths() {
                tracing::warn!(paths = paths.len(), "trace truncated: max_paths reached");
                truncated = true;
                break;
            }

            tracing::trace!(address = %crate::addr_hex(&addr), depth = path.len(), "trace visiting");

            addresses_visited.insert(addr.clone());

            let next_arcs = self
                .fetch_cached(&mut out_cache, &mut in_cache, &addr, req.direction(), false)
                .await?;
            transfers_evaluated += next_arcs.len();

            let is_haircut = matches!(
                req.strategy(),
                TaintStrategy::Haircut | TaintStrategy::Fifo | TaintStrategy::Lifo
            );

            let total_in: Amount = if matches!(req.direction(), TraceDirection::Backward) {
                next_arcs
                    .iter()
                    .filter(|t| t.amount().decimals() == tainted.decimals())
                    .fold(Amount::zero(tainted.decimals()), |acc, t| acc + t.amount())
            } else if is_haircut || next_arcs.is_empty() {
                let inc = self
                    .fetch_cached(&mut out_cache, &mut in_cache, &addr, req.direction(), true)
                    .await?;
                inc.iter()
                    .filter(|t| t.amount().decimals() == tainted.decimals())
                    .fold(Amount::zero(tainted.decimals()), |acc, t| acc + t.amount())
            } else {
                tainted
            };

            let taint_ratio: Ratio = if is_haircut {
                if total_in.is_zero() {
                    Ratio::ONE
                } else {
                    tainted.ratio_of(&total_in)
                }
            } else {
                Ratio::ONE
            };

            if next_arcs.is_empty() {
                let sink = self
                    .classify_sink(&addr, tainted, total_in.max(tainted))
                    .await?;
                sinks.push(sink);
                let depth = path.len() as u32;
                let hops = path.iter().map(|a| (**a).clone()).collect();
                paths.push(FlowPath::new(hops, tainted, taint_ratio, depth));
                continue;
            }

            for t in next_arcs.iter() {
                if !req.include_unconfirmed() && !t.is_confirmed() {
                    continue;
                }

                let propagated = match req.strategy() {
                    TaintStrategy::Poison => t.amount(),
                    _ => taint_ratio.apply_to(t.amount()),
                };

                if let Some(min_ratio) = req.limits().min_amount_ratio()
                    && propagated.ratio_of(&t.amount()) < min_ratio
                {
                    continue;
                }

                let next_addr = match req.direction() {
                    TraceDirection::Forward | TraceDirection::Both => t.to().clone(),
                    TraceDirection::Backward => t.from().clone(),
                };

                if path_visited.contains(&next_addr) {
                    continue;
                }

                let mut next_visited = path_visited.clone();
                next_visited.insert(next_addr.clone());
                let mut next_path = path.clone();
                next_path.push(Arc::clone(t));
                queue.push((next_path, next_addr, propagated, next_visited));
            }
        }

        sinks.sort_by_key(|s| std::cmp::Reverse(s.risk_score()));
        sinks.dedup_by(|a, b| a.address() == b.address());

        let paths_found = paths.len();
        let depth_reached = paths.iter().map(|p| p.depth()).max().unwrap_or(0);

        tracing::info!(
            addresses_visited = addresses_visited.len(),
            transfers_evaluated,
            paths_found,
            sinks = sinks.len(),
            depth_reached,
            truncated,
            "trace complete"
        );

        Ok(TraceResult::new(
            req,
            paths,
            sinks,
            TraceStats::new(
                addresses_visited.len(),
                transfers_evaluated,
                paths_found,
                depth_reached,
                truncated,
            ),
        ))
    }

    #[tracing::instrument(skip(self, addr), fields(address = %crate::addr_hex(addr)))]
    async fn score(&self, addr: &Address) -> DomainResult<RiskReport> {
        tracing::info!(address = %crate::addr_hex(addr), "score started");
        let mut signals: Vec<RiskSignal> = Vec::new();

        if let Some(entity) = self.entities.find_by_address(addr).await? {
            tracing::debug!(
                address = %crate::addr_hex(addr),
                category = ?entity.category(),
                "direct entity label found"
            );
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
            .trace(TraceRequest::new(
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
            .trace(TraceRequest::new(
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
            address = %crate::addr_hex(addr),
            score = report.overall_score().value(),
            signals = report.signals().len(),
            is_high_risk = report.is_high_risk(),
            "score complete"
        );
        Ok(report)
    }

    #[tracing::instrument(skip(self, addr), fields(address = %crate::addr_hex(addr)))]
    async fn check_sanctions(&self, addr: &Address) -> DomainResult<SanctionsCheckResult> {
        tracing::debug!(address = %crate::addr_hex(addr), "sanctions check");
        let entity = self.entities.find_by_address(addr).await?;

        let (is_sanctioned, sanction_list, label) = match entity {
            Some(e) => {
                let list: Option<SanctionList> =
                    if let EntityCategory::Sanctioned { sanction_list } = e.category() {
                        Some(sanction_list.clone())
                    } else {
                        None
                    };
                let label = e.label().map(|l| l.name().to_string());
                (list.is_some(), list, label)
            }
            None => (false, None, None),
        };

        tracing::debug!(
            address = %crate::addr_hex(addr),
            is_sanctioned,
            "sanctions check result"
        );
        Ok(SanctionsCheckResult::new(
            addr.clone(),
            is_sanctioned,
            sanction_list,
            label,
        ))
    }

    async fn check_sanctions_batch(
        &self,
        addrs: &[Address],
    ) -> DomainResult<Vec<SanctionsCheckResult>> {
        let mut results = Vec::with_capacity(addrs.len());
        for addr in addrs {
            results.push(self.check_sanctions(addr).await?);
        }
        Ok(results)
    }

    async fn deposit_reuse_cluster(
        &self,
        deposit_addr: &Address,
    ) -> DomainResult<Option<ClusterEvidence>> {
        let incoming = self.transfers.find_incoming(deposit_addr, None).await?;

        if incoming.len() < 3 {
            return Ok(None);
        }

        let senders: Vec<Address> = {
            let mut v: Vec<Address> = incoming.into_iter().map(|t| t.from().clone()).collect();
            v.sort_by(|a, b| a.bytes().cmp(b.bytes()));
            v.dedup();
            v
        };

        if senders.len() < 2 {
            return Ok(None);
        }

        Ok(Some(ClusterEvidence::new(
            senders,
            ClusteringHeuristic::DepositAddressReuse,
            Confidence::MEDIUM,
            Some(format!(
                "All senders route to deposit address {}",
                deposit_addr
            )),
        )))
    }

    async fn detect_peeling_chain(
        &self,
        addr: &Address,
    ) -> DomainResult<Option<ClusterEvidence>> {
        let incoming = self.transfers.find_incoming(addr, None).await?;
        let outgoing = self.transfers.find_outgoing(addr, None).await?;

        if incoming.is_empty() || outgoing.is_empty() {
            return Ok(None);
        }

        let in_sum = incoming.iter().fold(None::<Amount>, |acc, t| {
            Some(acc.map(|a| a + t.amount()).unwrap_or(t.amount()))
        });
        let out_sum = outgoing.iter().fold(None::<Amount>, |acc, t| {
            Some(acc.map(|a| a + t.amount()).unwrap_or(t.amount()))
        });

        let (Some(in_total), Some(out_total)) = (in_sum, out_sum) else {
            return Ok(None);
        };

        let retained = if out_total.raw() <= in_total.raw() {
            in_total - out_total
        } else {
            return Ok(None);
        };

        let retained_ratio = retained.ratio_of(&in_total);

        if retained_ratio > Ratio::from_percent(5) {
            return Ok(None);
        }

        let chain_addrs: Vec<Address> = outgoing.into_iter().map(|t| t.to().clone()).collect();

        Ok(Some(ClusterEvidence::new(
            chain_addrs,
            ClusteringHeuristic::PeelingChain,
            Confidence::HIGH,
            Some(format!(
                "Address retains only {:.1}% of inflow",
                retained_ratio.as_f64() * 100.0
            )),
        )))
    }

    async fn save_cluster(
        &self,
        evidence: ClusterEvidence,
        category: EntityCategory,
    ) -> DomainResult<()> {
        let mut entity = Entity::new(category, RiskScore::MEDIUM);
        for addr in evidence.addresses() {
            entity.add_address(addr.clone());
        }
        self.entities.save(&entity).await
    }
}

impl<R, E> RiskService<R, E>
where
    R: TransferRepository,
    E: EntityRepository,
{
    async fn fetch_cached(
        &self,
        out_cache: &mut HashMap<Address, Arc<Vec<Arc<Transfer>>>>,
        in_cache: &mut HashMap<Address, Arc<Vec<Arc<Transfer>>>>,
        addr: &Address,
        direction: TraceDirection,
        force_incoming: bool,
    ) -> DomainResult<Arc<Vec<Arc<Transfer>>>> {
        if force_incoming {
            if let Some(v) = in_cache.get(addr) {
                return Ok(Arc::clone(v));
            }
            let raw = self.transfers.find_incoming(addr, None).await?;
            let arcs = Arc::new(raw.into_iter().map(Arc::new).collect::<Vec<_>>());
            in_cache.insert(addr.clone(), Arc::clone(&arcs));
            return Ok(arcs);
        }

        match direction {
            TraceDirection::Forward => {
                if let Some(v) = out_cache.get(addr) {
                    return Ok(Arc::clone(v));
                }
                let raw = self.transfers.find_outgoing(addr, None).await?;
                let arcs = Arc::new(raw.into_iter().map(Arc::new).collect::<Vec<_>>());
                out_cache.insert(addr.clone(), Arc::clone(&arcs));
                Ok(arcs)
            }
            TraceDirection::Backward => {
                if let Some(v) = in_cache.get(addr) {
                    return Ok(Arc::clone(v));
                }
                let raw = self.transfers.find_incoming(addr, None).await?;
                let arcs = Arc::new(raw.into_iter().map(Arc::new).collect::<Vec<_>>());
                in_cache.insert(addr.clone(), Arc::clone(&arcs));
                Ok(arcs)
            }
            TraceDirection::Both => {
                let out = if let Some(v) = out_cache.get(addr) {
                    Arc::clone(v)
                } else {
                    let raw = self.transfers.find_outgoing(addr, None).await?;
                    let arcs = Arc::new(raw.into_iter().map(Arc::new).collect::<Vec<_>>());
                    out_cache.insert(addr.clone(), Arc::clone(&arcs));
                    arcs
                };
                let inc = if let Some(v) = in_cache.get(addr) {
                    Arc::clone(v)
                } else {
                    let raw = self.transfers.find_incoming(addr, None).await?;
                    let arcs = Arc::new(raw.into_iter().map(Arc::new).collect::<Vec<_>>());
                    in_cache.insert(addr.clone(), Arc::clone(&arcs));
                    arcs
                };
                let mut combined = Vec::with_capacity(out.len() + inc.len());
                combined.extend(out.iter().map(Arc::clone));
                combined.extend(inc.iter().map(Arc::clone));
                Ok(Arc::new(combined))
            }
        }
    }

    async fn resolve_seeds(
        &self,
        origin: &TraceOrigin,
        direction: TraceDirection,
    ) -> DomainResult<Vec<Transfer>> {
        match origin {
            TraceOrigin::Address(addr) => match direction {
                TraceDirection::Backward => self.transfers.find_incoming(addr, None).await,
                TraceDirection::Forward | TraceDirection::Both => {
                    self.transfers.find_outgoing(addr, None).await
                }
            },
            TraceOrigin::Transaction { chain, hash } => {
                self.transfers.find_by_tx(*chain, hash).await
            }
            TraceOrigin::Transfer(id) => {
                let all = self.transfers.find_by_tx(id.chain(), id.tx_hash()).await?;
                Ok(all
                    .into_iter()
                    .filter(|t| t.id().index() == id.index())
                    .collect())
            }
        }
    }

    async fn classify_sink(
        &self,
        addr: &Address,
        tainted: Amount,
        total: Amount,
    ) -> DomainResult<Sink> {
        let ratio = tainted.ratio_of(&total);

        let kind = match self.entities.find_by_address(addr).await? {
            Some(entity) => match entity.category() {
                EntityCategory::Exchange => SinkKind::Exchange {
                    name: entity
                        .label()
                        .map(|l| l.name().to_string())
                        .unwrap_or_default(),
                    requires_subpoena: true,
                },
                EntityCategory::Bridge => SinkKind::Bridge {
                    destination_chain: None,
                },
                EntityCategory::Mixer => SinkKind::Mixer,
                EntityCategory::Sanctioned { .. } => SinkKind::Sanctioned,
                EntityCategory::Darknet => SinkKind::Darknet,
                _ => SinkKind::Unresolved,
            },
            None => SinkKind::Unresolved,
        };

        Ok(Sink::new(addr.clone(), kind, tainted, ratio))
    }
}

fn sink_label(kind: &SinkKind) -> &'static str {
    match kind {
        SinkKind::Exchange { .. } => "exchange",
        SinkKind::Bridge { .. } => "bridge",
        SinkKind::Mixer => "mixer",
        SinkKind::Sanctioned => "sanctioned",
        SinkKind::Darknet => "darknet",
        SinkKind::Unresolved => "unresolved",
    }
}
