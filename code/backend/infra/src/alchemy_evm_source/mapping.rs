use std::collections::HashMap;

use chrono::TimeZone;
use serde_json::json;

use domain::{
    asset::{AssetId, TokenStandard},
    chain::ChainId,
    transfer::{Finality, Transfer, TransferId, TransferKind},
    primitives::{Amount, BlockRef, TxRef},
};

use super::TRANSFER_TOPIC;
use super::parse::{parse_eth_address, parse_hash32, parse_hex_u256, parse_hex_u64, unpad_address};

/// Build the `topics` array for an ERC-20 Transfer filter. Slot 0 is the
/// event signature hash, slot 1 is the optional `from` (padded), slot 2 is
/// the optional `to` (padded). `null` means "any" for that slot. Slot 3 is
/// omitted so ERC-721 Transfer events (which have an indexed tokenId there)
/// fall in too; the mapper filters them out by checking topics.len() == 3.
pub(super) fn build_transfer_topics(
    topic_from: Option<&str>,
    topic_to: Option<&str>,
) -> serde_json::Value {
    let null = serde_json::Value::Null;
    let f = topic_from
        .map(|s| json!(s))
        .unwrap_or(null.clone());
    let t = topic_to.map(|s| json!(s)).unwrap_or(null);
    json!([TRANSFER_TOPIC, f, t])
}

/// Map an Alchemy `trace_filter` row into a native-ETH Transfer. Filters:
/// * `error` field present → sub-call reverted, no value moved.
/// * `delegatecall` / `staticcall` → no value transfer by semantics.
/// * `value == 0` → nothing to record.
/// * `traceAddress == []` AND outer `type == "call"` — the outermost trace
///   IS the outer tx's value transfer (idx=0); inner traces shift by +1.
pub(super) fn map_trace_to_transfer(
    chain: ChainId,
    raw: &serde_json::Value,
    block_ts: &HashMap<u64, chrono::DateTime<chrono::Utc>>,
    by_tx: &mut HashMap<[u8; 32], u32>,
) -> anyhow::Result<Option<Transfer>> {
    use anyhow::{Context, anyhow};

    if raw.get("error").and_then(|v| v.as_str()).is_some() {
        return Ok(None);
    }

    let trace_type = raw
        .get("type")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("trace: missing type"))?;
    let action = raw
        .get("action")
        .ok_or_else(|| anyhow!("trace: missing action"))?;

    let (from_s, to_s, value_s) = match trace_type {
        "call" => {
            let call_type = action.get("callType").and_then(|v| v.as_str()).unwrap_or("call");
            if matches!(call_type, "delegatecall" | "staticcall") {
                return Ok(None);
            }
            let from_s = action
                .get("from")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("trace.call: missing from"))?;
            let to_s = action
                .get("to")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("trace.call: missing to"))?;
            let value_s = action
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("0x0");
            (from_s, to_s, value_s)
        }
        "create" => {
            let from_s = action
                .get("from")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("trace.create: missing from"))?;
            let to_s = raw
                .get("result")
                .and_then(|r| r.get("address"))
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("trace.create: missing result.address"))?;
            let value_s = action
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("0x0");
            (from_s, to_s, value_s)
        }
        "suicide" => {
            let from_s = action
                .get("address")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("trace.suicide: missing address"))?;
            let to_s = action
                .get("refundAddress")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("trace.suicide: missing refundAddress"))?;
            let value_s = action
                .get("balance")
                .and_then(|v| v.as_str())
                .unwrap_or("0x0");
            (from_s, to_s, value_s)
        }
        other => {
            tracing::debug!(trace_type = other, "alchemy: unknown trace type, skip");
            return Ok(None);
        }
    };

    let raw_val = parse_hex_u256(value_s).context("trace: value")?;
    if raw_val.is_zero() {
        return Ok(None);
    }

    let tx_hash_s = raw
        .get("transactionHash")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("trace: missing transactionHash"))?;
    let block_num = raw
        .get("blockNumber")
        .and_then(|v| v.as_u64())
        .or_else(|| {
            raw.get("blockNumber")
                .and_then(|v| v.as_str())
                .and_then(|s| parse_hex_u64(s).ok())
        })
        .ok_or_else(|| anyhow!("trace: missing blockNumber"))?;
    let block_hash_s = raw.get("blockHash").and_then(|v| v.as_str());

    let tx_hash = parse_hash32(tx_hash_s).context("trace: tx hash")?;
    let block_hash = block_hash_s
        .map(parse_hash32)
        .transpose()
        .context("trace: block hash")?
        .unwrap_or(tx_hash);

    let from = parse_eth_address(chain, from_s).context("trace: from")?;
    let to = parse_eth_address(chain, to_s).context("trace: to")?;

    let timestamp = match block_ts.get(&block_num) {
        Some(t) => *t,
        None => chrono::Utc.timestamp_opt(0, 0).single().unwrap(),
    };

    // Outer trace (traceAddress == []) covers the outer tx value transfer
    // → claim idx=0. Inner traces shift by +1 per tx so they never collide
    // on (chain, tx_hash, idx).
    let trace_addr_empty = raw
        .get("traceAddress")
        .and_then(|v| v.as_array())
        .map(|a| a.is_empty())
        .unwrap_or(true);
    let idx = if trace_addr_empty && trace_type == "call" {
        // First reservation per tx.
        by_tx.entry(tx_hash).or_insert(0);
        0
    } else {
        let position = by_tx.entry(tx_hash).or_insert(0);
        let i = position.saturating_add(1);
        *position += 1;
        i
    };

    Ok(Some(Transfer::new(
        TransferId::new(chain, tx_hash, idx),
        chain,
        TxRef::new(chain, tx_hash),
        from,
        to,
        AssetId::native(chain),
        Amount::new(raw_val, 18),
        BlockRef::new(chain, block_num, block_hash),
        timestamp,
        TransferKind::Native,
        Finality::Confirmed,
    )))
}

