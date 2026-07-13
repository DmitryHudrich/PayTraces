use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use domain::entity::{Entity, RiskScore};
use domain::label_tag::{LabelTag, TagCategory, TagHistoryEvent, TagId, TagSource};
use domain::primitives::Confidence;
use serde::{Deserialize, Serialize};

use crate::error::{ApiError, ErrorResponse};
use crate::format::{parse_address, tag_category_str, tag_source_str};
use crate::state::{AppState, resolve_chain_id};

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
pub struct LabelRequest {
    #[schema(example = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")]
    pub address: String,
    #[schema(example = 1)]
    pub chain_id: Option<u32>,
    #[schema(example = "sanctioned")]
    pub category: String,
    #[schema(example = "OFAC SDN — Tornado Cash")]
    pub label_name: Option<String>,
    #[schema(example = "ofac_sdn")]
    pub source: Option<String>,
    #[schema(example = "confirmed")]
    pub confidence: Option<String>,
    #[schema(example = 100)]
    pub risk_score: Option<u8>,
    #[schema(example = "ofac")]
    pub sanction_list: Option<String>,
    #[schema(example = "https://...")]
    pub evidence_url: Option<String>,
    #[schema(value_type = Option<String>)]
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct AddressDto {
    address: String,
    chain_id: u32,
    attached_at: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct LabelTagDto {
    tag_id: String,
    category: String,
    label_name: Option<String>,
    source: String,
    confidence: u8,
    risk_score: u8,
    sanction_list: Option<String>,
    active: bool,
    superseded_by: Option<String>,
    created_at: String,
    expires_at: Option<String>,
    evidence_url: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct HistoryEventDto {
    tag_id: String,
    action: String,
    at: String,
    actor: String,
    reason: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct EntityResponse {
    entity_id: String,
    addresses: Vec<AddressDto>,
    tags: Vec<LabelTagDto>,
    aggregate_risk_score: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    history: Option<Vec<HistoryEventDto>>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct LabelsBulkResponse {
    upserted: usize,
    errors: Vec<String>,
}

fn parse_category(s: &str) -> Result<TagCategory, ApiError> {
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
        other => return Err(ApiError::bad_request(format!("unknown category: {other}"))),
    })
}

fn parse_source(s: Option<&str>) -> Result<TagSource, ApiError> {
    Ok(match s.unwrap_or("internal_analyst") {
        "ofac_sdn" => TagSource::OfacSdn,
        "eu_sanctions" => TagSource::EuSanctions,
        "un_sanctions" => TagSource::UnSanctions,
        "internal_analyst" => TagSource::InternalAnalyst,
        "heuristic_cluster" => TagSource::HeuristicCluster,
        "legacy_import" => TagSource::LegacyImport,
        other => match other.strip_prefix("third_party:") {
            Some(detail) => TagSource::ThirdParty(detail.to_string()),
            None => return Err(ApiError::bad_request(format!("unknown source: {other}"))),
        },
    })
}

fn parse_confidence(s: Option<&str>) -> Confidence {
    match s.unwrap_or("medium") {
        "low" => Confidence::LOW,
        "high" => Confidence::HIGH,
        "confirmed" | "certain" => Confidence::CERTAIN,
        _ => Confidence::MEDIUM,
    }
}

fn tag_to_dto(t: &LabelTag) -> LabelTagDto {
    LabelTagDto {
        tag_id: t.tag_id().value().to_string(),
        category: tag_category_str(t.category()).to_string(),
        label_name: t.label_name().map(str::to_string),
        source: tag_source_str(t.source()),
        confidence: t.confidence().value(),
        risk_score: t.risk_score().value(),
        sanction_list: t.sanction_list().map(str::to_string),
        active: t.active(),
        superseded_by: t.superseded_by().map(|id| id.value().to_string()),
        created_at: t.created_at().to_rfc3339(),
        expires_at: t.expires_at().map(|e| e.to_rfc3339()),
        evidence_url: t.evidence_url().map(str::to_string),
    }
}

fn history_to_dto(e: &TagHistoryEvent) -> HistoryEventDto {
    HistoryEventDto {
        tag_id: e.tag_id().value().to_string(),
        action: format!("{:?}", e.action()).to_lowercase(),
        at: e.at().to_rfc3339(),
        actor: tag_source_str(e.actor()),
        reason: e.reason().map(str::to_string),
    }
}

fn entity_to_dto(state: &AppState, e: &Entity, history: Option<Vec<TagHistoryEvent>>) -> EntityResponse {
    EntityResponse {
        entity_id: e.id().value().to_string(),
        addresses: e
            .addresses()
            .iter()
            .map(|a| AddressDto {
                address: a.address().canonical(),
                chain_id: a.address().chain().value(),
                attached_at: a.attached_at().to_rfc3339(),
            })
            .collect(),
        tags: e.tags().iter().map(tag_to_dto).collect(),
        aggregate_risk_score: e.aggregate_risk_score(state.tag_aggregation()).value(),
        history: history.map(|h| h.iter().map(history_to_dto).collect()),
    }
}

pub(crate) async fn apply_label(
    state: &AppState,
    body: &LabelRequest,
) -> Result<Entity, ApiError> {
    let chain = resolve_chain_id(state, body.chain_id);
    let addr = parse_address(&body.address, chain)?;
    let category = parse_category(&body.category)?;
    let source = parse_source(body.source.as_deref())?;
    let confidence = parse_confidence(body.confidence.as_deref());
    let risk_score = body
        .risk_score
        .map(RiskScore::new)
        .unwrap_or_else(|| usecase::default_risk_for(category));

    let input = usecase::TagApplyInput {
        category,
        label_name: body.label_name.clone(),
        source,
        confidence,
        risk_score,
        sanction_list: body.sanction_list.clone(),
        expires_at: body.expires_at,
        evidence_url: body.evidence_url.clone(),
    };

    usecase::apply_tag(state.entities(), state.tag_history(), &addr, input)
        .await
        .map_err(ApiError::Internal)
}

#[utoipa::path(
    post, path = "/labels",
    description = "Attach a risk tag to an address (admin only).\n\n\
                   ## What this does\n\n\
                   Resolves the target entity by address (creating one if none exists) and \
                   applies a `LabelTag` following these rules:\n\n\
                   1. An active tag with the same `(category, source)` and a *lower or equal* \
                      confidence than this request is updated in place — same `tag_id`.\n\
                   2. An active tag with the same `(category, source)` and a *higher* confidence \
                      than the existing one auto-supersedes it — a new tag is created, the old \
                      one is deactivated with `superseded_by` pointing at the new tag.\n\
                   3. An active tag with the same category but a *different* source is left \
                      untouched — sources are independent evidence and can coexist (e.g. \
                      `sanctioned` from both `ofac_sdn` and `eu_sanctions`).\n\n\
                   Every change is recorded in the append-only tag history. Multiple categories \
                   (e.g. `mixer` + `sanctioned`) can be active on the same address at once.\n\n\
                   ## Example\n\n\
                   ```bash\n\
                   curl -X POST 'http://localhost:8080/labels' \\\n\
                     -H 'X-API-Version: 1' \\\n\
                     -H 'X-Admin-Api-Key: <admin-key>' \\\n\
                     -H 'Content-Type: application/json' \\\n\
                     -d '{\n\
                       \"address\": \"0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045\",\n\
                       \"chain_id\": 1,\n\
                       \"category\": \"sanctioned\",\n\
                       \"label_name\": \"OFAC SDN\",\n\
                       \"source\": \"ofac_sdn\",\n\
                       \"confidence\": \"confirmed\",\n\
                       \"risk_score\": 100,\n\
                       \"sanction_list\": \"ofac\"\n\
                     }'\n\
                   ```\n\n\
                   ## Notes\n\n\
                   Requires the `X-Admin-Api-Key` header. `risk_score` defaults from `category` \
                   when omitted; `confidence` defaults to `medium`, `source` to `internal_analyst`.",
    request_body = LabelRequest,
    responses(
        (status = 200, body = EntityResponse),
        (status = 400, body = ErrorResponse),
        (status = 401, body = ErrorResponse),
    ),
    tag = "Labels"
)]
pub async fn labels_set(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LabelRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let entity = apply_label(&state, &body).await?;
    Ok(Json(entity_to_dto(&state, &entity, None)))
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PathAddrQuery {
    #[param(example = 1)]
    pub chain_id: Option<u32>,
    #[param(example = false)]
    pub include_history: Option<bool>,
}

#[utoipa::path(
    get, path = "/labels/{addr}",
    description = "Look up the entity attached to an address (admin only).\n\n\
                   ## What this does\n\n\
                   Returns the entity that contains the supplied address: every sibling address, \
                   every tag (active and inactive) with its full lifecycle fields, and the \
                   `aggregate_risk_score` computed from active tags via the configured strategy \
                   (`score.aggregate_strategy` in config.yaml). Pass `?include_history=true` to \
                   also get the append-only history log — omitted by default to keep the common \
                   case light.\n\n\
                   This is a DB-only read. No chain RPC is touched.\n\n\
                   ## Example\n\n\
                   ```bash\n\
                   curl 'http://localhost:8080/labels/0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045?chain_id=1&include_history=true' \\\n\
                     -H 'X-API-Version: 1' \\\n\
                     -H 'X-Admin-Api-Key: <admin-key>'\n\
                   ```\n\n\
                   ## Notes\n\n\
                   Requires the `X-Admin-Api-Key` header. `chain_id` defaults to 1 (Ethereum \
                   mainnet); pass it explicitly for other chains.",
    params(
        ("addr" = String, Path, description = "Address in hex (EVM) or canonical chain form."),
        PathAddrQuery,
    ),
    responses(
        (status = 200, body = EntityResponse),
        (status = 400, body = ErrorResponse),
        (status = 401, body = ErrorResponse),
    ),
    tag = "Labels"
)]
pub async fn labels_get(
    State(state): State<Arc<AppState>>,
    Path(addr_param): Path<String>,
    Query(q): Query<PathAddrQuery>,
) -> Result<impl IntoResponse, ApiError> {
    use domain::ports::{EntityRepository, TagHistoryRepository};
    let chain = resolve_chain_id(&state, q.chain_id);
    let addr = parse_address(&addr_param, chain)?;
    let entity = state
        .entities()
        .find_by_address(&addr)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::bad_request("no entity for this address".to_string()))?;

    let history = if q.include_history.unwrap_or(false) {
        Some(
            state
                .tag_history()
                .history_for_entity(entity.id())
                .await
                .map_err(ApiError::Internal)?,
        )
    } else {
        None
    };

    Ok(Json(entity_to_dto(&state, &entity, history)))
}

