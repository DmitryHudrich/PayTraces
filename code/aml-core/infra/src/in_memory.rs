use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::RwLock;

use domain::entity::{AddressKind, EntityLabel, LabelSource};
use domain::error::DomainResult;
use domain::ports::{
    AddressKindRepository, Alert, AlertSink, LabelProvider, WatchlistEntry, WatchlistRepository,
};
use domain::primitives::Address;

#[derive(Default)]
pub struct InMemoryAddressKinds {
    by_addr: RwLock<HashMap<Address, AddressKind>>,
}

#[async_trait]
impl AddressKindRepository for InMemoryAddressKinds {
    async fn kind(&self, addr: &Address) -> DomainResult<AddressKind> {
        Ok(self
            .by_addr
            .read()
            .unwrap()
            .get(addr)
            .cloned()
            .unwrap_or(AddressKind::Unknown))
    }
    async fn set_kind(&self, addr: &Address, kind: AddressKind) -> DomainResult<()> {
        self.by_addr.write().unwrap().insert(addr.clone(), kind);
        Ok(())
    }
}

#[derive(Default)]
pub struct InMemoryWatchlist {
    entries: RwLock<HashMap<Address, WatchlistEntry>>,
}

#[async_trait]
impl WatchlistRepository for InMemoryWatchlist {
    async fn add(&self, entry: WatchlistEntry) -> DomainResult<()> {
        self.entries
            .write()
            .unwrap()
            .insert(entry.address.clone(), entry);
        Ok(())
    }
    async fn remove(&self, addr: &Address) -> DomainResult<bool> {
        Ok(self.entries.write().unwrap().remove(addr).is_some())
    }
    async fn list(&self) -> DomainResult<Vec<WatchlistEntry>> {
        Ok(self.entries.read().unwrap().values().cloned().collect())
    }
    async fn contains(&self, addr: &Address) -> DomainResult<bool> {
        Ok(self.entries.read().unwrap().contains_key(addr))
    }
}

#[derive(Default)]
pub struct InMemoryAlerts {
    log: RwLock<Vec<Alert>>,
}

#[async_trait]
impl AlertSink for InMemoryAlerts {
    async fn record(&self, alert: Alert) -> DomainResult<()> {
        self.log.write().unwrap().push(alert);
        Ok(())
    }
    async fn list(&self) -> DomainResult<Vec<Alert>> {
        Ok(self.log.read().unwrap().clone())
    }
}

/// In-memory label provider seeded with a tiny baseline list. Suitable for
/// MVP/local-dev — swap for a file-backed or HTTP-backed provider later.
pub struct StaticLabelProvider {
    by_addr: HashMap<Address, EntityLabel>,
}

impl StaticLabelProvider {
    pub fn new() -> Self {
        Self {
            by_addr: HashMap::new(),
        }
    }

    pub fn insert(&mut self, addr: Address, name: &str, url: Option<&str>, source: LabelSource) {
        let label = EntityLabel::new(name.to_string(), url.map(str::to_owned), source);
        self.by_addr.insert(addr, label);
    }
}

impl Default for StaticLabelProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LabelProvider for StaticLabelProvider {
    async fn resolve(&self, addr: &Address) -> DomainResult<Option<EntityLabel>> {
        Ok(self.by_addr.get(addr).cloned())
    }
}

/// Run watchlist matching over a freshly-saved batch of transfers and record
/// alerts for each touched watchlist address.
pub async fn check_watchlist_and_alert<W: WatchlistRepository, S: AlertSink>(
    watchlist: &W,
    alerts: &S,
    transfers: &[domain::transfer::Transfer],
) -> DomainResult<usize> {
    let mut recorded = 0usize;
    for t in transfers {
        for addr in [t.from(), t.to()] {
            if watchlist.contains(addr).await? {
                alerts
                    .record(Alert {
                        address: addr.clone(),
                        triggered_by_tx: *t.id().tx_hash(),
                        triggered_by_idx: t.id().index(),
                        created_at: Utc::now(),
                        reason: Some(format!("transfer involving watchlist address {}", addr)),
                    })
                    .await?;
                recorded += 1;
            }
        }
    }
    Ok(recorded)
}
