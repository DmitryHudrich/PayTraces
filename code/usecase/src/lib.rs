fn addr_hex(addr: &domain::primitives::Address) -> String {
    format!("0x{}", hex::encode(&addr.bytes))
}





pub mod ingest_address {
    use domain::error::DomainResult;
    use domain::ports::{BlockRange, ChainSource, TransferRepository};
    use domain::primitives::Address;

    use crate::addr_hex;

    pub struct IngestAddressUseCase<S, R> {
        source: S,
        repo: R,
    }

    impl<S: ChainSource, R: TransferRepository> IngestAddressUseCase<S, R> {
        pub fn new(source: S, repo: R) -> Self {
            Self { source, repo }
        }

        pub async fn execute(&self, addr: &Address, range: BlockRange) -> DomainResult<usize> {
            tracing::info!(
                address = %addr_hex(addr),
                from_block = range.from_height,
                to_block = range.to_height,
                "ingest started"
            );

            let transfers = self.source.transfers_for_address(addr, range).await?;
            let count = transfers.len();

            tracing::info!(address = %addr_hex(addr), count, "fetched transfers, saving");

            self.repo.save(&transfers).await?;

            tracing::info!(address = %addr_hex(addr), count, "ingest complete");
            Ok(count)
        }
    }
}





pub mod build_transfer_graph {
    use std::collections::{HashMap, HashSet, VecDeque};

    use domain::error::DomainResult;
    use domain::ports::{BlockRange, TransferRepository};
    use domain::primitives::Address;
    use domain::transfer::Transfer;

    pub struct BuildTransferGraphUseCase<R> {
        repo: R,
    }

    #[derive(Debug, Clone)]
    pub struct TransferGraph {
        pub nodes: HashSet<Address>,
        pub edges: Vec<Transfer>,
    }

    impl TransferGraph {
        pub fn neighbors_of(&self, addr: &Address) -> Vec<&Address> {
            let mut result = Vec::new();
            for t in &self.edges {
                if &t.from == addr {
                    result.push(&t.to);
                } else if &t.to == addr {
                    result.push(&t.from);
                }
            }
            result
        }

        pub fn outgoing(&self, addr: &Address) -> Vec<&Transfer> {
            self.edges.iter().filter(|t| &t.from == addr).collect()
        }

        pub fn incoming(&self, addr: &Address) -> Vec<&Transfer> {
            self.edges.iter().filter(|t| &t.to == addr).collect()
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub struct GraphRequest {
        pub range: Option<BlockRange>,
        pub max_depth: u32,
        pub max_nodes: usize,
    }

    impl Default for GraphRequest {
        fn default() -> Self {
            Self {
                range: None,
                max_depth: 3,
                max_nodes: 500,
            }
        }
    }

    impl<R: TransferRepository> BuildTransferGraphUseCase<R> {
        pub fn new(repo: R) -> Self {
            Self { repo }
        }

        pub async fn execute(
            &self,
            origin: &Address,
            req: GraphRequest,
        ) -> DomainResult<TransferGraph> {
            tracing::info!(
                origin = %super::addr_hex(origin),
                max_depth = req.max_depth,
                max_nodes = req.max_nodes,
                "graph build started"
            );

            let mut nodes: HashSet<Address> = HashSet::new();
            let mut edges: Vec<Transfer> = Vec::new();
            let mut visited: HashSet<Address> = HashSet::new();
            let mut queue: VecDeque<(Address, u32)> = VecDeque::new();

            queue.push_back((origin.clone(), 0));
            visited.insert(origin.clone());
            nodes.insert(origin.clone());

            while let Some((addr, depth)) = queue.pop_front() {
                if depth >= req.max_depth || nodes.len() >= req.max_nodes {
                    tracing::debug!(
                        depth, nodes = nodes.len(), max_depth = req.max_depth, max_nodes = req.max_nodes,
                        "graph BFS limit reached"
                    );
                    break;
                }

                let transfers = self.repo.find_by_address(&addr, req.range).await?;

                tracing::debug!(
                    address = %super::addr_hex(&addr), depth,
                    transfers = transfers.len(), nodes = nodes.len(),
                    "graph BFS step"
                );

                for t in transfers {
                    let counterparty = if &t.from == &addr { t.to.clone() } else { t.from.clone() };

                    nodes.insert(t.from.clone());
                    nodes.insert(t.to.clone());
                    edges.push(t);

                    if !visited.contains(&counterparty) && nodes.len() < req.max_nodes {
                        visited.insert(counterparty.clone());
                        queue.push_back((counterparty, depth + 1));
                    }
                }
            }

            edges.sort_by(|a, b| a.id.tx_hash.cmp(&b.id.tx_hash).then(a.id.index.cmp(&b.id.index)));
            edges.dedup_by(|a, b| a.id == b.id);

            tracing::info!(
                origin = %super::addr_hex(origin),
                nodes = nodes.len(), edges = edges.len(),
                "graph build complete"
            );

            Ok(TransferGraph { nodes, edges })
        }
    }
}





pub mod trace_funds {
    use std::collections::{HashMap, HashSet};

