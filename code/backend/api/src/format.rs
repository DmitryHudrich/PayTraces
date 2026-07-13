use chrono::NaiveDateTime;
use serde::{Deserialize, Deserializer};

use crate::error::ApiError;
use domain::chain::{ChainId, ChainRegistry};
use domain::label_tag::{TagCategory, TagSource};
use domain::primitives::{Address, U256};
use domain::risk::RiskSignalKind;
use domain::trace::SinkKind;
use domain::transfer::TransferKind;

pub fn parse_address(s: &str, chain: ChainId) -> Result<Address, ApiError> {
    Address::parse(chain, s).map_err(|e| ApiError::bad_request(e.to_string()))
}

/// Tronscan's UI date format, e.g. `2026-07-13 08:04:09` — always UTC, since
/// that's what block/transfer timestamps are stored as everywhere else in
/// this codebase.
const TRONSCAN_DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

#[derive(Deserialize)]
#[serde(untagged)]
enum HeightOrDate {
    Height(u64),
    Text(String),
}

/// Deserializes `from_block`/`to_block`-style fields that, for Tron, are
/// actually ms-since-epoch timestamps (see `TronGridSource` doc comment).
/// Accepts either the raw number or a Tronscan-style UTC date string
/// (`"2026-07-13 08:04:09"`), so callers don't have to convert by hand.
pub fn deserialize_height_or_date<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let Some(value) = Option::<HeightOrDate>::deserialize(deserializer)? else {
        return Ok(None);
    };
    let height = match value {
        HeightOrDate::Height(h) => h,
        HeightOrDate::Text(s) => match s.parse::<u64>() {
            Ok(h) => h,
            Err(_) => {
                let dt = NaiveDateTime::parse_from_str(&s, TRONSCAN_DATETIME_FORMAT)
                    .map_err(|e| serde::de::Error::custom(format!("invalid date {s:?}: {e}")))?;
                dt.and_utc().timestamp_millis().max(0) as u64
            }
        },
    };
    Ok(Some(height))
}

pub fn format_amount(raw: U256, decimals: u8) -> String {
    let s = raw.to_string();
    if decimals == 0 {
        return s;
    }
    let dec = decimals as usize;
    let (int_part, frac_part) = if s.len() <= dec {
        (String::from("0"), format!("{:0>w$}", s, w = dec))
    } else {
        let split = s.len() - dec;
        (s[..split].to_string(), s[split..].to_string())
    };
    let frac_trunc = if frac_part.len() > 8 {
        &frac_part[..8]
    } else {
        &frac_part[..]
    };
    let frac_trim = frac_trunc.trim_end_matches('0');
    if frac_trim.is_empty() {
        int_part
    } else {
        format!("{int_part}.{frac_trim}")
    }
}

pub fn native_symbol(chains: &ChainRegistry, chain: ChainId) -> String {
    chains
        .get(chain)
        .map(|m| m.native_asset_symbol().to_string())
        .unwrap_or_default()
}

pub fn transfer_kind_str(k: &TransferKind) -> (&'static str, Option<String>) {
    match k {
        TransferKind::Native => ("native", None),
        TransferKind::Token { contract, .. } => ("token", Some(contract.canonical())),
        TransferKind::Internal => ("internal", None),
        TransferKind::Fee => ("fee", None),
        TransferKind::UtxoEdge { .. } => ("utxo_edge", None),
    }
}

pub fn edge_symbol(k: &TransferKind, native: &str) -> String {
    match k {
        TransferKind::Native | TransferKind::Internal | TransferKind::Fee => native.to_string(),
        TransferKind::Token { symbol, .. } => symbol.clone().unwrap_or_default(),
        TransferKind::UtxoEdge { .. } => native.to_string(),
    }
}

pub fn sink_kind_str(k: &SinkKind) -> (&'static str, Option<String>) {
    match k {
        SinkKind::Exchange { name, .. } => ("exchange", Some(name.clone())),
        SinkKind::Bridge { .. } => ("bridge", None),
        SinkKind::Mixer => ("mixer", None),
        SinkKind::Sanctioned => ("sanctioned", None),
        SinkKind::Darknet => ("darknet", None),
        SinkKind::Unresolved => ("unresolved", None),
    }
}

pub fn signal_kind_str(k: &RiskSignalKind) -> &'static str {
    match k {
        RiskSignalKind::DirectExposure => "direct_exposure",
        RiskSignalKind::IndirectExposure { .. } => "indirect_exposure",
        RiskSignalKind::SanctionedCounterparty => "sanctioned_counterparty",
        RiskSignalKind::MixerInteraction => "mixer_interaction",
        RiskSignalKind::DarknetMarket => "darknet_market",
        RiskSignalKind::RapidLayering => "rapid_layering",
        RiskSignalKind::HighVelocity => "high_velocity",
        RiskSignalKind::NewAddress => "new_address",
        RiskSignalKind::NoKyc => "no_kyc",
    }
}

pub fn tag_source_str(s: &TagSource) -> String {
    match s {
        TagSource::OfacSdn => "ofac_sdn".into(),
        TagSource::EuSanctions => "eu_sanctions".into(),
        TagSource::UnSanctions => "un_sanctions".into(),
        TagSource::InternalAnalyst => "internal_analyst".into(),
        TagSource::HeuristicCluster => "heuristic_cluster".into(),
        TagSource::ThirdParty(detail) => format!("third_party:{detail}"),
        TagSource::LegacyImport => "legacy_import".into(),
    }
}

pub fn tag_category_str(c: TagCategory) -> &'static str {
    match c {
        TagCategory::Exchange => "exchange",
        TagCategory::Mixer => "mixer",
        TagCategory::Bridge => "bridge",
        TagCategory::DefiProtocol => "defi",
        TagCategory::Sanctioned => "sanctioned",
        TagCategory::Scam => "scam",
        TagCategory::Gambling => "gambling",
        TagCategory::Darknet => "darknet",
        TagCategory::Mining => "mining",
        TagCategory::KnownService => "known_service",
        TagCategory::Unknown => "unknown",
    }
}

#[cfg(test)]
mod height_or_date_tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct T {
        #[serde(default, deserialize_with = "deserialize_height_or_date")]
        v: Option<u64>,
    }

    #[test]
    fn accepts_plain_number() {
        let t: T = serde_json::from_str(r#"{"v": 1783934511001}"#).unwrap();
        assert_eq!(t.v, Some(1783934511001));
    }

    #[test]
    fn accepts_numeric_string() {
        let t: T = serde_json::from_str(r#"{"v": "1783934511001"}"#).unwrap();
        assert_eq!(t.v, Some(1783934511001));
    }

    #[test]
    fn accepts_tronscan_date_string() {
        let t: T = serde_json::from_str(r#"{"v": "2026-07-13 08:04:09"}"#).unwrap();
        assert_eq!(t.v, Some(1783929849000));
    }

    #[test]
    fn missing_is_none() {
        let t: T = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(t.v, None);
    }
}

#[cfg(test)]
mod height_or_date_query_tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct T {
        #[serde(default, deserialize_with = "deserialize_height_or_date")]
        v: Option<u64>,
    }

    #[test]
    fn accepts_date_via_urlencoded_query_string() {
        let t: T = serde_urlencoded::from_str("v=2026-07-13%2008%3A04%3A09").unwrap();
        assert_eq!(t.v, Some(1783929849000));
    }

    #[test]
    fn accepts_number_via_urlencoded_query_string() {
        let t: T = serde_urlencoded::from_str("v=1783929849000").unwrap();
        assert_eq!(t.v, Some(1783929849000));
    }
}
