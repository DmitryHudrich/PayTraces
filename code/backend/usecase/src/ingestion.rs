use std::collections::{HashMap, HashSet, VecDeque};

use async_trait::async_trait;
use domain::chain::{ChainId, ChainRegistry};
use domain::error::DomainResult;
use domain::graph::{GraphRequest, TransferGraph};
use domain::ports::{BlockRange, ChainSourceRegistry, IngestionPort, TransferRepository};
use domain::primitives::Address;
use domain::transfer::Transfer;

pub struct IngestionService<S, R> {
    sources: S,
    repo: R,
    chains: ChainRegistry,
}

impl<S, R> IngestionService<S, R> {
    pub fn new(sources: S, repo: R, chains: ChainRegistry) -> Self {
        Self { sources, repo, chains }
    }

    pub fn sources(&self) -> &S {
        &self.sources
    }

    pub fn repo(&self) -> &R {
        &self.repo
    }

    pub fn chains(&self) -> &ChainRegistry {
        &self.chains
    }
}

impl<S, R> IngestionService<S, R>
where
    R: TransferRepository,
{
    pub async fn build_graph_from_db(
        &self,
        origin: &Address,
        req: GraphRequest,
    ) -> DomainResult<TransferGraph> {
        let mut nodes: HashSet<Address> = HashSet::new();
        let mut edges: Vec<Transfer> = Vec::new();
        let mut visited: HashSet<Address> = HashSet::new();
        let mut queue: VecDeque<(Address, u32)> = VecDeque::new();

        queue.push_back((origin.clone(), 0));
        visited.insert(origin.clone());
        nodes.insert(origin.clone());

        let range = req.range();

        while let Some((addr, depth)) = queue.pop_front() {
            let transfers = self.repo.find_by_address(&addr, range).await?;

            let next_depth = depth + 1;
            let can_expand = next_depth < req.max_depth();

            for t in transfers {
                let counterparty = if t.from() == &addr {
                    t.to().clone()
                } else {
                    t.from().clone()
                };

                nodes.insert(t.from().clone());
                nodes.insert(t.to().clone());
                edges.push(t);

                if can_expand
                    && !visited.contains(&counterparty)
                    && visited.len() < req.max_nodes()
                {
                    visited.insert(counterparty.clone());
                    queue.push_back((counterparty, next_depth));
                }
            }
        }

        edges.sort_by(|a, b| {
            a.id()
                .tx_hash()
                .cmp(b.id().tx_hash())
                .then(a.id().index().cmp(&b.id().index()))
        });
        edges.dedup_by(|a, b| a.id() == b.id());

        Ok(TransferGraph::new(nodes, edges))
    }
}

#[async_trait]
impl<S, R> IngestionPort for IngestionService<S, R>
where
    S: ChainSourceRegistry,
    R: TransferRepository,
{
    #[tracing::instrument(skip(self, origin), fields(
        address = %origin,
        max_depth = req.max_depth(),
        max_nodes = req.max_nodes(),
    ))]
    async fn build_graph(
        &self,
        origin: &Address,
        req: GraphRequest,
    ) -> DomainResult<TransferGraph> {
        tracing::info!(
            origin = %origin,
            chain = %origin.chain(),
            max_depth = req.max_depth(),
            max_nodes = req.max_nodes(),
            "graph build started"
        );

        let mut nodes: HashSet<Address> = HashSet::new();
        let mut edges: Vec<Transfer> = Vec::new();
        let mut visited: HashSet<Address> = HashSet::new();
        let mut queue: VecDeque<(Address, u32)> = VecDeque::new();

        queue.push_back((origin.clone(), 0));
        visited.insert(origin.clone());
        nodes.insert(origin.clone());

        let user_range = req.range().unwrap_or_else(BlockRange::full);
        let user_from = user_range.from_height();
        let user_to = user_range.to_height();

        let mut latest_by_chain: HashMap<ChainId, u64> = HashMap::new();

        while let Some((addr, depth)) = queue.pop_front() {
            let source = match self.sources.source(addr.chain()) {
                Some(s) => s,
                None => {
                    tracing::warn!(
                        address = %addr,
                        chain = %addr.chain(),
                        "no source registered for chain, skipping node"
                    );
                    continue;
                }
            };

            let latest_height = match latest_by_chain.get(&addr.chain()).copied() {
                Some(h) => h,
                None => match source.latest_block().await {
                    Ok(b) => {
                        let h = b.height();
                        latest_by_chain.insert(addr.chain(), h);
                        h
                    }
                    Err(e) => {
                        tracing::warn!(
                            chain = %addr.chain(),
                            error = %e,
                            "latest_block fetch failed; using user range as-is"
                        );
                        u64::MAX
                    }
                },
            };

            let effective_from = user_from;
            let effective_to = user_to.min(latest_height);

            let effective_range = if effective_to < effective_from {
                tracing::debug!(
                    address = %addr,
                    effective_from,
                    effective_to,
                    "skipping fetch: empty effective range"
                );
                None
            } else {
                Some(BlockRange::new(effective_from, effective_to))
            };

            if let Some(r) = effective_range
                && let Ok(fetched) = source
                    .transfers_for_address(&addr, r, req.max_nodes())
                    .await
                    .map_err(|e| {
                        tracing::warn!(
                            address = %addr,
                            error = %e,
                            "chain fetch failed, falling back to DB"
                        );
                        e
                    })
                && !fetched.is_empty()
            {
                if let Err(e) = self
                    .repo
                    .delete_in_range(&addr, effective_from, effective_to)
                    .await
                {
                    tracing::warn!(
                        address = %addr,
                        error = %e,
                        "delete-in-range failed; proceeding with save"
                    );
                }
                if let Err(e) = self.repo.save(&fetched).await {
                    tracing::warn!(address = %addr, error = %e, "save failed");
                }
            }

            let combined = self
                .repo
                .find_by_address(&addr, Some(user_range))
                .await
                .unwrap_or_default();

            tracing::debug!(
                address = %addr, depth,
                transfers = combined.len(),
                visited = visited.len(),
                nodes = nodes.len(),
                effective_from,
                effective_to,
                "graph BFS step"
            );

            let next_depth = depth + 1;
            let can_expand = next_depth < req.max_depth();

            for t in combined {
                let counterparty = if t.from() == &addr {
                    t.to().clone()
                } else {
                    t.from().clone()
                };

                nodes.insert(t.from().clone());
                nodes.insert(t.to().clone());
                edges.push(t);

                if can_expand && !visited.contains(&counterparty) && visited.len() < req.max_nodes()
                {
                    visited.insert(counterparty.clone());
                    queue.push_back((counterparty, next_depth));
                }
            }
        }

        edges.sort_by(|a, b| {
            a.id()
                .tx_hash()
                .cmp(b.id().tx_hash())
                .then(a.id().index().cmp(&b.id().index()))
        });
        edges.dedup_by(|a, b| a.id() == b.id());

        tracing::info!(
            origin = %origin,
            nodes = nodes.len(), edges = edges.len(),
            "graph build complete"
        );

        Ok(TransferGraph::new(nodes, edges))
    }
}
