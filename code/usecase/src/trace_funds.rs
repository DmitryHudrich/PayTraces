use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use domain::entity::EntityCategory;
use domain::error::DomainResult;
use domain::ports::{EntityRepository, TransferRepository};
use domain::primitives::{Address, Amount, Ratio};
use domain::trace::{
    FlowPath, Sink, SinkKind, TaintStrategy, TraceDirection, TraceOrigin, TraceRequest,
    TraceResult, TraceStats,
};
use domain::transfer::Transfer;

pub struct TraceFundsUseCase<R, E> {
    transfers: R,
    entities: E,
}

impl<R: TransferRepository, E: EntityRepository> TraceFundsUseCase<R, E> {
    pub fn new(transfers: R, entities: E) -> Self {
        Self { transfers, entities }
    }

    pub async fn execute(&self, req: TraceRequest) -> DomainResult<TraceResult> {
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

        // Per-trace caches: the same address can appear in many paths — fetch once, share via Arc.
        let mut out_cache: HashMap<Address, Arc<Vec<Arc<Transfer>>>> = HashMap::new();
        let mut in_cache: HashMap<Address, Arc<Vec<Arc<Transfer>>>> = HashMap::new();

        // Queue stores Arc<Transfer> hops — path.clone() copies only pointers, not Transfer data.
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

            tracing::trace!(address = %super::addr_hex(&addr), depth = path.len(), "trace visiting");

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
                // Clone Transfer data only at terminal paths, not during traversal.
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
                let mut next_path = path.clone(); // Vec<Arc> clone: pointer copies only
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

    // Fetches transfers for an address, using the per-trace cache to avoid redundant DB queries.
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
            TraceOrigin::Transaction(hash) => {
                self.transfers
                    .find_by_tx(domain::chain::ChainId::ETH, hash)
                    .await
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
