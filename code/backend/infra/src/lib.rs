use async_trait::async_trait;
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;

use domain::chain::ChainId;
use domain::entity::{AddressRef, Entity, EntityId, RiskScore};
use domain::error::{DomainError, DomainResult};
use domain::label_tag::{LabelTag, TagAction, TagCategory, TagHistoryEvent, TagId, TagSource};
use domain::ports::{EntityRepository, TagHistoryRepository};
use domain::primitives::{Address, Confidence};

pub mod alchemy_evm_source;
pub mod bigquery_evm_source;
pub mod chain_sources;
pub mod eth_labels_source;
pub mod etherscan_evm_source;
pub mod evm_routed_source;
pub mod fetch_wallet_api;
pub mod forensics_repo;
pub mod in_memory;
mod job_repo;
pub mod key_pool;
pub mod pg;
pub mod price;
pub mod rate_limiter;
pub mod tron_source;
pub mod tron_tag_source;
mod transfer_repo;

pub use alchemy_evm_source::{AlchemyEvmConfig, AlchemyEvmSource};
pub use bigquery_evm_source::{BigQueryEvmConfig, BigQueryEvmSource};
pub use chain_sources::{ChainSources, ChainSourcesBuilder};
pub use eth_labels_source::{EthLabelsConfig, EthLabelsSource};
pub use etherscan_evm_source::{EtherscanEvmConfig, EtherscanEvmSource};
pub use evm_routed_source::{
    Capability, RoutedChains, RoutedEvmSource, RoutedEvmSourceBuilder,
};
pub use forensics_repo::{PostgresAddressKinds, PostgresAlerts, PostgresWatchlist};
pub use in_memory::{
    InMemoryAddressKinds, InMemoryAlerts, InMemoryWatchlist, StaticLabelProvider,
    check_watchlist_and_alert,
};
pub use job_repo::{JobRepository, JobRow};
pub use price::{StaticPriceProvider, enrich_with_usd};
pub use transfer_repo::PostgresTransferRepository;
pub use tron_source::{TronGridConfig, TronGridSource};
pub use tron_tag_source::TronTagSource;

pub struct PostgresEntityRepository {
    pool: Pool,
}

impl PostgresEntityRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub fn clone_with_pool(&self) -> Self {
        Self {
            pool: self.pool.clone(),
        }
    }
}

impl Clone for PostgresEntityRepository {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
        }
    }
}

#[async_trait]
impl EntityRepository for PostgresEntityRepository {
    async fn find_by_id(&self, id: &EntityId) -> DomainResult<Option<Entity>> {
        let client = self.pool.get().await.map_err(pool_err)?;

        let row = client
            .query_opt("SELECT id, created_at FROM entities WHERE id = $1", &[&id.value()])
            .await
            .map_err(pg_err)?;

        let Some(row) = row else { return Ok(None) };
        let entity_id = EntityId::from_uuid(row.get("id"));
        let addresses = load_addresses(&client, &entity_id).await?;
        let tags = load_tags(&client, &entity_id).await?;
        Ok(Some(Entity::from_parts(entity_id, addresses, tags, row.get("created_at"))))
    }

    async fn find_by_address(&self, addr: &Address) -> DomainResult<Option<Entity>> {
        let client = self.pool.get().await.map_err(pool_err)?;

        let addr_hex = hex::encode(addr.bytes());
        let chain_id = addr.chain().value() as i32;

        let row = client
            .query_opt(
                "SELECT e.id, e.created_at
                 FROM entities e
                 JOIN entity_addresses ea ON ea.entity_id = e.id
                 WHERE ea.chain_id = $1 AND ea.address = $2
                 LIMIT 1",
                &[&chain_id, &addr_hex],
            )
            .await
            .map_err(pg_err)?;

        let Some(row) = row else { return Ok(None) };
        let entity_id = EntityId::from_uuid(row.get("id"));
        let addresses = load_addresses(&client, &entity_id).await?;
        let tags = load_tags(&client, &entity_id).await?;
        Ok(Some(Entity::from_parts(entity_id, addresses, tags, row.get("created_at"))))
    }

    async fn find_active_tag(
        &self,
        entity_id: &EntityId,
        category: TagCategory,
        source: &TagSource,
    ) -> DomainResult<Option<LabelTag>> {
        let client = self.pool.get().await.map_err(pool_err)?;
        let cat_s = category_to_str(category);
        let (source_s, source_detail) = source_to_str(source);

        let row = client
            .query_opt(
                "SELECT tag_id, category, label_name, source, source_detail, confidence,
                        risk_score, sanction_list, active, superseded_by, created_at,
                        expires_at, evidence_url
                 FROM label_tags
                 WHERE entity_id = $1 AND category = $2 AND source = $3
                   AND COALESCE(source_detail, '') = COALESCE($4, '')
                   AND active = true
                   AND (expires_at IS NULL OR expires_at > now())
                 LIMIT 1",
                &[&entity_id.value(), &cat_s, &source_s, &source_detail],
            )
            .await
            .map_err(pg_err)?;

        row.as_ref().map(row_to_tag).transpose()
    }