/// Map a single `eth_getBlockByNumber(includeTxs=true)` transaction row
/// into a native Transfer. Filters:
/// * Contract-creation tx (`to == null`) — value still moves into the new
///   contract; pull the destination from the receipt only if we had it.
///   In this MVP we drop creation txs (the inner `transfers_for_address`
///   trace_filter pathway picks them up via the create trace).
/// * Value == 0 — skip (pure contract call).
pub(super) fn map_native_tx_to_transfer(
    chain: ChainId,
    raw: &serde_json::Value,
    block_number: u64,
    block_hash: [u8; 32],
    timestamp: chrono::DateTime<chrono::Utc>,
    by_tx: &mut HashMap<[u8; 32], u32>,
) -> anyhow::Result<Option<Transfer>> {
    use anyhow::{Context, anyhow};

    let to_s = match raw.get("to").and_then(|v| v.as_str()).filter(|s| !s.is_empty()) {
        Some(s) => s,
        // Contract creation — outer tx's value goes into the deployed
        // contract, but `to` is null on the body. Skip; the trace-filter
        // path captures the same transfer via a CREATE row.
        None => return Ok(None),
    };
    let value_s = raw
        .get("value")
        .and_then(|v| v.as_str())
        .unwrap_or("0x0");
    let raw_val = parse_hex_u256(value_s).context("tx: value")?;
    if raw_val.is_zero() {
        return Ok(None);
    }
    let from_s = raw
        .get("from")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("tx: missing from"))?;
    let tx_hash_s = raw
        .get("hash")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("tx: missing hash"))?;
    let tx_hash = parse_hash32(tx_hash_s).context("tx: hash")?;
    let from = parse_eth_address(chain, from_s).context("tx: from")?;
    let to = parse_eth_address(chain, to_s).context("tx: to")?;

    // Reserve idx=0 for the outer native transfer — subsequent log/internal
    // entries for the same tx start at idx=1.
    by_tx.entry(tx_hash).or_insert(0);

    Ok(Some(Transfer::new(
        TransferId::new(chain, tx_hash, 0),
        chain,
        TxRef::new(chain, tx_hash),
        from,
        to,
        AssetId::native(chain),
        Amount::new(raw_val, 18),
        BlockRef::new(chain, block_number, block_hash),
        timestamp,
        TransferKind::Native,
        Finality::Confirmed,
    )))
}

