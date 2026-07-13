use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Query, State},
    response::IntoResponse,
};
use domain::entity::AddressKind;
use domain::graph::NodeDegree;
use domain::label_tag::LabelTag;
use domain::ports::{AddressKindRepository, EntityRepository, RiskPort, TransferRepository};
use domain::primitives::Address;
use serde::{Deserialize, Serialize};

use crate::error::{ApiError, ErrorResponse};
use crate::format::{confidence_str, parse_address, tag_category_str};
use crate::middleware::ApiVersion;
use crate::state::{AppState, resolve_chain_id};

const MAX_NODES_BATCH: usize = 500;

#[derive(Serialize, utoipa::ToSchema)]
pub struct TagDto {
    #[schema(example = "exchange")]
    category: String,
    #[schema(example = "Binance 14")]
    label_name: Option<String>,
    #[schema(example = "high")]
    confidence: String,
    #[schema(example = 42)]
    risk_score: u8,
}

fn tag_to_dto(tag: &LabelTag) -> TagDto {
    TagDto {
        category: tag_category_str(tag.category()).to_string(),
        label_name: tag.label_name().map(str::to_string),
        confidence: confidence_str(tag.confidence()).to_string(),
        risk_score: tag.risk_score().value(),
    }
}

/// One graph node, enriched (ТЗ §3.1). Every field beyond `address` is
/// omitted when absent: under `enrich=minimal` all of them are
/// `None`/omitted (cheapest possible response); under `enrich=full`
/// (default), `kind`/`service_name`/degrees/boundary flags are always
/// present, while `risk_score`/`is_high_risk` stay `null` when there's no
/// cached score and `tags` stays empty when there's no entity — enrichment
/// never blocks on computing either.
#[derive(Serialize, utoipa::ToSchema)]
pub struct NodeDto {
    #[schema(example = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")]
    address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "known_service")]
    kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Binance hot wallet 14")]
    service_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 42)]
    risk_score: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_high_risk: Option<bool>,
    /// Every currently-active tag on this address's entity, highest
    /// `risk_score` first — not just the top one.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<TagDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 12)]
    in_degree: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 4)]
    out_degree: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 16)]
    tx_count: Option<u32>,
    /// `true` when this node was only discovered as an edge's counterparty
    /// and this `GET /graph` call didn't itself query its transfers (BFS
    /// stopped here due to `max_depth`/`max_nodes`) — there may be more to
    /// see already sitting in the DB; re-query with wider bounds or centered
    /// on this address. Always `null` on `GET /nodes/batch`, which has no
    /// bounded view to be at the edge of.
    #[serde(skip_serializing_if = "Option::is_none")]
    is_view_boundary: Option<bool>,
    /// `true` when *no* transfer touching this address has ever been
    /// persisted — `POST /jobs/ingest` is needed before another query
    /// (of any kind) would surface anything new here.
    #[serde(skip_serializing_if = "Option::is_none")]
    is_ingest_boundary: Option<bool>,
}

fn address_kind_parts(kind: AddressKind) -> (String, Option<String>) {
    match kind {
        AddressKind::Eoa => ("eoa".to_string(), None),
        AddressKind::Contract => ("contract".to_string(), None),
        AddressKind::KnownService(name) => ("known_service".to_string(), Some(name)),
        AddressKind::Unknown => ("unknown".to_string(), None),
    }
}

/// `enrich` query value → "skip every enrichment query" flag. Shared by
/// `GET /graph` and `GET /nodes/batch` so both accept the same contract.
pub(crate) fn parse_minimal(enrich: Option<&str>) -> Result<bool, ApiError> {
    match enrich.unwrap_or("full") {
        "full" => Ok(false),
        "minimal" => Ok(true),
        other => Err(ApiError::bad_request(format!(
            "invalid enrich value '{other}' (allowed: full, minimal)"
        ))),
    }
}

/// Context only `GET /graph` has: the bounded BFS view it just built.
/// `GET /nodes/batch` takes an arbitrary address list with no graph around
/// it, so it always passes `None` — degrees and the view-boundary flag stay
/// `null` there.
pub(crate) struct GraphViewContext<'a> {
    pub degrees: &'a HashMap<Address, NodeDegree>,
    pub expanded: &'a HashSet<Address>,
}