    async fn save_entity(&self, entity: &Entity) -> DomainResult<()> {
        let client = self.pool.get().await.map_err(pool_err)?;

        client
            .execute(
                "INSERT INTO entities (id, created_at) VALUES ($1, $2)
                 ON CONFLICT (id) DO NOTHING",
                &[&entity.id().value(), &entity.created_at()],
            )
            .await
            .map_err(pg_err)?;

        if !entity.addresses().is_empty() {
            let entity_ids: Vec<uuid::Uuid> =
                entity.addresses().iter().map(|_| entity.id().value()).collect();
            let chain_ids: Vec<i32> = entity
                .addresses()
                .iter()
                .map(|a| a.address().chain().value() as i32)
                .collect();
            let addrs: Vec<String> = entity
                .addresses()
                .iter()
                .map(|a| hex::encode(a.address().bytes()))
                .collect();
            let attached_ats: Vec<DateTime<Utc>> =
                entity.addresses().iter().map(|a| a.attached_at()).collect();

            client
                .execute(
                    "INSERT INTO entity_addresses (entity_id, chain_id, address, attached_at)
                     SELECT * FROM UNNEST($1::UUID[], $2::INT[], $3::TEXT[], $4::TIMESTAMPTZ[])
                     ON CONFLICT (chain_id, address) DO NOTHING",
                    &[&entity_ids, &chain_ids, &addrs, &attached_ats],
                )
                .await
                .map_err(pg_err)?;
        }

        Ok(())
    }

    async fn upsert_tag(&self, entity_id: &EntityId, tag: &LabelTag) -> DomainResult<()> {
        let client = self.pool.get().await.map_err(pool_err)?;
        let cat_s = category_to_str(tag.category());
        let (source_s, source_detail) = source_to_str(tag.source());

        client
            .execute(
                "INSERT INTO label_tags (tag_id, entity_id, category, label_name, source,
                                          source_detail, confidence, risk_score, sanction_list,
                                          active, superseded_by, created_at, expires_at, evidence_url)
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
                 ON CONFLICT (tag_id) DO UPDATE
                    SET category      = EXCLUDED.category,
                        label_name    = EXCLUDED.label_name,
                        source        = EXCLUDED.source,
                        source_detail = EXCLUDED.source_detail,
                        confidence    = EXCLUDED.confidence,
                        risk_score    = EXCLUDED.risk_score,
                        sanction_list = EXCLUDED.sanction_list,
                        active        = EXCLUDED.active,
                        superseded_by = EXCLUDED.superseded_by,
                        expires_at    = EXCLUDED.expires_at,
                        evidence_url  = EXCLUDED.evidence_url",
                &[
                    &tag.tag_id().value(),
                    &entity_id.value(),
                    &cat_s,
                    &tag.label_name(),
                    &source_s,
                    &source_detail,
                    &(tag.confidence().value() as i16),
                    &(tag.risk_score().value() as i16),
                    &tag.sanction_list(),
                    &tag.active(),
                    &tag.superseded_by().map(|t| t.value()),
                    &tag.created_at(),
                    &tag.expires_at(),
                    &tag.evidence_url(),
                ],
            )
            .await
            .map_err(pg_err)?;

        Ok(())
    }

    async fn list_sanctioned(&self) -> DomainResult<Vec<Entity>> {
        let client = self.pool.get().await.map_err(pool_err)?;

        let rows = client
            .query(
                "SELECT DISTINCT e.id, e.created_at
                 FROM entities e
                 JOIN label_tags lt ON lt.entity_id = e.id
                 WHERE lt.category = 'sanctioned'
                   AND lt.active = true
                   AND (lt.expires_at IS NULL OR lt.expires_at > now())",
                &[],
            )
            .await
            .map_err(pg_err)?;

        let mut entities = Vec::with_capacity(rows.len());
        for row in &rows {
            let entity_id = EntityId::from_uuid(row.get("id"));
            let addresses = load_addresses(&client, &entity_id).await?;
            let tags = load_tags(&client, &entity_id).await?;
            entities.push(Entity::from_parts(entity_id, addresses, tags, row.get("created_at")));
        }
        Ok(entities)
    }
}

