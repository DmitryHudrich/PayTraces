fn addr_hex(addr: &domain::primitives::Address) -> String {
    format!("0x{}", hex::encode(addr.bytes()))
}

pub mod build_transfer_graph;
pub mod check_sanctions;
pub mod cluster_addresses;
pub mod ingest_address;
pub mod score_address;
pub mod trace_funds;
