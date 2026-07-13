use std::collections::HashMap;

use chrono::TimeZone;

use domain::{
    asset::{AssetId, TokenStandard},
    chain::ChainId,
    primitives::{Amount, BlockRef, TxRef, U256},
    transfer::{Finality, Transfer, TransferId, TransferKind},
};

use super::parse::{parse_eth_address, parse_hash32};

pub(super) fn map_native(chain: ChainId, raw: &serde_json::Value) -> anyhow::Result<Option<Transfer>> {
    use anyhow::{Context, anyhow};

    if raw.get("isError").and_then(|v| v.as_str()) == Some("1") {
        return Ok(None);
    }
    let value_s = raw
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("native: missing value"))?;
    let raw_val = U256::from_dec_str(value_s).context("native: value")?;
    if raw_val.is_zero() {
        return Ok(None);
    }

    let to_s = raw
        .get("to")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let Some(to_s) = to_s else {
        return Ok(None);
    };
    let from_s = raw
        .get("from")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("native: missing from"))?;
    let tx_hash_s = raw
        .get("hash")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("native: missing hash"))?;
    let block_num_s = raw
        .get("blockNumber")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("native: missing blockNumber"))?;
    let ts_s = raw
        .get("timeStamp")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("native: missing timeStamp"))?;
    let block_hash_s = raw.get("blockHash").and_then(|v| v.as_str());

    let tx_hash = parse_hash32(tx_hash_s).context("native: tx hash")?;
    let block_hash = block_hash_s
        .map(parse_hash32)
        .transpose()
        .context("native: block_hash")?
        .unwrap_or(tx_hash);
    let block_number: u64 = block_num_s.parse().context("native: blockNumber")?;
    let ts_secs: i64 = ts_s.parse().context("native: timeStamp")?;
    let timestamp = chrono::Utc
        .timestamp_opt(ts_secs, 0)
        .single()
        .ok_or_else(|| anyhow!("native: bad timestamp {ts_secs}"))?;

    let from = parse_eth_address(chain, from_s).context("native: from")?;
    let to = parse_eth_address(chain, to_s).context("native: to")?;

    let finality = match raw.get("txreceipt_status").and_then(|v| v.as_str()) {
        Some("0") => Finality::Reorged,
        _ => Finality::Confirmed,
    };

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
        finality,
    )))
}

/// Map a `txlistinternal` row into a Transfer. Internal rows describe value
/// movements driven by `CALL`/`CALLCODE`/`CREATE*`/`SELFDESTRUCT` opcodes
/// during contract execution — i.e. the ones missing from `txlist`.
///
/// Filtering:
/// * `isError == "1"` — the sub-call reverted, no value moved.
/// * `type == "delegatecall" | "staticcall"` — these never transfer value
///   regardless of the `value` field (delegatecall preserves caller context,
///   staticcall forbids state changes).
/// * `value == 0` — nothing to record.
///
/// `to` is empty for `create`/`create2` rows; in that case the destination
/// is the freshly-deployed contract address sitting in `contractAddress`.
pub(super) fn map_internal(
    chain: ChainId,
    raw: &serde_json::Value,
    by_tx: &mut HashMap<[u8; 32], u32>,
) -> anyhow::Result<Option<Transfer>> {
    use anyhow::{Context, anyhow};

    if raw.get("isError").and_then(|v| v.as_str()) == Some("1") {
        return Ok(None);
    }
    if matches!(
        raw.get("type").and_then(|v| v.as_str()),
        Some("delegatecall") | Some("staticcall")
    ) {
        return Ok(None);
    }

    let value_s = raw
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("internal: missing value"))?;
    let raw_val = U256::from_dec_str(value_s).context("internal: value")?;
    if raw_val.is_zero() {
        return Ok(None);
    }

    let from_s = raw
        .get("from")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("internal: missing from"))?;
    let to_s = raw
        .get("to")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let dest_s = match to_s {
        Some(s) => s.to_string(),
        None => raw
            .get("contractAddress")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty() && *s != "0x")
            .ok_or_else(|| anyhow!("internal: missing to/contractAddress"))?
            .to_string(),
    };

    let tx_hash_s = raw
        .get("hash")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("internal: missing hash"))?;
    let block_num_s = raw
        .get("blockNumber")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("internal: missing blockNumber"))?;
    let ts_s = raw
        .get("timeStamp")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("internal: missing timeStamp"))?;

    let tx_hash = parse_hash32(tx_hash_s).context("internal: tx hash")?;
    let block_number: u64 = block_num_s.parse().context("internal: blockNumber")?;
    let ts_secs: i64 = ts_s.parse().context("internal: timeStamp")?;
    let timestamp = chrono::Utc
        .timestamp_opt(ts_secs, 0)
        .single()
        .ok_or_else(|| anyhow!("internal: bad timestamp {ts_secs}"))?;

    let from = parse_eth_address(chain, from_s).context("internal: from")?;
    let to = parse_eth_address(chain, &dest_s).context("internal: to")?;

    // idx=0 is reserved for the outer native value transfer (txlist row);
    // bump per row within the same tx — shared counter with token rows in
    // the caller, so internal+token never collide on (chain, tx_hash, idx).
    let position = by_tx.entry(tx_hash).or_insert(0);
    let idx = position.saturating_add(1);
    *position += 1;

    Ok(Some(Transfer::new(
        TransferId::new(chain, tx_hash, idx),
        chain,
        TxRef::new(chain, tx_hash),
        from,
        to,
        AssetId::native(chain),
        Amount::new(raw_val, 18),
        // Internal rows don't carry blockHash; fall back to tx_hash like
        // map_native does for the same reason. Reorg classification is
        // driven by block height + confirmation_depth, not by hash equality.
        BlockRef::new(chain, block_number, tx_hash),
        timestamp,
        TransferKind::Native,
        Finality::Confirmed,
    )))
}