    use domain::entity::EntityCategory;
    use domain::error::DomainResult;
    use domain::ports::{EntityRepository, TransferRepository};
    use domain::primitives::{Address, Amount, Ratio};
    use domain::trace::{
        FlowPath, Sink, SinkKind, TaintStrategy, TraceDirection, TraceLimits, TraceOrigin,
        TraceRequest, TraceResult, TraceStats,
    };
    use domain::transfer::{Transfer, TransferId};

    pub struct TraceFundsUseCase<R, E> {
        transfers: R,
        entities: E,
    }

    impl<R: TransferRepository, E: EntityRepository> TraceFundsUseCase<R, E> {
        pub fn new(transfers: R, entities: E) -> Self {
            Self {
                transfers,
                entities,
            }
        }

        pub async fn execute(&self, req: TraceRequest) -> DomainResult<TraceResult> {
            tracing::info!(
                direction = ?req.direction,
                strategy = ?req.strategy,
                max_hops = req.limits.max_hops,
                max_addresses = req.limits.max_addresses,
                "trace started"
            );

            let seeds = self.resolve_seeds(&req.origin, req.direction).await?;
            tracing::debug!(seeds = seeds.len(), "trace seeds resolved");

            let mut paths: Vec<FlowPath> = Vec::new();
            let mut sinks: Vec<Sink> = Vec::new();
            
            let mut addresses_visited: HashSet<Address> = HashSet::new();
            let mut transfers_evaluated: usize = 0;
            let mut truncated = false;

            
            let mut queue: Vec<(Vec<Transfer>, Address, Amount, HashSet<Address>)> = seeds
                .into_iter()
                .map(|t| {
                    let (addr, origin_side) = match req.direction {
                        TraceDirection::Forward | TraceDirection::Both => {
                            (t.to.clone(), t.from.clone())
                        }
                        TraceDirection::Backward => (t.from.clone(), t.to.clone()),
                    };
                    let amount = t.amount;
                    let mut path_visited = HashSet::new();
                    path_visited.insert(addr.clone());
                    path_visited.insert(origin_side);
                    (vec![t], addr, amount, path_visited)
                })
                .collect();

            while let Some((path, addr, tainted, path_visited)) = queue.pop() {
                if path.len() as u32 > req.limits.max_hops {
                    tracing::debug!(hops = path.len(), max_hops = req.limits.max_hops, "path truncated: max hops");
                    truncated = true;
                    continue;
                }
                if addresses_visited.len() >= req.limits.max_addresses {
                    tracing::warn!(addresses = addresses_visited.len(), "trace truncated: max_addresses reached");
                    truncated = true;
                    break;
                }
                if paths.len() >= req.limits.max_paths {
                    tracing::warn!(paths = paths.len(), "trace truncated: max_paths reached");
                    truncated = true;
                    break;
                }

                tracing::trace!(
                    address = %super::addr_hex(&addr),
                    depth = path.len(),
                    "trace visiting"
                );

                addresses_visited.insert(addr.clone());

                let next_transfers: Vec<Transfer> = match req.direction {
                    TraceDirection::Forward => self.transfers.find_outgoing(&addr, None).await?,
                    TraceDirection::Backward => self.transfers.find_incoming(&addr, None).await?,
                    TraceDirection::Both => {
                        let mut v = self.transfers.find_outgoing(&addr, None).await?;
                        v.extend(self.transfers.find_incoming(&addr, None).await?);
                        v
                    }
                };

                transfers_evaluated += next_transfers.len();

                
                
                
                let is_haircut = matches!(
                    req.strategy,
                    TaintStrategy::Haircut | TaintStrategy::Fifo | TaintStrategy::Lifo
                );
                let need_total_in = is_haircut || next_transfers.is_empty();

                let total_in: Amount = if matches!(req.direction, TraceDirection::Backward) {
                    next_transfers
                        .iter()
                        .filter(|t| t.amount.decimals == tainted.decimals)
                        .fold(Amount::zero(tainted.decimals), |acc, t| acc + t.amount)
                } else if need_total_in {
                    let inc = self.transfers.find_incoming(&addr, None).await?;
                    inc.iter()
                        .filter(|t| t.amount.decimals == tainted.decimals)
                        .fold(Amount::zero(tainted.decimals), |acc, t| acc + t.amount)
                } else {
                    tainted
                };

                let taint_ratio: Ratio = if is_haircut {
                    if total_in.is_zero() { Ratio::ONE } else { tainted.ratio_of(&total_in) }
                } else {
                    Ratio::ONE
                };

                if next_transfers.is_empty() {
                    let sink = self.classify_sink(&addr, tainted, total_in.max(tainted)).await?;
                    sinks.push(sink);
                    paths.push(FlowPath {
                        depth: path.len() as u32,
                        taint_ratio,
                        tainted_amount: tainted,
                        hops: path,
                    });
                    continue;
                }

                for t in next_transfers {
                    if !req.include_unconfirmed && !t.is_confirmed() {
                        continue;
                    }

                    let propagated = match req.strategy {
                        TaintStrategy::Poison => t.amount,
                        _ => taint_ratio.apply_to(t.amount),
                    };

                    if let Some(min_ratio) = req.limits.min_amount_ratio {
                        if propagated.ratio_of(&t.amount) < min_ratio {
                            continue;
                        }
                    }

                    let next_addr = match req.direction {
                        TraceDirection::Forward | TraceDirection::Both => t.to.clone(),
                        TraceDirection::Backward => t.from.clone(),
                    };

                    if path_visited.contains(&next_addr) {
                        continue;
                    }

                    let mut next_visited = path_visited.clone();
                    next_visited.insert(next_addr.clone());
                    let mut next_path = path.clone();
                    next_path.push(t);
                    queue.push((next_path, next_addr, propagated, next_visited));
                }
            }

            sinks.sort_by_key(|s| std::cmp::Reverse(s.risk_score()));
            sinks.dedup_by(|a, b| a.address == b.address);

            tracing::info!(
                addresses_visited = addresses_visited.len(),
                transfers_evaluated,
                paths_found = paths.len(),
                sinks = sinks.len(),
                depth_reached = paths.iter().map(|p| p.depth).max().unwrap_or(0),
                truncated,
                "trace complete"
            );

            Ok(TraceResult {
                request: req,
                terminal_sinks: sinks,
                stats: TraceStats {
                    addresses_visited: addresses_visited.len(),
                    transfers_evaluated,
                    paths_found: paths.len(),
                    depth_reached: paths.iter().map(|p| p.depth).max().unwrap_or(0),
                    truncated,
                },
                paths,
            })
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
                    let all = self.transfers.find_by_tx(id.chain, &id.tx_hash).await?;
                    Ok(all.into_iter().filter(|t| t.id.index == id.index).collect())
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
                Some(entity) => match &entity.category {
                    EntityCategory::Exchange => SinkKind::Exchange {
                        name: entity
                            .label
                            .as_ref()
                            .map(|l| l.name.clone())
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

            Ok(Sink {
                address: addr.clone(),
                kind,
                tainted_amount: tainted,
                taint_ratio: ratio,
            })
        }
    }
}





pub mod score_address {
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
                tracing::debug!(address = %super::addr_hex(addr), category = ?entity.category, "direct entity label found");
                let (severity, kind) = match &entity.category {
                    EntityCategory::Sanctioned { .. } => {
                        (RiskScore::CRITICAL, RiskSignalKind::SanctionedCounterparty)
                    }
                    EntityCategory::Mixer => (RiskScore::HIGH, RiskSignalKind::MixerInteraction),
                    EntityCategory::Darknet => (RiskScore::CRITICAL, RiskSignalKind::DarknetMarket),
                    EntityCategory::Scam => (RiskScore::HIGH, RiskSignalKind::DirectExposure),
                    _ => (RiskScore::LOW, RiskSignalKind::DirectExposure),
                };
                signals.push(RiskSignal {
                    kind,
                    severity,
                    description: format!(
                        "Address is labelled: {}",
                        entity
                            .label
                            .as_ref()
                            .map(|l| l.name.as_str())
                            .unwrap_or("unknown")
                    ),
                    evidence: RiskEvidence::EntityCategory(entity.category.clone()),
                });
            }

            
            let backward = self
                .trace
                .execute(TraceRequest {
                    origin: TraceOrigin::Address(addr.clone()),
                    direction: TraceDirection::Backward,
                    strategy: TaintStrategy::Haircut,
                    limits: TraceLimits {
                        max_hops: 5,
                        max_addresses: 200,
                        max_paths: 100,
                        min_amount_ratio: Some(Ratio::from_percent(5)),
                    },
                    include_unconfirmed: false,
                })
                .await?;

