use domain::chain::ChainId;
use domain::primitives::Address;

pub(super) fn parse_hex_u64(s: &str) -> anyhow::Result<u64> {
    let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
    Ok(u64::from_str_radix(s, 16)?)
}

pub(super) fn parse_hex_u256(s: &str) -> anyhow::Result<domain::primitives::U256> {
    let s = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")).unwrap_or(s);
    if s.is_empty() {
        return Ok(domain::primitives::U256::zero());
    }
    Ok(domain::primitives::U256::from_str_radix(s, 16)?)
}

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

/// Strip the 12-byte left-pad from a topic-encoded address.
pub(super) fn unpad_address(topic: &str) -> String {
    let s = topic.strip_prefix("0x").unwrap_or(topic);
    if s.len() >= 40 {
        format!("0x{}", &s[s.len() - 40..])
    } else {
        format!("0x{s}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpad_address_strips_left_pad() {
        let topic = "0x000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7";
        assert_eq!(unpad_address(topic), "0xdac17f958d2ee523a2206206994597c13d831ec7");
    }

    #[test]
    fn parse_hex_u64_handles_prefix_and_case() {
        assert_eq!(parse_hex_u64("0x10").unwrap(), 16);
        assert_eq!(parse_hex_u64("0X10").unwrap(), 16);
        assert_eq!(parse_hex_u64("ff").unwrap(), 255);
    }

    #[test]
    fn parse_hex_u256_empty_is_zero() {
        assert!(parse_hex_u256("0x").unwrap().is_zero());
        assert!(parse_hex_u256("").unwrap().is_zero());
    }
}
