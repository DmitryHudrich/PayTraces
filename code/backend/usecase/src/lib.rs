pub mod ingestion;
pub mod labels;
pub mod risk;
pub mod union_find;

pub use ingestion::{AdaptiveConcurrency, AutoTagging, IngestionService};
pub use labels::{
    TagApplyInput, TagPatchInput, apply_tag, deactivate_all, deactivate_one, default_risk_for,
    patch_tag,
};
pub use risk::RiskService;
pub use union_find::UnionFind;

fn addr_hex(addr: &domain::primitives::Address) -> String {
    format!("0x{}", hex::encode(addr.bytes()))
}
