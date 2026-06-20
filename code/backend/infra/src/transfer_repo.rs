use async_trait::async_trait;
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;

use domain::asset::{AssetId, AssetKind, TokenStandard};
use domain::chain::ChainId;
use domain::error::{DomainError, DomainResult};
use domain::ports::{BlockRange, TransferRepository};
use domain::primitives::{Address, Amount, BlockRef, TxRef, U256};
use domain::transfer::{Finality, Transfer, TransferId, TransferKind};

use crate::{pg_err, pool_err};

fn addr_str(addr: &Address) -> String {
    format!("0x{}", hex::encode(addr.bytes()))
}

const COLS: &str = "chain_id, tx_hash, idx, from_addr, to_addr, asset_contract, \
                    amount::text AS amount, decimals, block_height, block_hash, \
                    ts, kind, token_standard, vin_idx, vout_idx, finality";

pub struct PostgresTransferRepository {
    pool: Pool,
}

impl PostgresTransferRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
}

impl Clone for PostgresTransferRepository {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
        }
    }
}

#[async_trait]
impl TransferRepository for PostgresTransferRepository {
    async fn save(&self, transfers: &[Transfer]) -> DomainResult<()> {
        if transfers.is_empty() {
            tracing::debug!("save called with 0 transfers, skipping");
            return Ok(());
        }
        tracing::debug!(count = transfers.len(), "saving transfers to postgres");
        let client = self.pool.get().await.map_err(pool_err)?;

        let n = transfers.len();
        let mut chain_ids: Vec<i32> = Vec::with_capacity(n);
        let mut tx_hashes: Vec<Vec<u8>> = Vec::with_capacity(n);
        let mut idxs: Vec<i32> = Vec::with_capacity(n);
        let mut froms: Vec<Vec<u8>> = Vec::with_capacity(n);
        let mut tos: Vec<Vec<u8>> = Vec::with_capacity(n);
        let mut contracts: Vec<Option<Vec<u8>>> = Vec::with_capacity(n);
        let mut amounts: Vec<String> = Vec::with_capacity(n);
        let mut decimals: Vec<i16> = Vec::with_capacity(n);
        let mut heights: Vec<i64> = Vec::with_capacity(n);
        let mut block_hashes: Vec<Vec<u8>> = Vec::with_capacity(n);
        let mut timestamps: Vec<DateTime<Utc>> = Vec::with_capacity(n);
        let mut kinds: Vec<&'static str> = Vec::with_capacity(n);
        let mut standards: Vec<Option<String>> = Vec::with_capacity(n);
        let mut vin_idxs: Vec<Option<i32>> = Vec::with_capacity(n);
        let mut vout_idxs: Vec<Option<i32>> = Vec::with_capacity(n);
        let mut finalities: Vec<&'static str> = Vec::with_capacity(n);

        for t in transfers {
            chain_ids.push(t.chain().value() as i32);
            tx_hashes.push(t.tx_ref().hash().to_vec());
            idxs.push(t.id().index() as i32);
            froms.push(t.from().bytes().to_vec());
            tos.push(t.to().bytes().to_vec());
            contracts.push(match t.asset().kind() {
                AssetKind::Contract(b) => Some(b.clone()),
                _ => None,
            });
            amounts.push(t.amount().raw().to_string());
            decimals.push(t.amount().decimals() as i16);
            heights.push(t.block().height() as i64);
            block_hashes.push(t.block().hash().to_vec());
            timestamps.push(t.timestamp());
            kinds.push(match t.kind() {
                TransferKind::Native => "native",
                TransferKind::Internal => "internal",
                TransferKind::Token { .. } => "token",
                TransferKind::Fee => "fee",
                TransferKind::UtxoEdge { .. } => "utxo_edge",
            });
            standards.push(match t.kind() {
                TransferKind::Token { standard, .. } => {
                    Some(format!("{standard:?}").to_lowercase())
                }
                _ => None,
            });
            let (vin_i, vout_i) = match t.kind() {
                TransferKind::UtxoEdge {
                    vin_index,
                    vout_index,
                } => (Some(*vin_index as i32), Some(*vout_index as i32)),
                _ => (None, None),
            };
            vin_idxs.push(vin_i);
            vout_idxs.push(vout_i);
            finalities.push(match t.finality() {
                Finality::Confirmed => "confirmed",
                Finality::Unconfirmed => "unconfirmed",
                Finality::Reorged => "reorged",
                Finality::Pending { .. } => "pending",
            });
        }

        client
            .execute(
                "INSERT INTO transfers (chain_id, tx_hash, idx, from_addr, to_addr,
                                        asset_contract, amount, decimals, block_height,
                                        block_hash, ts, kind, token_standard,
                                        vin_idx, vout_idx, finality)
                 SELECT u.chain_id, u.tx_hash, u.idx, u.from_addr, u.to_addr,
                        u.asset_contract, u.amount::numeric, u.decimals, u.block_height,
                        u.block_hash, u.ts, u.kind, u.token_standard,
                        u.vin_idx, u.vout_idx, u.finality
                 FROM UNNEST($1::int4[], $2::bytea[], $3::int4[], $4::bytea[], $5::bytea[],
                             $6::bytea[], $7::text[], $8::int2[], $9::int8[],
                             $10::bytea[], $11::timestamptz[], $12::text[], $13::text[],
                             $14::int4[], $15::int4[], $16::text[])
                 AS u(chain_id, tx_hash, idx, from_addr, to_addr, asset_contract, amount,
                      decimals, block_height, block_hash, ts, kind, token_standard,
                      vin_idx, vout_idx, finality)
                 ON CONFLICT (chain_id, tx_hash, idx) DO UPDATE
                    SET from_addr      = EXCLUDED.from_addr,
                        to_addr        = EXCLUDED.to_addr,
                        asset_contract = EXCLUDED.asset_contract,
                        amount         = EXCLUDED.amount,
                        decimals       = EXCLUDED.decimals,
                        block_height   = EXCLUDED.block_height,
                        block_hash     = EXCLUDED.block_hash,
                        ts             = EXCLUDED.ts,
                        kind           = EXCLUDED.kind,
                        token_standard = EXCLUDED.token_standard,
                        vin_idx        = EXCLUDED.vin_idx,
                        vout_idx       = EXCLUDED.vout_idx,
                        finality       = EXCLUDED.finality",
                &[
                    &chain_ids,
                    &tx_hashes,
                    &idxs,
                    &froms,
                    &tos,
                    &contracts,
                    &amounts,
                    &decimals,
                    &heights,
                    &block_hashes,
                    &timestamps,
                    &kinds,
                    &standards,
                    &vin_idxs,
                    &vout_idxs,
                    &finalities,
                ],
            )
            .await
            .map_err(pg_err)?;