#[utoipa::path(
    delete, path = "/labels/{addr}",
    description = "Deactivate every active tag on the address's entity (admin only).\n\n\
                   ## What this does\n\n\
                   Sets `active = false` on every currently-active tag of the entity attached to \
                   this address (the entity and its addresses are preserved) and records one \
                   `Deactivated` history event per tag. Tags are never physically deleted — they \
                   remain for audit via `GET /labels/{addr}?include_history=true`.\n\n\
                   To deactivate a single tag rather than the whole set, use \
                   `DELETE /labels/{addr}/tags/{tag_id}`.\n\n\
                   ## Example\n\n\
                   ```bash\n\
                   curl -X DELETE 'http://localhost:8080/labels/0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045?chain_id=1' \\\n\
                     -H 'X-API-Version: 1' \\\n\
                     -H 'X-Admin-Api-Key: <admin-key>'\n\
                   ```\n\n\
                   ## Notes\n\n\
                   Requires the `X-Admin-Api-Key` header.",
    params(
        ("addr" = String, Path, description = "Address in hex (EVM) or canonical chain form."),
        PathAddrQuery,
    ),
    responses(
        (status = 200),
        (status = 400, body = ErrorResponse),
        (status = 401, body = ErrorResponse),
    ),
    tag = "Labels"
)]
pub async fn labels_delete(
    State(state): State<Arc<AppState>>,
    Path(addr_param): Path<String>,
    Query(q): Query<PathAddrQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let chain = resolve_chain_id(&state, q.chain_id);
    let addr = parse_address(&addr_param, chain)?;

    let result = usecase::deactivate_all(
        state.entities(),
        state.tag_history(),
        &addr,
        TagSource::InternalAnalyst,
        Some("deactivated via DELETE /labels/{addr}".to_string()),
    )
    .await
    .map_err(ApiError::Internal)?;

    Ok(Json(match result {
        Some((e, tags_deactivated)) => serde_json::json!({
            "deactivated": true,
            "entity_id": e.id().value().to_string(),
            "tags_deactivated": tags_deactivated,
        }),
        None => serde_json::json!({ "deactivated": false }),
    }))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct TagPatchRequest {
    pub active: Option<bool>,
    #[serde(default, with = "option_option_datetime")]
    #[schema(value_type = Option<String>)]
    pub expires_at: Option<Option<DateTime<Utc>>>,
    pub superseded_by: Option<String>,
    pub risk_score: Option<u8>,
}

/// `expires_at` needs three states over the wire: absent (don't touch),
/// `null` (clear it), or a timestamp (set it) — a plain `Option<T>` can't
/// tell "absent" from "null" apart, so this wraps it in `Option<Option<T>>`
/// and only treats a present-but-possibly-null JSON key as "touch this field".
mod option_option_datetime {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(d: D) -> Result<Option<Option<DateTime<Utc>>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Some(Option::<DateTime<Utc>>::deserialize(d)?))
    }
}