pub struct PostgresTagHistoryRepository {
    pool: Pool,
}

impl PostgresTagHistoryRepository {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }
}

impl Clone for PostgresTagHistoryRepository {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
        }
    }
}

#[async_trait]
impl TagHistoryRepository for PostgresTagHistoryRepository {
    async fn append(&self, event: &TagHistoryEvent) -> DomainResult<()> {
        let client = self.pool.get().await.map_err(pool_err)?;
        client
            .execute(
                "INSERT INTO tag_history (tag_id, action, at, actor, reason)
                 VALUES ($1, $2, $3, $4, $5)",
                &[
                    &event.tag_id().value(),
                    &action_to_str(event.action()),
                    &event.at(),
                    &actor_to_str(event.actor()),
                    &event.reason(),
                ],
            )
            .await
            .map_err(pg_err)?;
        Ok(())
    }

    async fn history_for_tag(&self, tag_id: &TagId) -> DomainResult<Vec<TagHistoryEvent>> {
        let client = self.pool.get().await.map_err(pool_err)?;
        let rows = client
            .query(
                "SELECT tag_id, action, at, actor, reason FROM tag_history
                 WHERE tag_id = $1 ORDER BY at",
                &[&tag_id.value()],
            )
            .await
            .map_err(pg_err)?;
        rows.iter().map(row_to_history).collect()
    }

    async fn history_for_entity(&self, entity_id: &EntityId) -> DomainResult<Vec<TagHistoryEvent>> {
        let client = self.pool.get().await.map_err(pool_err)?;
        let rows = client
            .query(
                "SELECT th.tag_id, th.action, th.at, th.actor, th.reason
                 FROM tag_history th
                 JOIN label_tags lt ON lt.tag_id = th.tag_id
                 WHERE lt.entity_id = $1
                 ORDER BY th.at",
                &[&entity_id.value()],
            )
            .await
            .map_err(pg_err)?;
        rows.iter().map(row_to_history).collect()
    }
}

async fn load_addresses(
    client: &deadpool_postgres::Object,
    entity_id: &EntityId,
) -> DomainResult<Vec<AddressRef>> {
    let rows = client
        .query(
            "SELECT chain_id, address, attached_at FROM entity_addresses WHERE entity_id = $1",
            &[&entity_id.value()],
        )
        .await
        .map_err(pg_err)?;

    rows.iter()
        .map(|r| {
            let chain = ChainId::new(r.get::<_, i32>("chain_id") as u32);
            let bytes = hex::decode(r.get::<_, &str>("address"))
                .map_err(|e| DomainError::InsufficientData(format!("address hex: {e}")))?;
            Ok(AddressRef::new(Address::new(chain, bytes), r.get("attached_at")))
        })
        .collect()
}

async fn load_tags(
    client: &deadpool_postgres::Object,
    entity_id: &EntityId,
) -> DomainResult<Vec<LabelTag>> {
    let rows = client
        .query(
            "SELECT tag_id, category, label_name, source, source_detail, confidence,
                    risk_score, sanction_list, active, superseded_by, created_at,
                    expires_at, evidence_url
             FROM label_tags WHERE entity_id = $1 ORDER BY created_at",
            &[&entity_id.value()],
        )
        .await
        .map_err(pg_err)?;

    rows.iter().map(row_to_tag).collect()
}

fn row_to_tag(row: &tokio_postgres::Row) -> DomainResult<LabelTag> {
    let tag_id = TagId::from_uuid(row.get("tag_id"));
    let category = str_to_category(row.get("category"))?;
    let label_name = row.get::<_, Option<&str>>("label_name").map(str::to_string);
    let source = str_to_source(row.get("source"), row.get("source_detail"))?;
    let confidence = Confidence::new(row.get::<_, i16>("confidence") as u8);
    let risk_score = RiskScore::new(row.get::<_, i16>("risk_score") as u8);
    let sanction_list = row.get::<_, Option<&str>>("sanction_list").map(str::to_string);
    let active = row.get("active");
    let superseded_by = row
        .get::<_, Option<uuid::Uuid>>("superseded_by")
        .map(TagId::from_uuid);
    let created_at = row.get("created_at");
    let expires_at = row.get("expires_at");
    let evidence_url = row.get::<_, Option<&str>>("evidence_url").map(str::to_string);

    Ok(LabelTag::from_parts(
        tag_id,
        category,
        label_name,
        source,
        confidence,
        risk_score,
        sanction_list,
        active,
        superseded_by,
        created_at,
        expires_at,
        evidence_url,
    ))
}