pub(super) fn map_token(
    chain: ChainId,
    raw: &serde_json::Value,
    by_tx: &mut HashMap<[u8; 32], u32>,
) -> anyhow::Result<Option<Transfer>> {
    use anyhow::{Context, anyhow};

    let from_s = raw
        .get("from")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("token: missing from"))?;
    let to_s = raw
        .get("to")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    let Some(to_s) = to_s else {
        return Ok(None);
    };
    let contract_s = raw
        .get("contractAddress")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("token: missing contractAddress"))?;
    let value_s = raw
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("token: missing value"))?;
    let tx_hash_s = raw
        .get("hash")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("token: missing hash"))?;
    let block_num_s = raw
        .get("blockNumber")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("token: missing blockNumber"))?;
    let ts_s = raw
        .get("timeStamp")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("token: missing timeStamp"))?;
    let block_hash_s = raw.get("blockHash").and_then(|v| v.as_str());
    let decimals_s = raw.get("tokenDecimal").and_then(|v| v.as_str());
    let symbol = raw
        .get("tokenSymbol")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let tx_hash = parse_hash32(tx_hash_s).context("token: tx hash")?;
    let block_hash = block_hash_s
        .map(parse_hash32)
        .transpose()
        .context("token: block_hash")?
        .unwrap_or(tx_hash);
    let block_number: u64 = block_num_s.parse().context("token: blockNumber")?;
    let ts_secs: i64 = ts_s.parse().context("token: timeStamp")?;
    let timestamp = chrono::Utc
        .timestamp_opt(ts_secs, 0)
        .single()
        .ok_or_else(|| anyhow!("token: bad timestamp {ts_secs}"))?;
    let decimals: u8 = decimals_s
        .map(|s| s.parse::<u8>())
        .transpose()
        .context("token: tokenDecimal")?
        .unwrap_or(18);
    let raw_val = U256::from_dec_str(value_s).context("token: value")?;

    let from = parse_eth_address(chain, from_s).context("token: from")?;
    let to = parse_eth_address(chain, to_s).context("token: to")?;
    let contract = parse_eth_address(chain, contract_s).context("token: contractAddress")?;

    // idx=0 is reserved for the (single) native transfer in this tx; shift
    // token rows by +1 so they never collide on the (chain, tx_hash, idx) PK
    // when a tx has both a native value transfer and an ERC-20 Transfer event.
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
        Amount::new(raw_val, decimals),
        BlockRef::new(chain, block_number, block_hash),
        timestamp,
        TransferKind::Token {
            contract,
            standard: TokenStandard::Erc20,
            symbol,
        },
        Finality::Confirmed,
    )))
}
