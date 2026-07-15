use std::sync::Arc;

use axum::{
    Json,
    extract::{Query, State},
    response::IntoResponse,
};
use domain::graph::GraphRequest;
use domain::ports::BlockRange;
use serde::{Deserialize, Serialize};

use crate::error::{ApiError, ErrorResponse};
use crate::format::{
    deserialize_height_or_date, edge_symbol, format_amount, native_symbol, parse_address,
    transfer_kind_str,
};
use crate::handlers::nodes::{GraphViewContext, NodeDto, enrich_nodes, parse_minimal};
use crate::state::{AppState, resolve_chain_id};

#[derive(Serialize, utoipa::ToSchema)]
pub struct EdgeDto {
    #[schema(example = "a1b2c3...")]
    tx_hash: String,
    index: u32,
    from: String,
    to: String,
    #[schema(example = "1000000000000000000")]
    raw: String,
    #[schema(example = "1.0")]
    formatted: String,
    #[schema(example = "ETH")]
    symbol: String,
    decimals: u8,
    block: u64,
    ts: i64,
    kind: String,
    contract: Option<String>,
    chain_id: u32,
}

/// One page of the persisted transfer graph. Each node carries
/// `kind`/`risk_score`/`tags`/degree/boundary-flags so callers don't need
/// N+1 follow-up requests (ТЗ §3).
#[derive(Serialize, utoipa::ToSchema)]
pub struct GraphPage {
    total_nodes: usize,
    total_edges: usize,
    page: u32,
    page_size: usize,
    total_pages: u32,
    has_next: bool,
    nodes: Vec<NodeDto>,
    edges: Vec<EdgeDto>,
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct GraphQuery {
    #[param(example = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")]
    address: String,
    #[param(example = 1)]
    chain_id: Option<u32>,
    #[param(example = 2, minimum = 1)]
    max_depth: Option<u32>,
    #[param(example = 500)]
    max_nodes: Option<usize>,
    #[param(example = 10000)]
    max_transfers_per_address: Option<usize>,
    /// Block number for most chains; for Tron, ms-since-epoch or a
    /// Tronscan-style UTC date string (`"2026-07-13 08:04:09"`).
    #[param(example = 19000000)]
    #[serde(default, deserialize_with = "deserialize_height_or_date")]
    from_block: Option<u64>,
    /// Block number for most chains; for Tron, ms-since-epoch or a
    /// Tronscan-style UTC date string (`"2026-07-13 08:04:09"`).
    #[param(example = 20000000)]
    #[serde(default, deserialize_with = "deserialize_height_or_date")]
    to_block: Option<u64>,
    #[param(example = 0)]
    page: Option<u32>,
    #[param(example = 100)]
    page_size: Option<usize>,
    /// `full` (default) populates every `NodeDto` field; `minimal` returns
    /// just `address`, skipping the enrichment queries entirely.
    #[param(example = "full")]
    enrich: Option<String>,
}

#[utoipa::path(
    get, path = "/graph",
    description = "Read the persisted transfer graph around an address, paginated by edge.\n\n\
                   ## What this does\n\n\
                   Walks the persisted graph in PostgreSQL outward from `address`, BFS-bounded \
                   by `max_depth` hops and `max_nodes` distinct counterparties, then returns the \
                   discovered transfers paginated by edge. This endpoint never contacts a chain \
                   RPC; it is a pure read against whatever ingestion has already persisted, so \
                   the response is fast and deterministic but bounded by what has already been \
                   ingested.\n\n\
                   ## When to use it\n\n\
                   Run `POST /jobs/ingest` for the same address first and wait for the job to \
                   reach `succeeded`. Then call this endpoint to render the graph, drive a \
                   visualisation, or feed it into downstream tooling. If you call it before \
                   ingestion, you will get an empty graph rather than an error.\n\n\
                   ## Example\n\n\
                   ```bash\n\
                   curl 'http://localhost:8080/graph?address=0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045&chain_id=1&max_depth=2&page=0&page_size=100'\n\
                   ```\n\n\
                   ## Notes\n\n\
                   The `nodes` array is only returned on `page == 0` because the node set is \
                   global per request; subsequent pages send an empty `nodes` array to save \
                   bandwidth. `page_size` is clamped to `[1, 1000]`. The `from_block` / \
                   `to_block` filters apply to the persisted edges only.\n\n\
                   ## Node enrichment\n\n\
                   Each `nodes` entry (`NodeDto`) carries `kind`, `risk_score` (cache-only, \
                   never computed inline — `null` on a cache miss), every active `tags` entry (not \
                   just the top one), `in_degree`/`out_degree`/`tx_count`, and two boundary flags — \
                   `is_view_boundary` (this node exists in the DB with more edges than shown here; \
                   BFS just didn't expand it — re-query, don't re-ingest) and `is_ingest_boundary` \
                   (nothing has ever been persisted for this address — `POST /jobs/ingest` first). \
                   Closes the `/graph` + N×`/address/.../kind` + N×`/score` + N×`/labels/{addr}` \
                   round-trip. Pass `?enrich=minimal` to get only `address` per node (skips \
                   every enrichment query, including the boundary flags).",
    params(GraphQuery),
    responses(
        (status = 200, body = GraphPage),
        (status = 400, body = ErrorResponse),
        (status = 500, body = ErrorResponse),
    ),
    tag = "Graph"
)]
pub async fn get_graph(
    State(state): State<Arc<AppState>>,
    Query(q): Query<GraphQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let chain = resolve_chain_id(&state, q.chain_id);
    let addr = parse_address(&q.address, chain)?;

    let range = match (q.from_block, q.to_block) {
        (None, None) => None,
        (from, to) => Some(BlockRange::new(from.unwrap_or(0), to.unwrap_or(u64::MAX))),
    };

    let graph = state
        .ingestion()
        .build_graph_from_db(
            &addr,
            GraphRequest::new(
                range,
                q.max_depth.unwrap_or(3),
                q.max_nodes.unwrap_or(500),
                q.max_transfers_per_address.unwrap_or(10_000),
            ),
        )
        .await
        .map_err(ApiError::Internal)?;

    let page = q.page.unwrap_or(0) as usize;
    let page_size = q.page_size.unwrap_or(100).clamp(1, 1000);

    let total_nodes = graph.nodes().len();
    let total_edges = graph.edges().len();
    let total_pages = total_edges.div_ceil(page_size).max(1) as u32;

    let start = page * page_size;
    let edge_slice = graph.edges().get(start..).unwrap_or(&[]);
    let edge_page = &edge_slice[..edge_slice.len().min(page_size)];

    let native = native_symbol(state.chains(), chain);

    let edges: Vec<EdgeDto> = edge_page
        .iter()
        .map(|t| {
            let (kind, contract) = transfer_kind_str(t.kind());
            EdgeDto {
                tx_hash: hex::encode(t.tx_ref().hash()),
                index: t.id().index(),
                from: t.from().canonical(),
                to: t.to().canonical(),
                raw: t.amount().raw().to_string(),
                formatted: format_amount(t.amount().raw(), t.amount().decimals()),
                symbol: edge_symbol(t.kind(), &native),
                decimals: t.amount().decimals(),
                block: t.block().height(),
                ts: t.timestamp().timestamp(),
                kind: kind.into(),
                contract,
                chain_id: t.chain().value(),
            }
        })
        .collect();

    let minimal = parse_minimal(q.enrich.as_deref())?;
    let nodes: Vec<NodeDto> = if page == 0 {
        let node_addrs: Vec<_> = graph.nodes().iter().cloned().collect();
        let degrees = graph.degrees();
        let view = GraphViewContext {
            degrees: &degrees,
            expanded: graph.expanded(),
        };
        enrich_nodes(&state, &node_addrs, Some(view), minimal).await?
    } else {
        Vec::new()
    };

    Ok(Json(GraphPage {
        total_nodes,
        total_edges,
        page: page as u32,
        page_size,
        total_pages,
        has_next: page as u32 + 1 < total_pages,
        nodes,
        edges,
    }))
}
