use domain::chain::ChainId;
use domain::primitives::Address;

pub(super) fn parse_hash32(s: &str) -> anyhow::Result<[u8; 32]> {
    use anyhow::{Context, anyhow};
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).context("hex decode")?;
    bytes
        .try_into()
        .map_err(|v: Vec<u8>| anyhow!("expected 32 bytes, got {}", v.len()))
}

pub(super) fn parse_eth_address(chain: ChainId, s: &str) -> anyhow::Result<Address> {
    use anyhow::Context;
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).context("hex decode eth address")?;
    if bytes.len() != 20 {
        anyhow::bail!("eth address expected 20 bytes, got {}", bytes.len());
    }
    Ok(Address::new(chain, bytes))
}
