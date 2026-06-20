pub fn trc20_transfers(address_b58: &str, fingerprint: Option<&str>) -> String {
    let mut s = format!(
        "/v1/accounts/{address_b58}/transactions/trc20?limit=200&only_confirmed=true"
    );
    if let Some(fp) = fingerprint {
        s.push_str("&fingerprint=");
        s.push_str(fp);
    }
    s
}

pub fn trc20_transfers_for_token(
    address_b58: &str,
    contract_address_b58: &str,
    fingerprint: Option<&str>,
) -> String {
    let mut s = format!(
        "/v1/accounts/{address_b58}/transactions/trc20?contract_address={contract_address_b58}&limit=200&only_confirmed=true"
    );
    if let Some(fp) = fingerprint {
        s.push_str("&fingerprint=");
        s.push_str(fp);
    }
    s
}

pub fn native_transfers(address_b58: &str, fingerprint: Option<&str>) -> String {
    let mut s = format!(
        "/v1/accounts/{address_b58}/transactions?limit=200&only_confirmed=true"
    );
    if let Some(fp) = fingerprint {
        s.push_str("&fingerprint=");
        s.push_str(fp);
    }
    s
}