#[utoipa::path(
    patch, path = "/labels/{addr}/tags/{tag_id}",
    description = "Point-edit one tag's lifecycle fields (admin only).\n\n\
                   ## What this does\n\n\
                   Updates `active`, `expires_at`, `superseded_by`, and/or `risk_score` on a \
                   single tag, whichever fields are present in the body — omitted fields are left \
                   untouched. This is the only way to set `superseded_by` manually (the automatic \
                   supersede path only fires on same-source, higher-confidence `POST /labels` \
                   calls). Writes one history event reflecting the net effect (`reactivated` if \
                   `active` flips false→true, `deactivated` if true→false, `updated` otherwise).\n\n\
                   ## Notes\n\n\
                   Requires the `X-Admin-Api-Key` header.",
    params(
        ("addr" = String, Path, description = "Address in hex (EVM) or canonical chain form."),
        ("tag_id" = String, Path, description = "Tag UUID."),
        PathAddrQuery,
    ),
    request_body = TagPatchRequest,
    responses(
        (status = 200, body = LabelTagDto),
        (status = 400, body = ErrorResponse),
        (status = 401, body = ErrorResponse),
    ),
    tag = "Labels"
)]
pub async fn labels_patch_tag(
    State(state): State<Arc<AppState>>,
    Path((addr_param, tag_id_param)): Path<(String, String)>,
    Query(q): Query<PathAddrQuery>,
    Json(body): Json<TagPatchRequest>,
) -> Result<impl IntoResponse, ApiError> {
    use domain::ports::EntityRepository;
    let chain = resolve_chain_id(&state, q.chain_id);
    let addr = parse_address(&addr_param, chain)?;
    let tag_id = parse_tag_id(&tag_id_param)?;

    let entity = state
        .entities()
        .find_by_address(&addr)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::bad_request("no entity for this address".to_string()))?;

    let superseded_by = match body.superseded_by {
        Some(s) => Some(Some(parse_tag_id(&s)?)),
        None => None,
    };

    let patch = usecase::TagPatchInput {
        active: body.active,
        expires_at: body.expires_at,
        superseded_by,
        risk_score: body.risk_score.map(RiskScore::new),
    };

    let tag = usecase::patch_tag(
        state.entities(),
        state.tag_history(),
        &entity,
        tag_id,
        patch,
        TagSource::InternalAnalyst,
        Some("admin patch via PATCH /labels/{addr}/tags/{tag_id}".to_string()),
    )
    .await
    .map_err(ApiError::Internal)?
    .ok_or_else(|| ApiError::bad_request("no such tag on this entity".to_string()))?;

    Ok(Json(tag_to_dto(&tag)))
}

