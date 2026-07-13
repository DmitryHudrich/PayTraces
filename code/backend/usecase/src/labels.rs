//! Tag resolution engine shared by `POST /labels` and the ingestion
//! auto-tagger, so both callers apply the exact same rules (ТЗ §4):
//!
//! 1. Find the entity by address, create one if none exists.
//! 2. An active tag with the same `(category, source)`:
//!    - higher `confidence` than the incoming tag → **auto-supersede**: a
//!      new tag is created, the old one is deactivated and points at it
//!      via `superseded_by` (ТЗ §4 rule 4).
//!    - equal or lower confidence → **update in place**: mutate the
//!      existing tag's mutable fields, keep its `tag_id` (ТЗ §4 rule 2).
//! 3. Same category, different source, or a brand new category → always
//!    an independent new tag; nothing existing is touched (ТЗ §4 rule 3).
//!
//! Every branch writes a `TagHistoryEvent` (ТЗ §3.3 — append-only, never
//! mutated).

use chrono::{DateTime, Utc};
use domain::entity::{Entity, RiskScore};
use domain::error::DomainResult;
use domain::label_tag::{LabelTag, TagAction, TagCategory, TagHistoryEvent, TagId, TagSource};
use domain::ports::{EntityRepository, TagHistoryRepository};
use domain::primitives::{Address, Confidence};

/// Default `risk_score` for a category when the caller (an admin request or
/// an external tag candidate) doesn't supply one. Shared by `POST /labels`
/// and the ingestion auto-tagger so both land on the same defaults.
pub fn default_risk_for(category: TagCategory) -> RiskScore {
    match category {
        TagCategory::Sanctioned => RiskScore::new(100),
        TagCategory::Darknet => RiskScore::new(95),
        TagCategory::Mixer => RiskScore::new(90),
        TagCategory::Scam => RiskScore::new(75),
        TagCategory::Bridge => RiskScore::new(40),
        TagCategory::Exchange => RiskScore::new(30),
        _ => RiskScore::new(25),
    }
}

#[derive(Debug, Clone)]
pub struct TagApplyInput {
    pub category: TagCategory,
    pub label_name: Option<String>,
    pub source: TagSource,
    pub confidence: Confidence,
    pub risk_score: RiskScore,
    pub sanction_list: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub evidence_url: Option<String>,
}

/// Runs the resolution rules above and returns the fully-reloaded entity
/// (all addresses + all tags) afterwards.
pub async fn apply_tag(
    entities: &dyn EntityRepository,
    history: &dyn TagHistoryRepository,
    addr: &Address,
    input: TagApplyInput,
) -> DomainResult<Entity> {
    let mut entity = entities
        .find_by_address(addr)
        .await?
        .unwrap_or_default();
    entity.add_address(addr.clone());
    entities.save_entity(&entity).await?;

    let existing = entities
        .find_active_tag(entity.id(), input.category, &input.source)
        .await?;

    match existing {
        Some(mut old_tag) if input.confidence > old_tag.confidence() => {
            let new_tag = LabelTag::new(
                input.category,
                input.label_name,
                input.source.clone(),
                input.confidence,
                input.risk_score,
                input.sanction_list,
                input.expires_at,
                input.evidence_url,
            );
            let new_tag_id = new_tag.tag_id();
            let old_tag_id = old_tag.tag_id();

            old_tag.set_active(false);
            old_tag.set_superseded_by(Some(new_tag_id));

            // Insert the new tag before updating the old one — `old_tag`'s
            // `superseded_by` is a foreign key into `label_tags(tag_id)`,
            // so it must already exist or the upsert violates the FK.
            entities.upsert_tag(entity.id(), &new_tag).await?;
            entities.upsert_tag(entity.id(), &old_tag).await?;

            history
                .append(&TagHistoryEvent::new(
                    old_tag_id,
                    TagAction::Superseded,
                    input.source.clone(),
                    Some(format!("superseded by higher-confidence tag {}", new_tag_id.value())),
                ))
                .await?;
            history
                .append(&TagHistoryEvent::new(new_tag_id, TagAction::Added, input.source, None))
                .await?;
        }
        Some(mut old_tag) => {
            old_tag.set_label_name(input.label_name);
            old_tag.set_risk_score(input.risk_score);
            old_tag.set_sanction_list(input.sanction_list);
            old_tag.set_expires_at(input.expires_at);
            old_tag.set_evidence_url(input.evidence_url);

            entities.upsert_tag(entity.id(), &old_tag).await?;
            history
                .append(&TagHistoryEvent::new(old_tag.tag_id(), TagAction::Updated, input.source, None))
                .await?;
        }
        None => {
            let new_tag = LabelTag::new(
                input.category,
                input.label_name,
                input.source.clone(),
                input.confidence,
                input.risk_score,
                input.sanction_list,
                input.expires_at,
                input.evidence_url,
            );
            entities.upsert_tag(entity.id(), &new_tag).await?;
            history
                .append(&TagHistoryEvent::new(new_tag.tag_id(), TagAction::Added, input.source, None))
                .await?;
        }
    }

    Ok(entities
        .find_by_address(addr)
        .await?
        .expect("entity was just saved for this address"))
}

