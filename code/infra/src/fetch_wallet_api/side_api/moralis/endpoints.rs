pub fn wallet_history(walltet_addr: &str) -> String {
    format!("/api/v2.2/wallets/{}/history", walltet_addr)
}

pub fn eth_latest_block() -> String {
    "/api/v2.2/latestBlockNumber/eth".to_string()
}