            for sink in &backward.terminal_sinks {
                if sink.risk_score() >= RiskScore::HIGH.value() {
                    let hops = backward
                        .paths
                        .iter()
                        .filter(|p| p.destination() == Some(&sink.address))
                        .map(|p| p.depth)
                        .min()
                        .unwrap_or(0);

                    let kind = if hops == 0 {
                        RiskSignalKind::DirectExposure
                    } else {
                        RiskSignalKind::IndirectExposure { hops }
                    };

                    signals.push(RiskSignal {
                        kind,
                        severity: RiskScore::new(sink.risk_score()),
                        description: format!(
                            "Funds traceable to high-risk sink ({})",
                            sink_label(&sink.kind)
                        ),
                        evidence: RiskEvidence::SinkExposure(vec![sink.clone()]),
                    });
                }
            }

            
            let forward = self
                .trace
                .execute(TraceRequest {
                    origin: TraceOrigin::Address(addr.clone()),
                    direction: TraceDirection::Forward,
                    strategy: TaintStrategy::Haircut,
                    limits: TraceLimits {
                        max_hops: 5,
                        max_addresses: 200,
                        max_paths: 100,
                        min_amount_ratio: Some(Ratio::from_percent(5)),
                    },
                    include_unconfirmed: false,
                })
                .await?;

