use async_trait::async_trait;
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;

use domain::chain::ChainId;
use domain::entity::AddressKind;
use domain::error::DomainResult;
use domain::ports::{
    AddressKindRepository, Alert, AlertSink, WatchlistEntry, WatchlistRepository,
};
use domain::primitives::Address;

use crate::{pg_err, pool_err};

pub struct PostgresWatchlist {
    pool: Pool,
}

impl PostgresWatchlist {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WatchlistRepository for PostgresWatchlist {
    async fn add(&self, entry: WatchlistEntry) -> DomainResult<()> {
        let client = self.pool.get().await.map_err(pool_err)?;
        client
            .execute(
                "INSERT INTO watchlist (chain_id, address, reason)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (chain_id, address) DO UPDATE
                    SET reason = EXCLUDED.reason",
                &[
                    &(entry.address.chain().value() as i32),
                    &entry.address.bytes(),
                    &entry.reason,
                ],
            )
            .await
            .map_err(pg_err)?;
        Ok(())
    }

    async fn remove(&self, addr: &Address) -> DomainResult<bool> {
        let client = self.pool.get().await.map_err(pool_err)?;
        let n = client
            .execute(
                "DELETE FROM watchlist WHERE chain_id = $1 AND address = $2",
                &[&(addr.chain().value() as i32), &addr.bytes()],
            )
            .await
            .map_err(pg_err)?;
        Ok(n > 0)
    }

    async fn list(&self) -> DomainResult<Vec<WatchlistEntry>> {
        let client = self.pool.get().await.map_err(pool_err)?;
        let rows = client
            .query("SELECT chain_id, address, reason FROM watchlist", &[])
            .await
            .map_err(pg_err)?;
        rows.iter()
            .map(|r| {
                let chain = ChainId::new(r.get::<_, i32>("chain_id") as u32);
                let bytes: Vec<u8> = r.get("address");
                Ok(WatchlistEntry {
                    address: Address::new(chain, bytes),
                    reason: r.get::<_, Option<String>>("reason"),
                })
            })
            .collect()
    }

    async fn contains(&self, addr: &Address) -> DomainResult<bool> {
        let client = self.pool.get().await.map_err(pool_err)?;
        let row = client
            .query_opt(
                "SELECT 1 FROM watchlist WHERE chain_id = $1 AND address = $2",
                &[&(addr.chain().value() as i32), &addr.bytes()],
            )
            .await
            .map_err(pg_err)?;
        Ok(row.is_some())
    }
}

pub struct PostgresAlerts {
    pool: Pool,
}

impl PostgresAlerts {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AlertSink for PostgresAlerts {
    async fn record(&self, alert: Alert) -> DomainResult<()> {
        let client = self.pool.get().await.map_err(pool_err)?;
        client
            .execute(
                "INSERT INTO alerts (chain_id, address, tx_hash, tx_idx, reason, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6)",
                &[
                    &(alert.address.chain().value() as i32),
                    &alert.address.bytes(),
                    &&alert.triggered_by_tx[..],
                    &(alert.triggered_by_idx as i32),
                    &alert.reason,
                    &alert.created_at,
                ],
            )
            .await
            .map_err(pg_err)?;
        Ok(())
    }

    async fn list(&self) -> DomainResult<Vec<Alert>> {
        let client = self.pool.get().await.map_err(pool_err)?;
        let rows = client
            .query(
                "SELECT chain_id, address, tx_hash, tx_idx, reason, created_at
                 FROM alerts
                 ORDER BY created_at DESC
                 LIMIT 1000",
                &[],
            )
            .await
            .map_err(pg_err)?;
        rows.iter()
            .map(|r| {
                let chain = ChainId::new(r.get::<_, i32>("chain_id") as u32);
                let bytes: Vec<u8> = r.get("address");
                let mut tx_hash = [0u8; 32];
                let raw: Vec<u8> = r.get("tx_hash");
                let n = raw.len().min(32);
                tx_hash[..n].copy_from_slice(&raw[..n]);
                let created_at: DateTime<Utc> = r.get("created_at");
                Ok(Alert {
                    address: Address::new(chain, bytes),
                    triggered_by_tx: tx_hash,
                    triggered_by_idx: r.get::<_, i32>("tx_idx") as u32,
                    reason: r.get::<_, Option<String>>("reason"),
                    created_at,
                })
            })
            .collect()
    }
}

pub struct PostgresAddressKinds {
    pool: Pool,
}

impl PostgresAddressKinds {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AddressKindRepository for PostgresAddressKinds {
    async fn kind(&self, addr: &Address) -> DomainResult<AddressKind> {
        let client = self.pool.get().await.map_err(pool_err)?;
        let row = client
            .query_opt(
                "SELECT kind, service_name FROM address_kind
                 WHERE chain_id = $1 AND address = $2",
                &[&(addr.chain().value() as i32), &addr.bytes()],
            )
            .await
            .map_err(pg_err)?;
        Ok(match row {
            None => AddressKind::Unknown,
            Some(r) => match r.get::<_, &str>("kind") {
                "eoa" => AddressKind::Eoa,
                "contract" => AddressKind::Contract,
                "known_service" => AddressKind::KnownService(
                    r.get::<_, Option<String>>("service_name")
                        .unwrap_or_default(),
                ),
                _ => AddressKind::Unknown,
            },
        })
    }