/// Map an Alchemy `eth_getLogs` row (ERC-20 Transfer) into a Transfer.
/// Filters ERC-721 by requiring exactly 3 topics (event sig + from + to).
pub(super) fn map_log_to_transfer(
    chain: ChainId,
    raw: &serde_json::Value,
    block_ts: &HashMap<u64, chrono::DateTime<chrono::Utc>>,
    by_tx: &mut HashMap<[u8; 32], u32>,
) -> anyhow::Result<Option<Transfer>> {
    use anyhow::{Context, anyhow};

    let topics = raw
        .get("topics")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("log: missing topics"))?;
    if topics.len() != 3 {
        // ERC-721 (4 topics) or non-Transfer event reaching our filter.
        return Ok(None);
    }
    let from_topic = topics[1].as_str().ok_or_else(|| anyhow!("log: topic1 not str"))?;
    let to_topic = topics[2].as_str().ok_or_else(|| anyhow!("log: topic2 not str"))?;
    let from = parse_eth_address(chain, &unpad_address(from_topic))
        .context("log: from address")?;
    let to = parse_eth_address(chain, &unpad_address(to_topic))
        .context("log: to address")?;

    let data = raw.get("data").and_then(|v| v.as_str()).unwrap_or("0x");
    let raw_val = parse_hex_u256(data).context("log: data")?;
    if raw_val.is_zero() {
        return Ok(None);
    }

    let contract_s = raw
        .get("address")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("log: missing address"))?;
    let contract = parse_eth_address(chain, contract_s).context("log: contract")?;

    let tx_hash_s = raw
        .get("transactionHash")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("log: missing transactionHash"))?;
    let block_num_s = raw
        .get("blockNumber")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("log: missing blockNumber"))?;
    let block_hash_s = raw.get("blockHash").and_then(|v| v.as_str());

    let tx_hash = parse_hash32(tx_hash_s).context("log: tx hash")?;
    let block_hash = block_hash_s
        .map(parse_hash32)
        .transpose()
        .context("log: block hash")?
        .unwrap_or(tx_hash);
    let block_num = parse_hex_u64(block_num_s).context("log: blockNumber")?;

    let timestamp = match block_ts.get(&block_num) {
        Some(t) => *t,
        None => chrono::Utc.timestamp_opt(0, 0).single().unwrap(),
    };

    let position = by_tx.entry(tx_hash).or_insert(0);
    let idx = position.saturating_add(1);
    *position += 1;

    Ok(Some(Transfer::new(
        TransferId::new(chain, tx_hash, idx),
        chain,
        TxRef::new(chain, tx_hash),
        from,
        to,
        AssetId::contract(chain, contract.bytes().to_vec()),
        // Decimals are unknown from the Transfer event alone; default to 18
        // and let downstream asset enrichment correct it. Same compromise
        // that the moralis/bigquery sources make on missing-decimal events.
        Amount::new(raw_val, 18),
        BlockRef::new(chain, block_num, block_hash),
        timestamp,
        TransferKind::Token {
            contract,
            standard: TokenStandard::Erc20,
            symbol: None,
        },
        Finality::Confirmed,
    )))
}

/// Heuristic match for Alchemy responses that signal an `eth_getLogs` window
/// exceeded the response-size cap and needs to be subdivided. The exact
/// wording has drifted between Alchemy versions ("query returned more than
/// 10000 results", "log response size exceeded", "response size too large",
/// "response size limit"); accepting any of these — but nothing else —
/// keeps the bisection branch firing on real overflow without swallowing
/// unrelated errors that mention "log".
pub(super) fn is_log_response_too_large(msg: &str) -> bool {
    let l = msg.to_ascii_lowercase();
    l.contains("log response size exceeded")
        || l.contains("response size exceeded")
        || l.contains("response size too large")
        || l.contains("response size limit")
        || l.contains("query returned more than")
        || l.contains("too many results")
        || l.contains("result window")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_transfer_topics_full_filter() {
        let v = build_transfer_topics(Some("0xfrom"), Some("0xto"));
        assert_eq!(v[0], TRANSFER_TOPIC);
        assert_eq!(v[1], "0xfrom");
        assert_eq!(v[2], "0xto");
    }

    #[test]
    fn build_transfer_topics_from_only() {
        let v = build_transfer_topics(Some("0xfrom"), None);
        assert_eq!(v[1], "0xfrom");
        assert!(v[2].is_null());
    }

    #[test]
    fn bisection_trigger_matches_known_alchemy_phrasings() {
        // The canonical wordings we've observed from Alchemy across versions.
        assert!(is_log_response_too_large(
            "alchemy eth_getLogs rpc error -32602: query returned more than 10000 results"
        ));
        assert!(is_log_response_too_large(
            "Log response size exceeded. You can make eth_getLogs requests with up to a 2K block range."
        ));
        assert!(is_log_response_too_large(
            "response size exceeded the limit"
        ));
        assert!(is_log_response_too_large("response size too large"));
        assert!(is_log_response_too_large(
            "Response size limit reached, please use a smaller range"
        ));
        assert!(is_log_response_too_large("too many results"));
        // Newer wording observed in Alchemy proxy:
        assert!(is_log_response_too_large("result window too large"));
    }

    #[test]
    fn bisection_trigger_ignores_unrelated_errors() {
        // The previous heuristic was `contains("log")` which fires on
        // *anything* mentioning logging — including innocuous library
        // errors. The hardened matcher must NOT trip on these.
        assert!(!is_log_response_too_large(
            "tracing log subscriber failed to initialize"
        ));
        assert!(!is_log_response_too_large(
            "method eth_getLogs not enabled on this endpoint"
        ));
        assert!(!is_log_response_too_large(
            "invalid argument 0: hex string of odd length"
        ));
        assert!(!is_log_response_too_large("rate limit reached"));
        assert!(!is_log_response_too_large(""));
    }
}