            for sink in &forward.terminal_sinks {
                if sink.risk_score() >= RiskScore::HIGH.value() {
                    signals.push(RiskSignal {
                        kind: RiskSignalKind::DirectExposure,
                        severity: RiskScore::new(sink.risk_score()),
                        description: format!(
                            "Funds sent to high-risk destination ({})",
                            sink_label(&sink.kind)
                        ),
                        evidence: RiskEvidence::SinkExposure(vec![sink.clone()]),
                    });
                }
            }

            let report = RiskReport::new(addr.clone(), signals);
            tracing::info!(
                address = %super::addr_hex(addr),
                score = report.overall_score.value(),
                signals = report.signals.len(),
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
}





pub mod check_sanctions {
    use domain::entity::{EntityCategory, SanctionList};
    use domain::error::DomainResult;
    use domain::ports::EntityRepository;
    use domain::primitives::Address;

    #[derive(Debug, Clone)]
    pub struct SanctionsCheckResult {
        pub address: Address,
        pub is_sanctioned: bool,
        pub sanction_list: Option<SanctionList>,
        pub label: Option<String>,
    }

    pub struct CheckSanctionsUseCase<E> {
        entities: E,
    }

    impl<E: EntityRepository> CheckSanctionsUseCase<E> {
        pub fn new(entities: E) -> Self {
            Self { entities }
        }

        pub async fn execute(&self, addr: &Address) -> DomainResult<SanctionsCheckResult> {
            tracing::debug!(address = %super::addr_hex(addr), "sanctions check");
            let entity = self.entities.find_by_address(addr).await?;

            let (is_sanctioned, sanction_list, label) = match entity {
                Some(e) => {
                    let list = if let EntityCategory::Sanctioned { sanction_list } = &e.category {
                        Some(sanction_list.clone())
                    } else {
                        None
                    };
                    let label = e.label.as_ref().map(|l| l.name.clone());
                    (list.is_some(), list, label)
                }
                None => (false, None, None),
            };

            tracing::debug!(
                address = %super::addr_hex(addr),
                is_sanctioned,
                "sanctions check result"
            );
            Ok(SanctionsCheckResult {
                address: addr.clone(),
                is_sanctioned,
                sanction_list,
                label,
            })
        }