#[utoipa::path(
    delete, path = "/labels/{addr}/tags/{tag_id}",
    description = "Deactivate a single tag (admin only).\n\n\
                   Sets `active = false` on the specified tag only, leaving any other active tags \
                   on the entity untouched, and records a `Deactivated` history event.\n\n\
                   ## Notes\n\n\
                   Requires the `X-Admin-Api-Key` header.",
    params(
        ("addr" = String, Path, description = "Address in hex (EVM) or canonical chain form."),
        ("tag_id" = String, Path, description = "Tag UUID."),
        PathAddrQuery,
    ),
    responses(
        (status = 200),
        (status = 400, body = ErrorResponse),
        (status = 401, body = ErrorResponse),
    ),
    tag = "Labels"
)]
pub async fn labels_delete_tag(
    State(state): State<Arc<AppState>>,
    Path((addr_param, tag_id_param)): Path<(String, String)>,
    Query(q): Query<PathAddrQuery>,
) -> Result<impl IntoResponse, ApiError> {
    use domain::ports::EntityRepository;
    let chain = resolve_chain_id(&state, q.chain_id);
    let addr = parse_address(&addr_param, chain)?;
    let tag_id = parse_tag_id(&tag_id_param)?;

    let entity = state
        .entities()
        .find_by_address(&addr)
        .await
        .map_err(ApiError::Internal)?
        .ok_or_else(|| ApiError::bad_request("no entity for this address".to_string()))?;

    let deactivated = usecase::deactivate_one(
        state.entities(),
        state.tag_history(),
        &entity,
        tag_id,
        TagSource::InternalAnalyst,
        Some("deactivated via DELETE /labels/{addr}/tags/{tag_id}".to_string()),
    )
    .await
    .map_err(ApiError::Internal)?;

    Ok(Json(serde_json::json!({ "deactivated": deactivated })))
}

fn parse_tag_id(s: &str) -> Result<TagId, ApiError> {
    uuid::Uuid::parse_str(s)
        .map(TagId::from_uuid)
        .map_err(|_| ApiError::bad_request(format!("invalid tag_id: {s}")))
}

#[utoipa::path(
    post, path = "/labels/bulk",
    request_body = [LabelRequest],
    responses(
        (status = 200, body = LabelsBulkResponse),
    ),
    tag = "Labels"
)]
pub async fn labels_bulk(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Vec<LabelRequest>>,
) -> Result<impl IntoResponse, ApiError> {
    let mut upserted = 0usize;
    let mut errors: Vec<String> = Vec::new();
    for req in body {
        match apply_label(&state, &req).await {
            Ok(_) => upserted += 1,
            Err(e) => {
                let msg = match e {
                    ApiError::BadRequest(s) => s,
                    ApiError::Unauthorized => "unauthorized".into(),
                    ApiError::Internal(de) => de.to_string(),
                    ApiError::InternalMsg(s) => s,
                };
                errors.push(format!("{}: {msg}", req.address));
            }
        }
    }
    Ok(Json(LabelsBulkResponse { upserted, errors }))
}