    async fn set_kind(&self, addr: &Address, kind: AddressKind) -> DomainResult<()> {
        let client = self.pool.get().await.map_err(pool_err)?;
        let (kind_s, name) = match &kind {
            AddressKind::Eoa => ("eoa", None),
            AddressKind::Contract => ("contract", None),
            AddressKind::KnownService(n) => ("known_service", Some(n.clone())),
            AddressKind::Unknown => ("unknown", None),
        };
        client
            .execute(
                "INSERT INTO address_kind (chain_id, address, kind, service_name)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT (chain_id, address) DO UPDATE
                    SET kind = EXCLUDED.kind,
                        service_name = EXCLUDED.service_name,
                        updated_at = now()",
                &[
                    &(addr.chain().value() as i32),
                    &addr.bytes(),
                    &kind_s,
                    &name,
                ],
            )
            .await
            .map_err(pg_err)?;
        Ok(())
    }

    /// One round-trip per N addresses. Order of the returned vector
    /// matches `addrs` exactly; rows that don't exist in `address_kind`
    /// become `Unknown` in the output (the caller's expected default).
    async fn kind_batch(&self, addrs: &[Address]) -> DomainResult<Vec<AddressKind>> {
        if addrs.is_empty() {
            return Ok(Vec::new());
        }
        let client = self.pool.get().await.map_err(pool_err)?;

        // Build positional inputs. `chain_id` happens to be the same for
        // every entry in any call shape we'll ever make, but staying
        // chain-agnostic in the query lets us mix chains in one call later.
        let chain_ids: Vec<i32> = addrs.iter().map(|a| a.chain().value() as i32).collect();
        let address_bytes: Vec<&[u8]> = addrs.iter().map(|a| a.bytes()).collect();

        // Use UNNEST so we keep input order via JOIN — straight ANY() can
        // give back rows in arbitrary order, and Postgres doesn't preserve
        // the array index for free.
        let rows = client
            .query(
                "WITH inputs AS (
                    SELECT unnest($1::INT[]) AS chain_id,
                           unnest($2::BYTEA[]) AS address,
                           generate_subscripts($1::INT[], 1) AS ord
                 )
                 SELECT i.ord, k.kind, k.service_name
                 FROM inputs i
                 LEFT JOIN address_kind k
                   ON k.chain_id = i.chain_id AND k.address = i.address
                 ORDER BY i.ord",
                &[&chain_ids, &address_bytes],
            )
            .await
            .map_err(pg_err)?;

        let mut out = Vec::with_capacity(addrs.len());
        for row in rows {
            let kind_s: Option<&str> = row.get("kind");
            let name: Option<String> = row.get("service_name");
            let parsed = match kind_s {
                None => AddressKind::Unknown,
                Some("eoa") => AddressKind::Eoa,
                Some("contract") => AddressKind::Contract,
                Some("known_service") => AddressKind::KnownService(name.unwrap_or_default()),
                Some(_) => AddressKind::Unknown,
            };
            out.push(parsed);
        }
        Ok(out)
    }

    /// One INSERT per N rows via UNNEST. Conflict resolution mirrors the
    /// single-row path so we never silently downgrade a `KnownService` to
    /// `eoa`/`contract` (the trigger of that incident lives in tests).
    async fn set_kind_batch(
        &self,
        entries: &[(Address, AddressKind)],
    ) -> DomainResult<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let client = self.pool.get().await.map_err(pool_err)?;

        let chain_ids: Vec<i32> = entries
            .iter()
            .map(|(a, _)| a.chain().value() as i32)
            .collect();
        let address_bytes: Vec<&[u8]> = entries.iter().map(|(a, _)| a.bytes()).collect();
        let kinds: Vec<&str> = entries
            .iter()
            .map(|(_, k)| match k {
                AddressKind::Eoa => "eoa",
                AddressKind::Contract => "contract",
                AddressKind::KnownService(_) => "known_service",
                AddressKind::Unknown => "unknown",
            })
            .collect();
        let names: Vec<Option<String>> = entries
            .iter()
            .map(|(_, k)| match k {
                AddressKind::KnownService(n) => Some(n.clone()),
                _ => None,
            })
            .collect();

        client
            .execute(
                "INSERT INTO address_kind (chain_id, address, kind, service_name)
                 SELECT * FROM UNNEST($1::INT[], $2::BYTEA[], $3::TEXT[], $4::TEXT[])
                 ON CONFLICT (chain_id, address) DO UPDATE
                    SET kind = EXCLUDED.kind,
                        service_name = EXCLUDED.service_name,
                        updated_at = now()",
                &[&chain_ids, &address_bytes, &kinds, &names],
            )
            .await
            .map_err(pg_err)?;
        Ok(())
    }
}
