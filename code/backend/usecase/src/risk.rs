use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

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
use moka::future::Cache;

#[derive(Debug, Clone)]
pub struct RiskCacheConfig {
    pub score_ttl: Duration,
    pub score_max_entries: u64,
    pub sanctions_ttl: Duration,
    pub sanctions_max_entries: u64,
}

impl Default for RiskCacheConfig {
    fn default() -> Self {
        Self {
            score_ttl: Duration::from_secs(300),
            score_max_entries: 10_000,
            sanctions_ttl: Duration::from_secs(900),
            sanctions_max_entries: 10_000,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HeuristicsConfig {
    pub min_fanout: usize,
    pub min_fanin: usize,
    pub fan_window: Duration,
    pub smurf_window: Duration,
    pub smurf_max_depth: u32,
}

impl Default for HeuristicsConfig {
    fn default() -> Self {
        Self {
            min_fanout: 5,
            min_fanin: 5,
            fan_window: Duration::from_secs(86_400),
            smurf_window: Duration::from_secs(86_400),
            smurf_max_depth: 2,
        }
    }
}

pub struct RiskService<R, E> {
    transfers: R,
    entities: E,
    score_cache: Cache<Address, RiskReport>,
    sanctions_cache: Cache<Address, SanctionsCheckResult>,
    heuristics: HeuristicsConfig,
}

impl<R, E> RiskService<R, E> {
    pub fn new(
        transfers: R,
        entities: E,
        cache: RiskCacheConfig,
        heuristics: HeuristicsConfig,
    ) -> Self {
        let score_cache = Cache::builder()
            .max_capacity(cache.score_max_entries)
            .time_to_live(cache.score_ttl)
            .build();
        let sanctions_cache = Cache::builder()
            .max_capacity(cache.sanctions_max_entries)
            .time_to_live(cache.sanctions_ttl)
            .build();
        Self {
            transfers,
            entities,
            score_cache,
            sanctions_cache,
            heuristics,
        }
    }
}

impl<R, E> RiskService<R, E>
where
    R: TransferRepository,
    E: EntityRepository,
{
    pub async fn score_batch(
        &self,
        addresses: &[Address],
    ) -> DomainResult<Vec<RiskReport>> {
        use futures::stream::{FuturesUnordered, StreamExt};
        let mut futs: FuturesUnordered<_> =
            addresses.iter().map(|addr| self.score(addr)).collect();
        let mut results = Vec::with_capacity(addresses.len());
        while let Some(res) = futs.next().await {
            results.push(res?);
        }
        Ok(results)
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
        if let Some(cached) = self.score_cache.get(addr).await {
            tracing::debug!(address = %crate::addr_hex(addr), "score cache hit");
            return Ok(cached);
        }
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
        self.score_cache.insert(addr.clone(), report.clone()).await;
        Ok(report)
    }

    #[tracing::instrument(skip(self, addr), fields(address = %crate::addr_hex(addr)))]
    async fn check_sanctions(&self, addr: &Address) -> DomainResult<SanctionsCheckResult> {
        if let Some(cached) = self.sanctions_cache.get(addr).await {
            tracing::debug!(address = %crate::addr_hex(addr), "sanctions cache hit");
            return Ok(cached);
        }
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
        let result = SanctionsCheckResult::new(
            addr.clone(),
            is_sanctioned,
            sanction_list,
            label,
        );
        self.sanctions_cache.insert(addr.clone(), result.clone()).await;
        Ok(result)
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

    async fn detect_fan_out(
        &self,
        addr: &Address,
    ) -> DomainResult<Option<ClusterEvidence>> {
        let outgoing = self.transfers.find_outgoing(addr, None).await?;
        let cfg = self.heuristics;
        Ok(burst_window_evidence(
            &outgoing,
            |t| t.to().clone(),
            cfg.min_fanout,
            cfg.fan_window,
            ClusteringHeuristic::FanOut,
            |n, window_secs| format!(
                "{n} unique receivers within a {window_secs}s window from {addr}"
            ),
        ))
    }

    async fn detect_fan_in(
        &self,
        addr: &Address,
    ) -> DomainResult<Option<ClusterEvidence>> {
        let incoming = self.transfers.find_incoming(addr, None).await?;
        let cfg = self.heuristics;
        Ok(burst_window_evidence(
            &incoming,
            |t| t.from().clone(),
            cfg.min_fanin,
            cfg.fan_window,
            ClusteringHeuristic::FanIn,
            |n, window_secs| format!(
                "{n} unique senders within a {window_secs}s window into {addr}"
            ),
        ))
    }

    async fn detect_smurfing_cycle(
        &self,
        addr: &Address,
    ) -> DomainResult<Option<ClusterEvidence>> {
        let cfg = self.heuristics;
        let outgoing = self.transfers.find_outgoing(addr, None).await?;

        let receivers: Vec<(Address, chrono::DateTime<chrono::Utc>)> = {
            let mut v: Vec<(Address, chrono::DateTime<chrono::Utc>)> =
                outgoing.iter().map(|t| (t.to().clone(), t.timestamp())).collect();
            v.sort_by(|a, b| a.0.bytes().cmp(b.0.bytes()).then(a.1.cmp(&b.1)));
            v.dedup_by(|a, b| a.0 == b.0);
            v.retain(|(r, _)| r != addr);
            v
        };

        if receivers.len() < cfg.min_fanout {
            return Ok(None);
        }

        let window_chrono = chrono::Duration::from_std(cfg.smurf_window)
            .unwrap_or_else(|_| chrono::Duration::seconds(86_400));

        let mut outgoing_cache: HashMap<Address, Vec<Transfer>> = HashMap::new();
        let mut downstream: HashMap<Address, HashSet<Address>> = HashMap::new();

        for (r, r_ts) in receivers.iter() {
            let reached =
                bfs_downstream(&self.transfers, &mut outgoing_cache, r, *r_ts, window_chrono, cfg.smurf_max_depth)
                    .await?;
            for y in reached {
                if &y == addr || &y == r {
                    continue;
                }
                downstream.entry(y).or_default().insert(r.clone());
            }
        }

        let mut hits: Vec<(Address, HashSet<Address>)> = downstream
            .into_iter()
            .filter(|(_, v)| v.len() >= cfg.min_fanin)
            .collect();
        if hits.is_empty() {
            return Ok(None);
        }

        hits.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.bytes().cmp(b.0.bytes())));
        let (y, via_set) = hits.into_iter().next().unwrap();
        let mut via: Vec<Address> = via_set.into_iter().collect();
        via.sort_by(|a, b| a.bytes().cmp(b.bytes()));
        let via_count = via.len();

        let mut addresses: Vec<Address> = Vec::with_capacity(2 + via.len());
        addresses.push(addr.clone());
        addresses.extend(via);
        addresses.push(y.clone());

        Ok(Some(ClusterEvidence::new(
            addresses,
            ClusteringHeuristic::SmurfingCycle,
            Confidence::MEDIUM,
            Some(format!(
                "{via_count} intermediaries route from {addr} to cash-out {y} within {}s (depth ≤ {})",
                cfg.smurf_window.as_secs(),
                cfg.smurf_max_depth
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

/// Slide a time-window over `transfers` (sorted by timestamp) and return
/// evidence if any window contains at least `min_unique` distinct counterparties
/// produced by `counterparty`. Returns the maximal-coverage window.
fn burst_window_evidence<F, N>(
    transfers: &[Transfer],
    counterparty: F,
    min_unique: usize,
    window: Duration,
    heuristic: ClusteringHeuristic,
    note: N,
) -> Option<ClusterEvidence>
where
    F: Fn(&Transfer) -> Address,
    N: Fn(usize, u64) -> String,
{
    if transfers.len() < min_unique {
        return None;
    }

    let mut sorted: Vec<&Transfer> = transfers.iter().collect();
    sorted.sort_by_key(|t| t.timestamp());

    let window_chrono = chrono::Duration::from_std(window).ok()?;

    let mut best: Option<Vec<Address>> = None;
    let mut left = 0usize;

    for right in 0..sorted.len() {
        let right_ts = sorted[right].timestamp();
        while left < right && right_ts - sorted[left].timestamp() > window_chrono {
            left += 1;
        }

        let mut seen: HashSet<Address> = HashSet::new();
        let mut uniq: Vec<Address> = Vec::new();
        for t in &sorted[left..=right] {
            let cp = counterparty(t);
            if seen.insert(cp.clone()) {
                uniq.push(cp);
            }
        }

        if uniq.len() >= min_unique
            && best.as_ref().map(|b| b.len() < uniq.len()).unwrap_or(true)
        {
            best = Some(uniq);
        }
    }

    let addresses = best?;
    let n = addresses.len();
    Some(ClusterEvidence::new(
        addresses,
        heuristic,
        Confidence::MEDIUM,
        Some(note(n, window.as_secs())),
    ))
}

/// BFS downstream from `start` over outgoing transfers, expanding only edges
/// whose timestamp is in `[anchor_ts, anchor_ts + window]`. Returns the set of
/// addresses reachable within `max_depth` hops, not including `start`.
/// Outgoing lookups are memoized into `cache` across calls.
async fn bfs_downstream<R: TransferRepository + ?Sized>(
    repo: &R,
    cache: &mut HashMap<Address, Vec<Transfer>>,
    start: &Address,
    anchor_ts: chrono::DateTime<chrono::Utc>,
    window: chrono::Duration,
    max_depth: u32,
) -> DomainResult<HashSet<Address>> {
    let mut reached: HashSet<Address> = HashSet::new();
    if max_depth == 0 {
        return Ok(reached);
    }

    let mut queue: std::collections::VecDeque<(Address, u32)> =
        std::collections::VecDeque::new();
    queue.push_back((start.clone(), 0));
    let mut visited: HashSet<Address> = HashSet::new();
    visited.insert(start.clone());

    let upper = anchor_ts + window;

    while let Some((addr, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }
        if !cache.contains_key(&addr) {
            let outs = repo.find_outgoing(&addr, None).await?;
            cache.insert(addr.clone(), outs);
        }
        let outs = cache.get(&addr).expect("just inserted");
        for t in outs {
            let ts = t.timestamp();
            if ts < anchor_ts || ts > upper {
                continue;
            }
            let next = t.to().clone();
            if visited.insert(next.clone()) {
                reached.insert(next.clone());
                queue.push_back((next, depth + 1));
            }
        }
    }

    Ok(reached)
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

#[cfg(test)]
mod heuristics_tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use domain::asset::AssetId;
    use domain::chain::ChainId;
    use domain::entity::{Entity, EntityId};
    use domain::primitives::{Amount, BlockRef, TxRef, U256};
    use domain::transfer::{Finality, TransferId, TransferKind};
    use std::sync::Mutex;

    fn addr(seed: u8) -> Address {
        let mut bytes = vec![0u8; 20];
        bytes[19] = seed;
        Address::new(ChainId::ETH, bytes)
    }

    fn make_transfer(
        from: &Address,
        to: &Address,
        ts_secs: i64,
        idx: u32,
    ) -> Transfer {
        let chain = ChainId::ETH;
        let mut hash = [0u8; 32];
        hash[28..32].copy_from_slice(&idx.to_be_bytes());
        Transfer::new(
            TransferId::new(chain, hash, idx),
            chain,
            TxRef::new(chain, hash),
            from.clone(),
            to.clone(),
            AssetId::native(chain),
            Amount::new(U256::from(1u64), 18),
            BlockRef::new(chain, 100 + idx as u64, [0u8; 32]),
            Utc.timestamp_opt(ts_secs, 0).unwrap(),
            TransferKind::Native,
            Finality::Confirmed,
        )
    }

    #[derive(Default)]
    struct MockTransfers {
        outgoing: Mutex<HashMap<Address, Vec<Transfer>>>,
        incoming: Mutex<HashMap<Address, Vec<Transfer>>>,
    }

    impl MockTransfers {
        fn push(&self, t: Transfer) {
            self.outgoing
                .lock()
                .unwrap()
                .entry(t.from().clone())
                .or_default()
                .push(t.clone());
            self.incoming
                .lock()
                .unwrap()
                .entry(t.to().clone())
                .or_default()
                .push(t);
        }
    }

    #[async_trait]
    impl TransferRepository for MockTransfers {
        async fn save(&self, _transfers: &[Transfer]) -> DomainResult<()> {
            Ok(())
        }
        async fn find_by_address(
            &self,
            _addr: &Address,
            _range: Option<domain::ports::BlockRange>,
            _after: Option<domain::ports::TransferCursor>,
            _limit: usize,
        ) -> DomainResult<Vec<Transfer>> {
            Ok(Vec::new())
        }
        async fn find_by_tx(
            &self,
            _chain: ChainId,
            _tx_hash: &[u8; 32],
        ) -> DomainResult<Vec<Transfer>> {
            Ok(Vec::new())
        }
        async fn find_outgoing(
            &self,
            addr: &Address,
            _after: Option<chrono::DateTime<Utc>>,
        ) -> DomainResult<Vec<Transfer>> {
            Ok(self.outgoing.lock().unwrap().get(addr).cloned().unwrap_or_default())
        }
        async fn find_incoming(
            &self,
            addr: &Address,
            _after: Option<chrono::DateTime<Utc>>,
        ) -> DomainResult<Vec<Transfer>> {
            Ok(self.incoming.lock().unwrap().get(addr).cloned().unwrap_or_default())
        }
        async fn min_block_height(&self, _addr: &Address) -> DomainResult<Option<u64>> {
            Ok(None)
        }
        async fn max_block_height(&self, _addr: &Address) -> DomainResult<Option<u64>> {
            Ok(None)
        }
        async fn delete_in_range(
            &self,
            _addr: &Address,
            _from_block: u64,
            _to_block: u64,
        ) -> DomainResult<u64> {
            Ok(0)
        }
    }

    #[derive(Default)]
    struct NopEntities;

    #[async_trait]
    impl EntityRepository for NopEntities {
        async fn find_by_id(&self, _id: &EntityId) -> DomainResult<Option<Entity>> {
            Ok(None)
        }
        async fn find_by_address(&self, _addr: &Address) -> DomainResult<Option<Entity>> {
            Ok(None)
        }
        async fn save(&self, _entity: &Entity) -> DomainResult<()> {
            Ok(())
        }
        async fn list_sanctioned(&self) -> DomainResult<Vec<Entity>> {
            Ok(Vec::new())
        }
    }

    fn service(
        transfers: MockTransfers,
        heuristics: HeuristicsConfig,
    ) -> RiskService<MockTransfers, NopEntities> {
        RiskService::new(transfers, NopEntities, RiskCacheConfig::default(), heuristics)
    }

    #[tokio::test]
    async fn fan_out_triggers_at_threshold() {
        let repo = MockTransfers::default();
        let src = addr(1);
        let t0 = 1_000_000;
        for i in 0..5u32 {
            repo.push(make_transfer(&src, &addr(10 + i as u8), t0 + i as i64, i));
        }
        let svc = service(
            repo,
            HeuristicsConfig {
                min_fanout: 5,
                fan_window: Duration::from_secs(60),
                ..HeuristicsConfig::default()
            },
        );
        let ev = svc.detect_fan_out(&src).await.unwrap().expect("evidence");
        assert_eq!(ev.addresses().len(), 5);
        assert!(matches!(ev.heuristic(), ClusteringHeuristic::FanOut));
    }

    #[tokio::test]
    async fn fan_out_silent_below_threshold() {
        let repo = MockTransfers::default();
        let src = addr(1);
        for i in 0..4u32 {
            repo.push(make_transfer(&src, &addr(10 + i as u8), 1_000_000 + i as i64, i));
        }
        let svc = service(
            repo,
            HeuristicsConfig {
                min_fanout: 5,
                fan_window: Duration::from_secs(60),
                ..HeuristicsConfig::default()
            },
        );
        assert!(svc.detect_fan_out(&src).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn fan_out_silent_when_spread_outside_window() {
        let repo = MockTransfers::default();
        let src = addr(1);
        // 5 distinct receivers, but spaced 1 day apart against a 60s window.
        for i in 0..5u32 {
            repo.push(make_transfer(&src, &addr(10 + i as u8), 1_000_000 + (i as i64) * 86_400, i));
        }
        let svc = service(
            repo,
            HeuristicsConfig {
                min_fanout: 5,
                fan_window: Duration::from_secs(60),
                ..HeuristicsConfig::default()
            },
        );
        assert!(svc.detect_fan_out(&src).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn fan_in_triggers_at_threshold() {
        let repo = MockTransfers::default();
        let dst = addr(1);
        for i in 0..5u32 {
            repo.push(make_transfer(&addr(20 + i as u8), &dst, 1_000_000 + i as i64, i));
        }
        let svc = service(
            repo,
            HeuristicsConfig {
                min_fanin: 5,
                fan_window: Duration::from_secs(60),
                ..HeuristicsConfig::default()
            },
        );
        let ev = svc.detect_fan_in(&dst).await.unwrap().expect("evidence");
        assert_eq!(ev.addresses().len(), 5);
        assert!(matches!(ev.heuristic(), ClusteringHeuristic::FanIn));
    }

    #[tokio::test]
    async fn smurfing_triggers_when_all_receivers_route_to_one_y() {
        let repo = MockTransfers::default();
        let distributor = addr(1);
        let cash_out = addr(99);
        for i in 0..5u32 {
            let r = addr(10 + i as u8);
            // distributor -> r at t0
            repo.push(make_transfer(&distributor, &r, 1_000_000 + i as i64, i));
            // r -> cash_out within window
            repo.push(make_transfer(&r, &cash_out, 1_001_000 + i as i64, 100 + i));
        }
        let svc = service(
            repo,
            HeuristicsConfig {
                min_fanout: 5,
                min_fanin: 5,
                fan_window: Duration::from_secs(60),
                smurf_window: Duration::from_secs(3_600),
                smurf_max_depth: 2,
            },
        );
        let ev = svc.detect_smurfing_cycle(&distributor).await.unwrap().expect("evidence");
        assert!(matches!(ev.heuristic(), ClusteringHeuristic::SmurfingCycle));
        assert_eq!(ev.addresses().first(), Some(&distributor));
        assert_eq!(ev.addresses().last(), Some(&cash_out));
        assert_eq!(ev.addresses().len(), 2 + 5);
    }

    #[tokio::test]
    async fn smurfing_silent_when_receivers_diverge() {
        let repo = MockTransfers::default();
        let distributor = addr(1);
        for i in 0..5u32 {
            let r = addr(10 + i as u8);
            let y = addr(90 + i as u8);
            repo.push(make_transfer(&distributor, &r, 1_000_000 + i as i64, i));
            repo.push(make_transfer(&r, &y, 1_001_000 + i as i64, 100 + i));
        }
        let svc = service(
            repo,
            HeuristicsConfig {
                min_fanout: 5,
                min_fanin: 5,
                fan_window: Duration::from_secs(60),
                smurf_window: Duration::from_secs(3_600),
                smurf_max_depth: 2,
            },
        );
        assert!(svc.detect_smurfing_cycle(&distributor).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn smurfing_finds_cash_out_two_hops_away() {
        let repo = MockTransfers::default();
        let distributor = addr(1);
        let cash_out = addr(99);
        for i in 0..5u32 {
            let r = addr(10 + i as u8);
            let m = addr(50 + i as u8);
            // distributor -> r -> m -> cash_out
            repo.push(make_transfer(&distributor, &r, 1_000_000 + i as i64, i));
            repo.push(make_transfer(&r, &m, 1_000_500 + i as i64, 100 + i));
            repo.push(make_transfer(&m, &cash_out, 1_001_000 + i as i64, 200 + i));
        }
        let svc = service(
            repo,
            HeuristicsConfig {
                min_fanout: 5,
                min_fanin: 5,
                fan_window: Duration::from_secs(60),
                smurf_window: Duration::from_secs(3_600),
                smurf_max_depth: 2,
            },
        );
        let ev = svc.detect_smurfing_cycle(&distributor).await.unwrap().expect("evidence");
        assert_eq!(ev.addresses().last(), Some(&cash_out));
    }
}