/// Builds one `NodeDto` per address in `addrs` (same order, ТЗ §3).
///
/// `risk_score`/`is_high_risk` come from `RiskPort::peek_score`, a
/// cache-only read — a miss becomes `null` rather than running the trace
/// `POST /score` would (ТЗ §3.2: "graph read never blocks on heavy compute").
/// `is_ingest_boundary` comes from `TransferRepository::touches_batch` —
/// also cheap, one batched `EXISTS` query for the whole list.
pub(crate) async fn enrich_nodes(
    state: &AppState,
    addrs: &[Address],
    view: Option<GraphViewContext<'_>>,
    minimal: bool,
) -> Result<Vec<NodeDto>, ApiError> {
    if minimal || addrs.is_empty() {
        return Ok(addrs
            .iter()
            .map(|a| NodeDto {
                address: a.canonical(),
                kind: None,
                service_name: None,
                risk_score: None,
                is_high_risk: None,
                tags: Vec::new(),
                in_degree: None,
                out_degree: None,
                tx_count: None,
                is_view_boundary: None,
                is_ingest_boundary: None,
            })
            .collect());
    }

    let kinds = state
        .address_kinds()
        .kind_batch(addrs)
        .await
        .map_err(ApiError::Internal)?;
    let all_tags = state
        .entities()
        .active_tags_batch(addrs)
        .await
        .map_err(ApiError::Internal)?;
    let touched = state
        .transfers()
        .touches_batch(addrs)
        .await
        .map_err(ApiError::Internal)?;

    let mut kinds = kinds.into_iter();
    let mut all_tags = all_tags.into_iter();
    let mut touched = touched.into_iter();

    let mut out = Vec::with_capacity(addrs.len());
    for addr in addrs {
        let kind = kinds.next().expect("kind_batch returns one entry per address");
        let tags = all_tags
            .next()
            .expect("active_tags_batch returns one entry per address");
        let is_touched = touched
            .next()
            .expect("touches_batch returns one entry per address");

        let (kind_str, service_name) = address_kind_parts(kind);
        let report = state
            .risk()
            .peek_score(addr)
            .await
            .map_err(ApiError::Internal)?;
        let (risk_score, is_high_risk) = match &report {
            Some(r) => (Some(r.overall_score().value()), Some(r.is_high_risk())),
            None => (None, None),
        };
        let degree = view.as_ref().and_then(|v| v.degrees.get(addr));
        let is_view_boundary = view.as_ref().map(|v| !v.expanded.contains(addr));

        out.push(NodeDto {
            address: addr.canonical(),
            kind: Some(kind_str),
            service_name,
            risk_score,
            is_high_risk,
            tags: tags.iter().map(tag_to_dto).collect(),
            in_degree: degree.map(|d| d.in_degree),
            out_degree: degree.map(|d| d.out_degree),
            tx_count: degree.map(|d| d.in_degree + d.out_degree),
            is_view_boundary,
            is_ingest_boundary: Some(!is_touched),
        });
    }
    Ok(out)
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct NodesBatchQuery {
    #[param(example = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045,0x28C6c06298d514Db089934071355E5743bf21d6")]
    addresses: String,
    #[param(example = 1)]
    chain_id: Option<u32>,
    #[param(example = "full")]
    enrich: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct NodesBatchResponse {
    nodes: Vec<NodeDto>,
}

#[utoipa::path(
    get, path = "/nodes/batch",
    description = "Batch-enrich a list of addresses with the same `NodeDto` shape `GET /graph` \
                   returns under `X-API-Version: 2` (ТЗ §6).\n\n\
                   ## What this does\n\n\
                   Fills in `kind`, `risk_score` (cache-only, never computes), every active \
                   `tags` entry, and `is_ingest_boundary` for every address in `addresses` \
                   (comma-separated, up to 500) in a small, fixed number of DB round-trips — no \
                   matter how many addresses are passed. Meant as the fallback for endpoints that \
                   still return bare address strings (`/cluster`, `/path`, `/heuristics`): fetch \
                   those first, then batch-enrich the addresses you care about here instead of \
                   calling `/address/{addr}/kind` + `/score` + `/labels/{addr}` once per address.\n\n\
                   `in_degree`/`out_degree`/`tx_count`/`is_view_boundary` are always `null` here — \
                   unlike `/graph`, this endpoint has no bounded edge set/BFS view to derive them \
                   from.\n\n\
                   ## Notes\n\n\
                   Requires `X-API-Version: 2`. `enrich=minimal` returns only `address` per node, \
                   skipping every enrichment query.",
    params(NodesBatchQuery),
    responses(
        (status = 200, body = NodesBatchResponse),
        (status = 400, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    ),
    tag = "Graph"
)]
pub async fn nodes_batch(
    State(state): State<Arc<AppState>>,
    Extension(version): Extension<ApiVersion>,
    Query(q): Query<NodesBatchQuery>,
) -> Result<impl IntoResponse, ApiError> {
    if version.0 < 2 {
        return Err(ApiError::bad_request(
            "GET /nodes/batch requires X-API-Version: 2".to_string(),
        ));
    }

    let chain = resolve_chain_id(&state, q.chain_id);
    let addr_strs: Vec<&str> = q
        .addresses
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    if addr_strs.is_empty() {
        return Err(ApiError::bad_request("addresses must not be empty".to_string()));
    }
    if addr_strs.len() > MAX_NODES_BATCH {
        return Err(ApiError::bad_request(format!(
            "batch too large: {} > {MAX_NODES_BATCH}",
            addr_strs.len()
        )));
    }

    let mut addrs = Vec::with_capacity(addr_strs.len());
    for s in addr_strs {
        addrs.push(parse_address(s, chain)?);
    }

    let minimal = parse_minimal(q.enrich.as_deref())?;
    let nodes = enrich_nodes(&state, &addrs, None, minimal).await?;
    Ok(Json(NodesBatchResponse { nodes }))
}