        pub async fn check_batch(
            &self,
            addrs: &[Address],
        ) -> DomainResult<Vec<SanctionsCheckResult>> {
            let mut results = Vec::with_capacity(addrs.len());
            for addr in addrs {
                results.push(self.execute(addr).await?);
            }
            Ok(results)
        }
    }
}





pub mod cluster_addresses {
    use domain::entity::{ClusterEvidence, ClusteringHeuristic, Entity, EntityCategory, RiskScore};
    use domain::error::DomainResult;
    use domain::ports::{EntityRepository, TransferRepository};
    use domain::primitives::{Address, Confidence};

    pub struct ClusterAddressesUseCase<R, E> {
        transfers: R,
        entities: E,
    }

    impl<R: TransferRepository, E: EntityRepository> ClusterAddressesUseCase<R, E> {
        pub fn new(transfers: R, entities: E) -> Self {
            Self {
                transfers,
                entities,
            }
        }

        
        
        pub async fn deposit_reuse_cluster(
            &self,
            deposit_addr: &Address,
        ) -> DomainResult<Option<ClusterEvidence>> {
            let incoming = self.transfers.find_incoming(deposit_addr, None).await?;

            if incoming.len() < 3 {
                return Ok(None);
            }

            let senders: Vec<Address> = {
                let mut v: Vec<Address> = incoming.into_iter().map(|t| t.from).collect();
                v.sort_by(|a, b| a.bytes.cmp(&b.bytes));
                v.dedup();
                v
            };

            if senders.len() < 2 {
                return Ok(None);
            }

            Ok(Some(ClusterEvidence {
                addresses: senders,
                heuristic: ClusteringHeuristic::DepositAddressReuse,
                confidence: Confidence::MEDIUM,
                notes: Some(format!(
                    "All senders route to deposit address {}",
                    deposit_addr
                )),
            }))
        }

        
        
        pub async fn detect_peeling_chain(
            &self,
            addr: &Address,
        ) -> DomainResult<Option<ClusterEvidence>> {
            let incoming = self.transfers.find_incoming(addr, None).await?;
            let outgoing = self.transfers.find_outgoing(addr, None).await?;

            if incoming.is_empty() || outgoing.is_empty() {
                return Ok(None);
            }

            let in_sum = incoming
                .iter()
                .fold(None::<domain::primitives::Amount>, |acc, t| {
                    Some(acc.map(|a| a + t.amount).unwrap_or(t.amount))
                });
            let out_sum = outgoing
                .iter()
                .fold(None::<domain::primitives::Amount>, |acc, t| {
                    Some(acc.map(|a| a + t.amount).unwrap_or(t.amount))
                });

            let (Some(in_total), Some(out_total)) = (in_sum, out_sum) else {
                return Ok(None);
            };

            let retained = if out_total.raw <= in_total.raw {
                in_total - out_total
            } else {
                return Ok(None);
            };

            let retained_ratio = retained.ratio_of(&in_total);

            if retained_ratio > domain::primitives::Ratio::from_percent(5) {
                return Ok(None);
            }

            let chain_addrs: Vec<Address> = outgoing.into_iter().map(|t| t.to).collect();

            Ok(Some(ClusterEvidence {
                addresses: chain_addrs,
                heuristic: ClusteringHeuristic::PeelingChain,
                confidence: Confidence::HIGH,
                notes: Some(format!(
                    "Address retains only {:.1}% of inflow",
                    retained_ratio.as_f64() * 100.0
                )),
            }))
        }

        pub async fn save_cluster(
            &self,
            evidence: ClusterEvidence,
            category: EntityCategory,
        ) -> DomainResult<()> {
            let mut entity = Entity::new(category, RiskScore::MEDIUM);
            for addr in evidence.addresses {
                entity.add_address(addr);
            }
            self.entities.save(&entity).await
        }
    }
}