/// Deactivates every currently-active tag on the entity attached to `addr`
/// (ТЗ §6.4 — `DELETE /labels/{addr}` no longer detaches the address, it
/// deactivates the whole label set). Returns `Ok(None)` if the address has
/// no entity; otherwise the reloaded entity plus the count of tags this
/// call actually flipped to inactive (not the entity's total inactive
/// count, which may include tags deactivated earlier).
pub async fn deactivate_all(
    entities: &dyn EntityRepository,
    history: &dyn TagHistoryRepository,
    addr: &Address,
    actor: TagSource,
    reason: Option<String>,
) -> DomainResult<Option<(Entity, usize)>> {
    let Some(entity) = entities.find_by_address(addr).await? else {
        return Ok(None);
    };

    let mut deactivated = 0usize;
    for tag in entity.tags().iter().filter(|t| t.active()) {
        let mut tag = tag.clone();
        tag.set_active(false);
        entities.upsert_tag(entity.id(), &tag).await?;
        history
            .append(&TagHistoryEvent::new(
                tag.tag_id(),
                TagAction::Deactivated,
                actor.clone(),
                reason.clone(),
            ))
            .await?;
        deactivated += 1;
    }

    Ok(entities
        .find_by_address(addr)
        .await?
        .map(|e| (e, deactivated)))
}

/// Deactivates one specific tag (`DELETE /labels/{addr}/tags/{tag_id}`).
/// Returns `Ok(false)` if no active tag with that id exists on the entity.
pub async fn deactivate_one(
    entities: &dyn EntityRepository,
    history: &dyn TagHistoryRepository,
    entity: &Entity,
    tag_id: TagId,
    actor: TagSource,
    reason: Option<String>,
) -> DomainResult<bool> {
    let Some(tag) = entity.tags().iter().find(|t| t.tag_id() == tag_id && t.active()) else {
        return Ok(false);
    };
    let mut tag = tag.clone();
    tag.set_active(false);
    entities.upsert_tag(entity.id(), &tag).await?;
    history
        .append(&TagHistoryEvent::new(tag_id, TagAction::Deactivated, actor, reason))
        .await?;
    Ok(true)
}

#[derive(Debug, Clone, Default)]
pub struct TagPatchInput {
    pub active: Option<bool>,
    pub expires_at: Option<Option<DateTime<Utc>>>,
    pub superseded_by: Option<Option<TagId>>,
    pub risk_score: Option<RiskScore>,
}

/// Admin-only point edit (`PATCH /labels/{addr}/tags/{tag_id}`, ТЗ §6.3).
/// Returns `Ok(None)` if the tag doesn't exist on this entity.
pub async fn patch_tag(
    entities: &dyn EntityRepository,
    history: &dyn TagHistoryRepository,
    entity: &Entity,
    tag_id: TagId,
    patch: TagPatchInput,
    actor: TagSource,
    reason: Option<String>,
) -> DomainResult<Option<LabelTag>> {
    let Some(tag) = entity.tags().iter().find(|t| t.tag_id() == tag_id) else {
        return Ok(None);
    };
    let mut tag = tag.clone();

    let was_active = tag.active();
    if let Some(active) = patch.active {
        tag.set_active(active);
    }
    if let Some(expires_at) = patch.expires_at {
        tag.set_expires_at(expires_at);
    }
    if let Some(superseded_by) = patch.superseded_by {
        tag.set_superseded_by(superseded_by);
    }
    if let Some(risk_score) = patch.risk_score {
        tag.set_risk_score(risk_score);
    }

    entities.upsert_tag(entity.id(), &tag).await?;

    let action = match (was_active, tag.active()) {
        (true, false) => TagAction::Deactivated,
        (false, true) => TagAction::Reactivated,
        _ => TagAction::Updated,
    };
    history
        .append(&TagHistoryEvent::new(tag_id, action, actor, reason))
        .await?;

    Ok(Some(tag))
}
