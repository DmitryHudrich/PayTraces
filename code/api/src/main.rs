use std::sync::Arc;

#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use axum::{
    Router,
    extract::{Query, State},
    response::IntoResponse,
    routing::{get, post},
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::{AppConfig, Cli};
use domain::chain::ChainId;
use domain::error::DomainError;
use domain::ports::BlockRange;
use domain::primitives::{Address, Ratio};
use domain::trace::{TaintStrategy, TraceDirection, TraceLimits, TraceOrigin, TraceRequest};
use infra::fetch_wallet_api::MoralisEthSource;
use infra::{PostgresEntityRepository, PostgresTransferRepository};
use usecase::{
    build_transfer_graph::{BuildTransferGraphUseCase, GraphRequest},
    check_sanctions::CheckSanctionsUseCase,
    ingest_address::IngestAddressUseCase,
    score_address::ScoreAddressUseCase,
    trace_funds::TraceFundsUseCase,
};

mod config;

pub struct AppState {
    pub ingest: IngestAddressUseCase<MoralisEthSource, PostgresTransferRepository>,
    pub graph: BuildTransferGraphUseCase<PostgresTransferRepository>,
    pub trace: TraceFundsUseCase<PostgresTransferRepository, PostgresEntityRepository>,
    pub score: ScoreAddressUseCase<PostgresTransferRepository, PostgresEntityRepository>,
    pub sanctions: CheckSanctionsUseCase<PostgresEntityRepository>,
}

#[derive(OpenApi)]
#[openapi(
    paths(ingest_address, get_graph, score_address, check_sanctions, trace_funds),
    info(title = "Crypto Forensics API", version = "1.0.0")
)]
struct ApiDoc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = AppConfig::load(&Cli::parse())?;

    tracing_subscriber::fmt()
        .with_env_filter(cfg.log.build_filter())
        .init();

    let moralis_key = cfg
        .moralis
        .api_key
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("moralis.api_key is required"))?
        .to_owned();

    let mut client_builder = reqwest::Client::builder();
    if let Some(ref url) = cfg.proxy.socks_url
        && !url.is_empty()
    {
        client_builder = client_builder.proxy(reqwest::Proxy::all(url)?);
    }
    let http_client = client_builder.build()?;

    let pool = infra::pg::create_pool(&cfg.database.url)?;
    infra::pg::run_migrations(&pool).await?;

    let eth_source = MoralisEthSource::new(
        moralis_key,
        cfg.moralis.base_url.clone(),
        http_client,
        cfg.moralis.cache.into_domain(),
    );

    let state = Arc::new(AppState {
        ingest: IngestAddressUseCase::new(
            eth_source,
            PostgresTransferRepository::new(pool.clone()),
        ),
        graph: BuildTransferGraphUseCase::new(PostgresTransferRepository::new(pool.clone())),
        trace: TraceFundsUseCase::new(
            PostgresTransferRepository::new(pool.clone()),
            PostgresEntityRepository::new(pool.clone()),
        ),
        score: ScoreAddressUseCase::new(
            PostgresTransferRepository::new(pool.clone()),
            PostgresEntityRepository::new(pool.clone()),
        ),
        sanctions: CheckSanctionsUseCase::new(PostgresEntityRepository::new(pool.clone())),
    });

    let addr = format!("{}:{}", cfg.server.host, cfg.server.port);

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/ingest", post(ingest_address))
        .route("/graph", get(get_graph))
        .route("/score", get(score_address))
        .route("/sanctions", get(check_sanctions))
        .route("/trace", get(trace_funds))
        .with_state(state)
        .layer(TraceLayer::new_for_http().on_failure(()).on_response(
            |resp: &axum::response::Response,
             latency: std::time::Duration,
             _span: &tracing::Span| {
                if resp.status().is_server_error() {
                    tracing::error!(
                        status = resp.status().as_u16(),
                        latency_ms = latency.as_millis(),
                        "5xx"
                    );
                }
            },
        ));

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(addr, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}

fn parse_address(s: &str, chain: ChainId) -> Result<Address, ApiError> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    let bytes = hex::decode(s).map_err(|_| ApiError::bad_request("invalid address hex"))?;

    if chain == ChainId::ETH && bytes.len() != 20 {
        return Err(ApiError::bad_request(format!(
            "expected 20-byte address, got {} bytes",
            bytes.len()
        )));
    }
    Ok(Address::new(chain, bytes))
}

