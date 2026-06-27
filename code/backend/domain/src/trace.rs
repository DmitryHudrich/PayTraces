use crate::primitives::{Address, Amount, Ratio};
use crate::transfer::Transfer;

#[derive(Debug, Clone)]
pub struct TraceRequest {
    origin: TraceOrigin,
    direction: TraceDirection,
    strategy: TaintStrategy,
    limits: TraceLimits,
    include_unconfirmed: bool,
}

impl TraceRequest {
    pub fn new(
        origin: TraceOrigin,
        direction: TraceDirection,
        strategy: TaintStrategy,
        limits: TraceLimits,
        include_unconfirmed: bool,
    ) -> Self {
        Self {
            origin,
            direction,
            strategy,
            limits,
            include_unconfirmed,
        }
    }

    pub fn origin(&self) -> &TraceOrigin {
        &self.origin
    }

    pub fn direction(&self) -> TraceDirection {
        self.direction
    }

    pub fn strategy(&self) -> TaintStrategy {
        self.strategy
    }

    pub fn limits(&self) -> TraceLimits {
        self.limits
    }

    pub fn include_unconfirmed(&self) -> bool {
        self.include_unconfirmed
    }
}

#[derive(Debug, Clone)]
pub enum TraceOrigin {
    Address(Address),
    Transaction {
        chain: crate::chain::ChainId,
        hash: [u8; 32],
    },
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
    max_hops: u32,
    max_addresses: usize,
    max_paths: usize,
    min_amount_ratio: Option<Ratio>,
    /// Edges with `edge_significance < min_edge_significance` are skipped.
    /// `None` keeps the trace inclusive (default behaviour).
    min_edge_significance: Option<f64>,
}

impl TraceLimits {
    pub fn new(
        max_hops: u32,
        max_addresses: usize,
        max_paths: usize,
        min_amount_ratio: Option<Ratio>,
    ) -> Self {
        Self {
            max_hops,
            max_addresses,
            max_paths,
            min_amount_ratio,
            min_edge_significance: None,
        }
    }

    pub fn with_min_edge_significance(mut self, value: f64) -> Self {
        self.min_edge_significance = Some(value);
        self
    }

    pub fn max_hops(self) -> u32 {
        self.max_hops
    }

    pub fn max_addresses(self) -> usize {
        self.max_addresses
    }

    pub fn max_paths(self) -> usize {
        self.max_paths
    }

    pub fn min_amount_ratio(self) -> Option<Ratio> {
        self.min_amount_ratio
    }

    pub fn min_edge_significance(self) -> Option<f64> {
        self.min_edge_significance
    }
}

impl Default for TraceLimits {
    fn default() -> Self {
        Self {
            max_hops: 10,
            max_addresses: 1_000,
            max_paths: 500,
            min_amount_ratio: Some(Ratio::from_percent(1)),
            min_edge_significance: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TraceResult {
    request: TraceRequest,
    paths: Vec<FlowPath>,
    terminal_sinks: Vec<Sink>,
    stats: TraceStats,
}

impl TraceResult {
    pub fn new(
        request: TraceRequest,
        paths: Vec<FlowPath>,
        terminal_sinks: Vec<Sink>,
        stats: TraceStats,
    ) -> Self {
        Self {
            request,
            paths,
            terminal_sinks,
            stats,
        }
    }

    pub fn request(&self) -> &TraceRequest {
        &self.request
    }

    pub fn paths(&self) -> &[FlowPath] {
        &self.paths
    }

    pub fn terminal_sinks(&self) -> &[Sink] {
        &self.terminal_sinks
    }

    pub fn stats(&self) -> TraceStats {
        self.stats
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub fn highest_risk_sink(&self) -> Option<&Sink> {
        self.terminal_sinks.iter().max_by_key(|s| s.risk_score())
    }
}

#[derive(Debug, Clone)]
pub struct FlowPath {
    hops: Vec<Transfer>,
    tainted_amount: Amount,
    taint_ratio: Ratio,
    depth: u32,
}

impl FlowPath {
    pub fn new(
        hops: Vec<Transfer>,
        tainted_amount: Amount,
        taint_ratio: Ratio,
        depth: u32,
    ) -> Self {
        Self {
            hops,
            tainted_amount,
            taint_ratio,
            depth,
        }
    }

    pub fn hops(&self) -> &[Transfer] {
        &self.hops
    }

    pub fn tainted_amount(&self) -> Amount {
        self.tainted_amount
    }

    pub fn taint_ratio(&self) -> Ratio {
        self.taint_ratio
    }

    pub fn depth(&self) -> u32 {
        self.depth
    }

    pub fn origin(&self) -> Option<&Address> {
        self.hops.first().map(|t| t.from())
    }

    pub fn destination(&self) -> Option<&Address> {
        self.hops.last().map(|t| t.to())
    }
}

#[derive(Debug, Clone)]
pub struct Sink {
    address: Address,
    kind: SinkKind,
    tainted_amount: Amount,
    taint_ratio: Ratio,
}

impl Sink {
    pub fn new(
        address: Address,
        kind: SinkKind,
        tainted_amount: Amount,
        taint_ratio: Ratio,
    ) -> Self {
        Self {
            address,
            kind,
            tainted_amount,
            taint_ratio,
        }
    }

    pub fn address(&self) -> &Address {
        &self.address
    }

    pub fn kind(&self) -> &SinkKind {
        &self.kind
    }

    pub fn tainted_amount(&self) -> Amount {
        self.tainted_amount
    }

    pub fn taint_ratio(&self) -> Ratio {
        self.taint_ratio
    }

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
    addresses_visited: usize,
    transfers_evaluated: usize,
    paths_found: usize,
    depth_reached: u32,
    truncated: bool,
}

impl TraceStats {
    pub fn new(
        addresses_visited: usize,
        transfers_evaluated: usize,
        paths_found: usize,
        depth_reached: u32,
        truncated: bool,
    ) -> Self {
        Self {
            addresses_visited,
            transfers_evaluated,
            paths_found,
            depth_reached,
            truncated,
        }
    }

    pub fn addresses_visited(self) -> usize {
        self.addresses_visited
    }

    pub fn transfers_evaluated(self) -> usize {
        self.transfers_evaluated
    }

    pub fn paths_found(self) -> usize {
        self.paths_found
    }

    pub fn depth_reached(self) -> u32 {
        self.depth_reached
    }

    pub fn truncated(self) -> bool {
        self.truncated
    }
}