        tracing::debug!(count = transfers.len(), "transfers saved");
        Ok(())
    }

    async fn find_by_address(
        &self,
        addr: &Address,
        range: Option<BlockRange>,
    ) -> DomainResult<Vec<Transfer>> {
        let client = self.pool.get().await.map_err(pool_err)?;

        let (from_h, to_h) = match range {
            Some(r) => (
                r.from_height().min(i64::MAX as u64) as i64,
                r.to_height().min(i64::MAX as u64) as i64,
            ),
            None => (0_i64, i64::MAX),
        };

        tracing::debug!(address = %addr_str(addr), from_block = from_h, to_block = to_h, "find_by_address");

        let rows = client
            .query(
                &format!(
                    "SELECT {COLS} FROM transfers
                     WHERE chain_id = $1
                       AND (from_addr = $2 OR to_addr = $2)
                       AND block_height BETWEEN $3 AND $4
                     ORDER BY block_height, idx"
                ),
                &[
                    &(addr.chain().value() as i32),
                    &addr.bytes(),
                    &from_h,
                    &to_h,
                ],
            )
            .await
            .map_err(pg_err)?;

        tracing::debug!(address = %addr_str(addr), count = rows.len(), "find_by_address result");
        rows.iter().map(row_to_transfer).collect()
    }

    async fn find_by_tx(&self, chain: ChainId, tx_hash: &[u8; 32]) -> DomainResult<Vec<Transfer>> {
        let client = self.pool.get().await.map_err(pool_err)?;

        tracing::debug!(chain = chain.value(), tx = %hex::encode(tx_hash), "find_by_tx");

        let rows = client
            .query(
                &format!(
                    "SELECT {COLS} FROM transfers
                     WHERE chain_id = $1 AND tx_hash = $2
                     ORDER BY idx"
                ),
                &[&(chain.value() as i32), &&tx_hash[..]],
            )
            .await
            .map_err(pg_err)?;

        tracing::debug!(tx = %hex::encode(tx_hash), count = rows.len(), "find_by_tx result");
        rows.iter().map(row_to_transfer).collect()
    }

    async fn find_outgoing(
        &self,
        addr: &Address,
        after: Option<DateTime<Utc>>,
    ) -> DomainResult<Vec<Transfer>> {
        self.find_directed(addr, after, true).await
    }

    async fn find_incoming(
        &self,
        addr: &Address,
        after: Option<DateTime<Utc>>,
    ) -> DomainResult<Vec<Transfer>> {
        self.find_directed(addr, after, false).await
    }

    async fn max_block_height(&self, addr: &Address) -> DomainResult<Option<u64>> {
        let client = self.pool.get().await.map_err(pool_err)?;
        let row = client
            .query_one(
                "SELECT MAX(block_height) AS h FROM transfers
                 WHERE chain_id = $1 AND (from_addr = $2 OR to_addr = $2)",
                &[&(addr.chain().value() as i32), &addr.bytes()],
            )
            .await
            .map_err(pg_err)?;
        let h: Option<i64> = row.get("h");
        Ok(h.map(|v| v.max(0) as u64))
    }

    async fn delete_in_range(
        &self,
        addr: &Address,
        from_block: u64,
        to_block: u64,
    ) -> DomainResult<u64> {
        let client = self.pool.get().await.map_err(pool_err)?;
        let from_h = from_block.min(i64::MAX as u64) as i64;
        let to_h = to_block.min(i64::MAX as u64) as i64;
        let affected = client
            .execute(
                "DELETE FROM transfers
                 WHERE chain_id = $1
                   AND (from_addr = $2 OR to_addr = $2)
                   AND block_height BETWEEN $3 AND $4",
                &[
                    &(addr.chain().value() as i32),
                    &addr.bytes(),
                    &from_h,
                    &to_h,
                ],
            )
            .await
            .map_err(pg_err)?;
        Ok(affected)
    }
}

