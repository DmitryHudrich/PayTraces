use std::collections::{HashSet, VecDeque};

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
            if t.from() == addr {
                result.push(t.to());
            } else if t.to() == addr {
                result.push(t.from());
            }
        }
        result
    }

    pub fn outgoing(&self, addr: &Address) -> Vec<&Transfer> {
        self.edges.iter().filter(|t| t.from() == addr).collect()
    }

    pub fn incoming(&self, addr: &Address) -> Vec<&Transfer> {
        self.edges.iter().filter(|t| t.to() == addr).collect()
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
                    depth,
                    nodes = nodes.len(),
                    max_depth = req.max_depth,
                    max_nodes = req.max_nodes,
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
                let counterparty = if t.from() == &addr {
                    t.to().clone()
                } else {
                    t.from().clone()
                };

                nodes.insert(t.from().clone());
                nodes.insert(t.to().clone());
                edges.push(t);

                if !visited.contains(&counterparty) && nodes.len() < req.max_nodes {
                    visited.insert(counterparty.clone());
                    queue.push_back((counterparty, depth + 1));
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
            origin = %super::addr_hex(origin),
            nodes = nodes.len(), edges = edges.len(),
            "graph build complete"
        );

        Ok(TransferGraph { nodes, edges })
    }
}