#[derive(Serialize)]
struct ErrBody {
    error: String,
}

pub enum ApiError {
    BadRequest(String),
    Internal(DomainError),
}

impl ApiError {
    fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::BadRequest(msg) => (
                axum::http::StatusCode::BAD_REQUEST,
                axum::Json(ErrBody { error: msg }),
            )
                .into_response(),
            Self::Internal(e) => {
                tracing::error!(error = %e, "domain error");
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(ErrBody {
                        error: "internal server error".into(),
                    }),
                )
                    .into_response()
            }
        }
    }
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct IngestQuery {
    pub address: String,
    pub from_block: Option<u64>,
    pub to_block: Option<u64>,
}

#[utoipa::path(
    post, path = "/ingest",
    params(IngestQuery),
    responses(
        (status = 200, description = "Ingested N transfers"),
        (status = 400, description = "Invalid address"),
        (status = 500, description = "Internal server error"),
    )
)]
pub async fn ingest_address(
    State(state): State<Arc<AppState>>,
    Query(q): Query<IngestQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let addr = parse_address(&q.address, ChainId::ETH)?;
    let range = BlockRange::new(q.from_block.unwrap_or(0), q.to_block.unwrap_or(u64::MAX));
    let count = state
        .ingest
        .execute(&addr, range)
        .await
        .map_err(ApiError::Internal)?;

    Ok(axum::Json(serde_json::json!({ "ingested": count })))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct GraphQuery {
    pub address: String,
    pub chain_id: Option<u32>,
    pub max_depth: Option<u32>,
    pub max_nodes: Option<usize>,
    pub from_block: Option<u64>,
    pub to_block: Option<u64>,
}

#[utoipa::path(
    get, path = "/graph",
    params(GraphQuery),
    responses(
        (status = 200, description = "Transfer graph"),
        (status = 400, description = "Invalid address"),
        (status = 500, description = "Internal server error"),
    )
)]
pub async fn get_graph(
    State(state): State<Arc<AppState>>,
    Query(q): Query<GraphQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let chain = ChainId::new(q.chain_id.unwrap_or(ChainId::ETH.value()));
    let addr = parse_address(&q.address, chain)?;

    let range = match (q.from_block, q.to_block) {
        (None, None) => None,
        (from, to) => Some(BlockRange::new(from.unwrap_or(0), to.unwrap_or(u64::MAX))),
    };

    let graph = state
        .graph
        .execute(
            &addr,
            GraphRequest {
                range,
                max_depth: q.max_depth.unwrap_or(3),
                max_nodes: q.max_nodes.unwrap_or(500),
            },
        )
        .await
        .map_err(ApiError::Internal)?;

    let nodes: Vec<_> = graph.nodes.iter().map(|a| hex::encode(a.bytes())).collect();
    let edges: Vec<_> = graph
        .edges
        .iter()
        .map(|t| {
            serde_json::json!({
                "tx_hash":  hex::encode(t.tx_ref().hash()),
                "index":    t.id().index(),
                "from":     hex::encode(t.from().bytes()),
                "to":       hex::encode(t.to().bytes()),
                "amount":   t.amount().raw().to_string(),
                "decimals": t.amount().decimals(),
                "block":    t.block().height(),
                "ts":       t.timestamp().timestamp(),
                "kind":     format!("{:?}", t.kind()),
            })
        })
        .collect();

    Ok(axum::Json(serde_json::json!({
        "node_count": nodes.len(),
        "edge_count": edges.len(),
        "nodes": nodes,
        "edges": edges,
    })))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ScoreQuery {
    pub address: String,
    pub chain_id: Option<u32>,
}

#[utoipa::path(
    get, path = "/score",
    params(ScoreQuery),
    responses(
        (status = 200, description = "Risk score report"),
        (status = 400, description = "Invalid address"),
        (status = 500, description = "Internal server error"),
    )
)]
pub async fn score_address(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ScoreQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let chain = ChainId::new(q.chain_id.unwrap_or(ChainId::ETH.value()));
    let addr = parse_address(&q.address, chain)?;

    let report = state
        .score
        .execute(&addr)
        .await
        .map_err(ApiError::Internal)?;

    let signals: Vec<_> = report
        .signals()
        .iter()
        .map(|s| {
            serde_json::json!({
                "kind":        format!("{:?}", s.kind()),
                "severity":    s.severity().value(),
                "description": s.description(),
            })
        })
        .collect();

    Ok(axum::Json(serde_json::json!({
        "address":      hex::encode(report.subject().bytes()),
        "score":        report.overall_score().value(),
        "is_high_risk": report.is_high_risk(),
        "signals":      signals,
        "generated_at": report.generated_at().to_rfc3339(),
    })))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct SanctionsQuery {
    pub address: String,
    pub chain_id: Option<u32>,
}

#[utoipa::path(
    get, path = "/sanctions",
    params(SanctionsQuery),
    responses(
        (status = 200, description = "Sanctions check result"),
        (status = 400, description = "Invalid address"),
        (status = 500, description = "Internal server error"),
    )
)]
pub async fn check_sanctions(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SanctionsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let chain = ChainId::new(q.chain_id.unwrap_or(ChainId::ETH.value()));
    let addr = parse_address(&q.address, chain)?;

    let result = state
        .sanctions
        .execute(&addr)
        .await
        .map_err(ApiError::Internal)?;

    Ok(axum::Json(serde_json::json!({
        "address":       hex::encode(result.address.bytes()),
        "is_sanctioned": result.is_sanctioned,
        "sanction_list": result.sanction_list.as_ref().map(|l| format!("{:?}", l)),
        "label":         result.label,
    })))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct TraceQuery {
    pub address: String,
    pub chain_id: Option<u32>,

    pub direction: Option<String>,

    pub strategy: Option<String>,
    pub max_hops: Option<u32>,
    pub max_addresses: Option<usize>,
}

#[utoipa::path(
    get, path = "/trace",
    params(TraceQuery),
    responses(
        (status = 200, description = "Trace result with paths and sinks"),
        (status = 400, description = "Invalid address or parameters"),
        (status = 500, description = "Internal server error"),
    )
)]
pub async fn trace_funds(
    State(state): State<Arc<AppState>>,
    Query(q): Query<TraceQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let chain = ChainId::new(q.chain_id.unwrap_or(ChainId::ETH.value()));
    let addr = parse_address(&q.address, chain)?;

    let direction = match q.direction.as_deref().unwrap_or("forward") {
        "forward" => TraceDirection::Forward,
        "backward" => TraceDirection::Backward,
        "both" => TraceDirection::Both,
        other => return Err(ApiError::bad_request(format!("unknown direction: {other}"))),
    };

    let strategy = match q.strategy.as_deref().unwrap_or("haircut") {
        "haircut" => TaintStrategy::Haircut,
        "poison" => TaintStrategy::Poison,
        "fifo" => TaintStrategy::Fifo,
        "lifo" => TaintStrategy::Lifo,
        other => return Err(ApiError::bad_request(format!("unknown strategy: {other}"))),
    };

    let result = state
        .trace
        .execute(TraceRequest::new(
            TraceOrigin::Address(addr),
            direction,
            strategy,
            TraceLimits::new(
                q.max_hops.unwrap_or(10),
                q.max_addresses.unwrap_or(1_000),
                500,
                Some(Ratio::from_percent(1)),
            ),
            false,
        ))
        .await
        .map_err(ApiError::Internal)?;

    let sinks: Vec<_> = result
        .terminal_sinks()
        .iter()
        .map(|s| {
            serde_json::json!({
                "address":        hex::encode(s.address().bytes()),
                "kind":           format!("{:?}", s.kind()),
                "risk_score":     s.risk_score(),
                "tainted_amount": s.tainted_amount().raw().to_string(),
                "taint_ratio":    s.taint_ratio().as_f64(),
            })
        })
        .collect();

    let paths: Vec<_> = result
        .paths()
        .iter()
        .map(|p| {
            serde_json::json!({
                "depth":          p.depth(),
                "tainted_amount": p.tainted_amount().raw().to_string(),
                "taint_ratio":    p.taint_ratio().as_f64(),
                "hops":           p.hops().len(),
                "origin":         p.origin().map(|a| hex::encode(a.bytes())),
                "destination":    p.destination().map(|a| hex::encode(a.bytes())),
            })
        })
        .collect();

    Ok(axum::Json(serde_json::json!({
        "stats": {
            "addresses_visited":   result.stats().addresses_visited(),
            "transfers_evaluated": result.stats().transfers_evaluated(),
            "paths_found":         result.stats().paths_found(),
            "depth_reached":       result.stats().depth_reached(),
            "truncated":           result.stats().truncated(),
        },
        "sinks": sinks,
        "paths": paths,
    })))
}
