use crate::primitives::{Address, Amount, BlockRef, Ratio};
use crate::transfer::Transfer;

#[derive(Debug, Clone)]
pub struct TraceRequest {
    pub origin: TraceOrigin,
    pub direction: TraceDirection,
    pub strategy: TaintStrategy,
    pub limits: TraceLimits,
    pub include_unconfirmed: bool,
}

#[derive(Debug, Clone)]
pub enum TraceOrigin {
    Address(Address),
    Transaction([u8; 32]),
    Transfer(crate::transfer::TransferId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceDirection {
    Forward,
    Backward,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaintStrategy {
    Poison,
    Haircut,
    Fifo,
    Lifo,
}

#[derive(Debug, Clone, Copy)]
pub struct TraceLimits {
    pub max_hops: u32,
    pub max_addresses: usize,
    pub max_paths: usize,
    pub min_amount_ratio: Option<Ratio>,
}

impl Default for TraceLimits {
    fn default() -> Self {
        Self {
            max_hops: 10,
            max_addresses: 1_000,
            max_paths: 500,
            min_amount_ratio: Some(Ratio::from_percent(1)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TraceResult {
    pub request: TraceRequest,
    pub paths: Vec<FlowPath>,
    pub terminal_sinks: Vec<Sink>,
    pub stats: TraceStats,
}

impl TraceResult {
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub fn highest_risk_sink(&self) -> Option<&Sink> {
        self.terminal_sinks.iter().max_by_key(|s| s.risk_score())
    }
}

#[derive(Debug, Clone)]
pub struct FlowPath {
    pub hops: Vec<Transfer>,
    pub tainted_amount: Amount,
    pub taint_ratio: Ratio,
    pub depth: u32,
}

impl FlowPath {
    pub fn origin(&self) -> Option<&Address> {
        self.hops.first().map(|t| &t.from)
    }

    pub fn destination(&self) -> Option<&Address> {
        self.hops.last().map(|t| &t.to)
    }
}

#[derive(Debug, Clone)]
pub struct Sink {
    pub address: Address,
    pub kind: SinkKind,
    pub tainted_amount: Amount,
    pub taint_ratio: Ratio,
}

impl Sink {
    pub fn risk_score(&self) -> u8 {
        match &self.kind {
            SinkKind::Exchange { .. } => 30,
            SinkKind::Bridge { .. } => 40,
            SinkKind::Mixer => 90,
            SinkKind::Sanctioned => 100,
            SinkKind::Darknet => 95,
            SinkKind::Unresolved => 20,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SinkKind {
    Exchange {
        name: String,
        requires_subpoena: bool,
    },
    Bridge {
        destination_chain: Option<crate::chain::ChainId>,
    },
    Mixer,
    Sanctioned,
    Darknet,
    Unresolved,
}

#[derive(Debug, Clone, Copy)]
pub struct TraceStats {
    pub addresses_visited: usize,
    pub transfers_evaluated: usize,
    pub paths_found: usize,
    pub depth_reached: u32,
    pub truncated: bool,
}

