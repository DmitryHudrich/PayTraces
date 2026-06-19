pub mod ingestion;
pub mod risk;

pub use ingestion::IngestionService;
pub use risk::RiskService;

fn addr_hex(addr: &domain::primitives::Address) -> String {
    format!("0x{}", hex::encode(addr.bytes()))
}
