use std::collections::{HashSet, VecDeque};

use async_trait::async_trait;
use domain::error::DomainResult;
use domain::graph::{GraphRequest, TransferGraph};
use domain::ports::{BlockRange, ChainSourceRegistry, IngestionPort, TransferRepository};
use domain::primitives::Address;
use domain::transfer::Transfer;

pub struct IngestionService<S, R> {
    sources: S,
    repo: R,
}

impl<S, R> IngestionService<S, R> {
    pub fn new(sources: S, repo: R) -> Self {
        Self { sources, repo }
    }

    pub fn sources(&self) -> &S {
        &self.sources
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

        let chain_range = req.range().unwrap_or_else(BlockRange::full);

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

            let transfers = match source
                .transfers_for_address(&addr, chain_range, req.max_nodes())
                .await
            {
                Ok(fetched) => {
                    if !fetched.is_empty()
                        && let Err(e) = self.repo.save(&fetched).await
                    {
                        tracing::warn!(
                            address = %addr,
                            error = %e,
                            "save failed"
                        );
                    }
                    fetched
                }
                Err(e) => {
                    tracing::warn!(
                        address = %addr,
                        error = %e,
                        "chain fetch failed, skipping node"
                    );
                    vec![]
                }
            };

            tracing::debug!(
                address = %addr, depth,
                transfers = transfers.len(),
                visited = visited.len(),
                nodes = nodes.len(),
                "graph BFS step"
            );

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
