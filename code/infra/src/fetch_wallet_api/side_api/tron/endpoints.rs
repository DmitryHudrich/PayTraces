pub fn trc20_transfers(address: &str, contract_address: &str) -> String {
    format!("/v1/accounts/{address}/transactions/trc20?contract_address={contract_address}&limit=200&only_confirmed=true")
}

pub fn trc20_transfers_with_cursor(address: &str, contract_address: &str, cursor: &str) -> String {
    format!("/v1/accounts/{address}/transactions/trc20?contract_address={contract_address}&limit=200&only_confirmed=true&fingerprint={cursor}")
}

pub fn native_transfers(address: &str) -> String {
    format!("/v1/accounts/{address}/transactions?limit=200&only_confirmed=true&only_to=false&only_from=false")
}

pub fn native_transfers_with_cursor(address: &str, cursor: &str) -> String {
    format!("/v1/accounts/{address}/transactions?limit=200&only_confirmed=true&only_to=false&only_from=false&fingerprint={cursor}")
}