impl PostgresTransferRepository {
    async fn find_directed(
        &self,
        addr: &Address,
        after: Option<DateTime<Utc>>,
        outgoing: bool,
    ) -> DomainResult<Vec<Transfer>> {
        let direction = if outgoing { "outgoing" } else { "incoming" };
        let col = if outgoing { "from_addr" } else { "to_addr" };
        let client = self.pool.get().await.map_err(pool_err)?;

        tracing::debug!(address = %addr_str(addr), direction, after = ?after, "find_directed");

        let rows = client
            .query(
                &format!(
                    "SELECT {COLS} FROM transfers
                     WHERE chain_id = $1
                       AND {col} = $2
                       AND ($3::timestamptz IS NULL OR ts > $3)
                     ORDER BY ts, idx"
                ),
                &[&(addr.chain().value() as i32), &addr.bytes(), &after],
            )
            .await
            .map_err(pg_err)?;

        tracing::debug!(address = %addr_str(addr), direction, count = rows.len(), "find_directed result");
        rows.iter().map(row_to_transfer).collect()
    }
}

fn row_to_transfer(row: &tokio_postgres::Row) -> DomainResult<Transfer> {
    let chain = ChainId::new(row.get::<_, i32>("chain_id") as u32);

    let tx_hash = bytes32(row.get("tx_hash"), "tx_hash")?;
    let idx = row.get::<_, i32>("idx") as u32;

    let from = Address::new(chain, row.get::<_, Vec<u8>>("from_addr"));
    let to = Address::new(chain, row.get::<_, Vec<u8>>("to_addr"));

    let asset_contract: Option<Vec<u8>> = row.get("asset_contract");
    let asset = match &asset_contract {
        Some(b) => AssetId::contract(chain, b.clone()),
        None => AssetId::native(chain),
    };

    let raw = U256::from_dec_str(row.get::<_, &str>("amount"))
        .map_err(|e| DomainError::InsufficientData(format!("amount parse: {e}")))?;
    let amount = Amount::new(raw, row.get::<_, i16>("decimals") as u8);

    let block = BlockRef::new(
        chain,
        row.get::<_, i64>("block_height") as u64,
        bytes32(row.get("block_hash"), "block_hash")?,
    );

    let timestamp: DateTime<Utc> = row.get("ts");

    let kind =
        match row.get::<_, &str>("kind") {
            "native" => TransferKind::Native,
            "internal" => TransferKind::Internal,
            "fee" => TransferKind::Fee,
            "utxo_edge" => {
                let vin_index = row.get::<_, Option<i32>>("vin_idx").ok_or_else(|| {
                    DomainError::InsufficientData("utxo_edge missing vin_idx".into())
                })? as u32;
                let vout_index = row.get::<_, Option<i32>>("vout_idx").ok_or_else(|| {
                    DomainError::InsufficientData("utxo_edge missing vout_idx".into())
                })? as u32;
                TransferKind::UtxoEdge {
                    vin_index,
                    vout_index,
                }
            }
            "token" => {
                let contract_bytes = asset_contract.clone().ok_or_else(|| {
                    DomainError::InsufficientData("token transfer without asset_contract".into())
                })?;
                let standard = match row.get::<_, Option<&str>>("token_standard") {
                    Some("erc20") => TokenStandard::Erc20,
                    Some("erc721") => TokenStandard::Erc721,
                    Some("erc1155") => TokenStandard::Erc1155,
                    Some("trc20") => TokenStandard::Trc20,
                    Some("trc10") => TokenStandard::Trc10,
                    Some("spl") => TokenStandard::Spl,
                    Some(other) => {
                        return Err(DomainError::InsufficientData(format!(
                            "unknown token standard: {other}"
                        )));
                    }
                    None => {
                        return Err(DomainError::InsufficientData(
                            "token transfer without token_standard".into(),
                        ));
                    }
                };
                TransferKind::Token {
                    contract: Address::new(chain, contract_bytes),
                    standard,
                }
            }
            other => {
                return Err(DomainError::InsufficientData(format!(
                    "unknown transfer kind: {other}"
                )));
            }
        };

    let finality = match row.get::<_, &str>("finality") {
        "confirmed" => Finality::Confirmed,
        "unconfirmed" => Finality::Unconfirmed,
        "pending" => Finality::Pending { confirmations: 0 },
        "reorged" => Finality::Reorged,
        other => {
            return Err(DomainError::InsufficientData(format!(
                "unknown finality: {other}"
            )));
        }
    };

    Ok(Transfer::new(
        TransferId::new(chain, tx_hash, idx),
        chain,
        TxRef::new(chain, tx_hash),
        from,
        to,
        asset,
        amount,
        block,
        timestamp,
        kind,
        finality,
    ))
}

fn bytes32(v: Vec<u8>, field: &str) -> DomainResult<[u8; 32]> {
    v.try_into().map_err(|v: Vec<u8>| {
        DomainError::InsufficientData(format!("{field}: expected 32 bytes, got {}", v.len()))
    })
}
