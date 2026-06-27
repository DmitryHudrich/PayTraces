use std::collections::HashSet;

use crate::ports::BlockRange;
use crate::primitives::Address;
use crate::transfer::Transfer;

#[derive(Debug, Clone)]
pub struct TransferGraph {
    nodes: HashSet<Address>,
    edges: Vec<Transfer>,
}

impl TransferGraph {
    pub fn new(nodes: HashSet<Address>, edges: Vec<Transfer>) -> Self {
        Self { nodes, edges }
    }

    pub fn nodes(&self) -> &HashSet<Address> {
        &self.nodes
    }

    pub fn edges(&self) -> &[Transfer] {
        &self.edges
    }

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

    /// BFS shortest path from `from` to `to` over the undirected projection of
    /// the transfer graph. Returns the sequence of edges that connects them, or
    /// `None` if unreachable. Picks the first edge per (u,v) pair encountered.
    pub fn shortest_path(&self, from: &Address, to: &Address) -> Option<Vec<Transfer>> {
        if from == to {
            return Some(Vec::new());
        }

        use std::collections::{HashMap, HashSet, VecDeque};

        let mut adjacency: HashMap<Address, Vec<(Address, &Transfer)>> = HashMap::new();
        for e in &self.edges {
            adjacency
                .entry(e.from().clone())
                .or_default()
                .push((e.to().clone(), e));
            adjacency
                .entry(e.to().clone())
                .or_default()
                .push((e.from().clone(), e));
        }

        let mut prev: HashMap<Address, (Address, Transfer)> = HashMap::new();
        let mut visited: HashSet<Address> = HashSet::new();
        let mut q: VecDeque<Address> = VecDeque::new();
        q.push_back(from.clone());
        visited.insert(from.clone());

        while let Some(curr) = q.pop_front() {
            if &curr == to {
                break;
            }
            let Some(neighbors) = adjacency.get(&curr) else {
                continue;
            };
            for (next, edge) in neighbors {
                if visited.insert(next.clone()) {
                    prev.insert(next.clone(), (curr.clone(), (*edge).clone()));
                    q.push_back(next.clone());
                }
            }
        }

        if !visited.contains(to) {
            return None;
        }
        let mut chain: Vec<Transfer> = Vec::new();
        let mut cursor = to.clone();
        while let Some((p, e)) = prev.remove(&cursor) {
            chain.push(e);
            cursor = p;
            if &cursor == from {
                break;
            }
        }
        chain.reverse();
        Some(chain)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GraphRequest {
    range: Option<BlockRange>,
    max_depth: u32,
    max_nodes: usize,
    max_transfers_per_address: usize,
}

impl GraphRequest {
    pub fn new(
        range: Option<BlockRange>,
        max_depth: u32,
        max_nodes: usize,
        max_transfers_per_address: usize,
    ) -> Self {
        Self {
            range,
            max_depth,
            max_nodes,
            max_transfers_per_address,
        }
    }

    pub fn range(&self) -> Option<BlockRange> {
        self.range
    }

    pub fn max_depth(&self) -> u32 {
        self.max_depth
    }

    pub fn max_nodes(&self) -> usize {
        self.max_nodes
    }

    pub fn max_transfers_per_address(&self) -> usize {
        self.max_transfers_per_address
    }
}

impl Default for GraphRequest {
    fn default() -> Self {
        Self {
            range: None,
            max_depth: 3,
            max_nodes: 500,
            max_transfers_per_address: 10_000,
        }
    }
}
