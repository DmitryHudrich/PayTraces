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
}

#[derive(Debug, Clone, Copy)]
pub struct GraphRequest {
    range: Option<BlockRange>,
    max_depth: u32,
    max_nodes: usize,
}

impl GraphRequest {
    pub fn new(range: Option<BlockRange>, max_depth: u32, max_nodes: usize) -> Self {
        Self {
            range,
            max_depth,
            max_nodes,
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
