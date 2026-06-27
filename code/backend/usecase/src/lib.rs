pub mod ingestion;
pub mod risk;
pub mod union_find;

pub use ingestion::{AdaptiveConcurrency, IngestionService};
pub use risk::RiskService;
pub use union_find::UnionFind;

fn addr_hex(addr: &domain::primitives::Address) -> String {
    format!("0x{}", hex::encode(addr.bytes()))
}