fn row_to_history(row: &tokio_postgres::Row) -> DomainResult<TagHistoryEvent> {
    let tag_id = TagId::from_uuid(row.get("tag_id"));
    let action = str_to_action(row.get("action"))?;
    let at = row.get("at");
    let actor = str_to_actor(row.get("actor"))?;
    let reason = row.get::<_, Option<&str>>("reason").map(str::to_string);
    Ok(TagHistoryEvent::from_parts(tag_id, action, at, actor, reason))
}

fn category_to_str(cat: TagCategory) -> &'static str {
    match cat {
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

fn str_to_category(s: &str) -> DomainResult<TagCategory> {
    Ok(match s {
        "exchange" => TagCategory::Exchange,
        "mixer" => TagCategory::Mixer,
        "bridge" => TagCategory::Bridge,
        "defi" => TagCategory::DefiProtocol,
        "sanctioned" => TagCategory::Sanctioned,
        "scam" => TagCategory::Scam,
        "gambling" => TagCategory::Gambling,
        "darknet" => TagCategory::Darknet,
        "mining" => TagCategory::Mining,
        "known_service" => TagCategory::KnownService,
        "unknown" => TagCategory::Unknown,
        other => {
            return Err(DomainError::InsufficientData(format!(
                "unknown tag category: {other}"
            )));
        }
    })
}

fn source_to_str(source: &TagSource) -> (&'static str, Option<String>) {
    match source {
        TagSource::OfacSdn => ("ofac_sdn", None),
        TagSource::EuSanctions => ("eu_sanctions", None),
        TagSource::UnSanctions => ("un_sanctions", None),
        TagSource::InternalAnalyst => ("internal_analyst", None),
        TagSource::HeuristicCluster => ("heuristic_cluster", None),
        TagSource::ThirdParty(detail) => ("third_party", Some(detail.clone())),
        TagSource::LegacyImport => ("legacy_import", None),
    }
}

fn str_to_source(s: &str, detail: Option<&str>) -> DomainResult<TagSource> {
    Ok(match s {
        "ofac_sdn" => TagSource::OfacSdn,
        "eu_sanctions" => TagSource::EuSanctions,
        "un_sanctions" => TagSource::UnSanctions,
        "internal_analyst" => TagSource::InternalAnalyst,
        "heuristic_cluster" => TagSource::HeuristicCluster,
        "third_party" => TagSource::ThirdParty(detail.unwrap_or_default().to_string()),
        "legacy_import" => TagSource::LegacyImport,
        other => {
            return Err(DomainError::InsufficientData(format!(
                "unknown tag source: {other}"
            )));
        }
    })
}

fn action_to_str(action: TagAction) -> &'static str {
    match action {
        TagAction::Added => "added",
        TagAction::Updated => "updated",
        TagAction::Deactivated => "deactivated",
        TagAction::Reactivated => "reactivated",
        TagAction::Expired => "expired",
        TagAction::Superseded => "superseded",
    }
}

fn str_to_action(s: &str) -> DomainResult<TagAction> {
    Ok(match s {
        "added" => TagAction::Added,
        "updated" => TagAction::Updated,
        "deactivated" => TagAction::Deactivated,
        "reactivated" => TagAction::Reactivated,
        "expired" => TagAction::Expired,
        "superseded" => TagAction::Superseded,
        other => {
            return Err(DomainError::InsufficientData(format!(
                "unknown tag action: {other}"
            )));
        }
    })
}

/// `tag_history.actor` is a single TEXT column; `ThirdParty(detail)` is
/// encoded as `third_party:<detail>` so it round-trips without a second
/// column (history rows are append-only and never joined on this field).
fn actor_to_str(actor: &TagSource) -> String {
    let (disc, detail) = source_to_str(actor);
    match detail {
        Some(d) => format!("{disc}:{d}"),
        None => disc.to_string(),
    }
}

fn str_to_actor(s: &str) -> DomainResult<TagSource> {
    match s.split_once(':') {
        Some((disc, detail)) => str_to_source(disc, Some(detail)),
        None => str_to_source(s, None),
    }
}

pub(crate) fn pg_err(e: tokio_postgres::Error) -> DomainError {
    let detail = e
        .as_db_error()
        .map(|db| {
            let mut msg = db.message().to_string();
            if let Some(d) = db.detail() {
                msg.push_str(&format!(": {d}"));
            }
            if let Some(h) = db.hint() {
                msg.push_str(&format!(" (hint: {h})"));
            }
            msg
        })
        .unwrap_or_else(|| e.to_string());
    DomainError::InsufficientData(detail)
}

pub(crate) fn pool_err(e: deadpool_postgres::PoolError) -> DomainError {
    DomainError::InsufficientData(e.to_string())
}
