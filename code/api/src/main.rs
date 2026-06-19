use std::sync::Arc;

#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use axum::{
    Router,
    extract::{Query, State},
    response::IntoResponse,
    routing::get,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::{AppConfig, Cli, TelemetryConfig};
use domain::chain::{ChainId, ChainRegistry};
use domain::error::DomainError;
use domain::graph::GraphRequest;
use domain::ports::{BlockRange, IngestionPort, RiskPort};
use domain::primitives::{Address, Ratio};
use domain::trace::{TaintStrategy, TraceDirection, TraceLimits, TraceOrigin, TraceRequest};
use infra::fetch_wallet_api::MoralisEthSource;
use infra::{
    ChainSources, PostgresEntityRepository, PostgresTransferRepository, TronGridSource,
};
use usecase::{IngestionService, RiskService};

mod config;

pub struct AppState {
    ingestion: IngestionService<ChainSources, PostgresTransferRepository>,
    risk: RiskService<PostgresTransferRepository, PostgresEntityRepository>,
    chains: ChainRegistry,
}

impl AppState {
    pub fn new(
        ingestion: IngestionService<ChainSources, PostgresTransferRepository>,
        risk: RiskService<PostgresTransferRepository, PostgresEntityRepository>,
        chains: ChainRegistry,
    ) -> Self {
        Self { ingestion, risk, chains }
    }

    pub fn ingestion(&self) -> &IngestionService<ChainSources, PostgresTransferRepository> {
        &self.ingestion
    }

    pub fn risk(&self) -> &RiskService<PostgresTransferRepository, PostgresEntityRepository> {
        &self.risk
    }

    pub fn chains(&self) -> &ChainRegistry {
        &self.chains
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "PayTraces — Crypto Forensics API",
        version = "1.0.0",
        description = "On-chain transfer graph construction, fund tracing and risk scoring \
                       for EVM-compatible blockchains.\n\n\
                       **Data source:** Moralis Deep Index API (Ethereum mainnet).\n\
                       Graph endpoints auto-ingest counterparty addresses on the fly \
                       and persist results to PostgreSQL for subsequent use by /trace and /score."
    ),
    paths(get_graph, score_address, check_sanctions, trace_funds, list_chains),
    components(schemas(
        GraphPage, EdgeDto,
        ScoreResponse, SignalDto,
        SanctionsResponse,
        TraceResponse, TraceStatsDto, SinkDto, PathDto,
        ChainsResponse, ChainDto,
        ErrorResponse
    ))
)]
struct ApiDoc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = AppConfig::load(&Cli::parse())?;

    let provider = if cfg.telemetry().enabled() {
        Some(init_tracer(cfg.telemetry())?)
    } else {
        None
    };

    let otel_layer = provider.as_ref().map(|p| {
        use opentelemetry::trace::TracerProvider as _;
        tracing_opentelemetry::layer().with_tracer(p.tracer("api"))
    });

    tracing_subscriber::registry()
        .with(cfg.log().build_filter())
        .with(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_writer(std::io::stderr),
        )
        .with(otel_layer)
        .init();

    if cfg.telemetry().enabled() {
        tracing::info!(
            endpoint = %cfg.telemetry().otlp_endpoint(),
            service = %cfg.telemetry().service_name(),
            "OTLP tracing enabled"
        );
    }

    let mut client_builder = reqwest::Client::builder();
    if let Some(url) = cfg.proxy().socks_url()
        && !url.is_empty()
    {
        client_builder = client_builder.proxy(reqwest::Proxy::all(url)?);
    }
    let http_client = client_builder.build()?;

    let pool = infra::pg::create_pool(cfg.database().url())?;
    infra::pg::run_migrations(&pool).await?;

    let mut sources = ChainSources::builder();

    if let Some(key) = cfg.moralis().api_key().filter(|k| !k.is_empty()) {
        let eth_source = MoralisEthSource::new(
            key.to_owned(),
            cfg.moralis().base_url().to_owned(),
            http_client.clone(),
            cfg.moralis().cache().clone().into_domain(),
        )
        .await;
        sources = sources.register(eth_source);
        tracing::info!(chain = "eth", "registered Moralis ETH source");
    } else {
        tracing::warn!("moralis.api_key not set — Ethereum chain disabled");
    }

    if cfg.trongrid().enabled() {
        let tron_source =
            TronGridSource::new(http_client.clone(), cfg.trongrid().clone().into_domain());
        sources = sources.register(tron_source);
        tracing::info!(chain = "tron", "registered TronGrid source");
    }

    let sources = sources.build();
    if sources.is_empty() {
        anyhow::bail!("no chain sources registered — check moralis/trongrid config");
    }

    let chain_registry = ChainRegistry::default_registry();

    let state = Arc::new(AppState::new(
        IngestionService::new(
            sources,
            PostgresTransferRepository::new(pool.clone()),
        ),
        RiskService::new(
            PostgresTransferRepository::new(pool.clone()),
            PostgresEntityRepository::new(pool.clone()),
        ),
        chain_registry,
    ));

    let addr = format!("{}:{}", cfg.server().host(), cfg.server().port());

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/chains", get(list_chains))
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
    tokio::select! {
        result = axum::serve(listener, app) => { result?; }
        _ = shutdown_signal() => {}
    }

    if let Some(p) = provider {
        tracing::info!("flushing OTLP spans");
        let result = tokio::task::spawn_blocking(move || {
            if let Err(e) = p.force_flush() {
                eprintln!("WARN: force_flush failed: {e}");
            }
            if let Err(e) = p.shutdown() {
                eprintln!("ERROR: tracer provider shutdown failed: {e}");
            }
        })
        .await;
        if let Err(e) = result {
            tracing::error!(error = %e, "tracer shutdown task panicked");
        }
    }
    Ok(())
}

async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = sigterm.recv() => {},
    }
    tracing::info!("shutdown signal received");
}

fn init_tracer(
    cfg: &TelemetryConfig,
) -> anyhow::Result<opentelemetry_sdk::trace::SdkTracerProvider> {
    use opentelemetry::KeyValue;
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::Resource;
    use opentelemetry_sdk::trace::{BatchConfigBuilder, BatchSpanProcessor, SdkTracerProvider};

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(cfg.otlp_endpoint())
        .build()?;

    let resource = Resource::builder_empty()
        .with_attribute(KeyValue::new("service.name", cfg.service_name().to_string()))
        .with_attribute(KeyValue::new(
            "service.version",
            env!("CARGO_PKG_VERSION"),
        ))
        .build();

    let processor = BatchSpanProcessor::builder(exporter)
        .with_batch_config(
            BatchConfigBuilder::default()
                .with_scheduled_delay(std::time::Duration::from_secs(1))
                .build(),
        )
        .build();

    let provider = SdkTracerProvider::builder()
        .with_span_processor(processor)
        .with_resource(resource)
        .build();

    opentelemetry::global::set_tracer_provider(provider.clone());
    Ok(provider)
}

fn parse_address(s: &str, chain: ChainId) -> Result<Address, ApiError> {
    Address::parse(chain, s).map_err(|e| ApiError::bad_request(e.to_string()))
}

// ── Response DTOs ────────────────────────────────────────────────────────────

/// A single transfer edge in the graph.
#[derive(Serialize, utoipa::ToSchema)]
pub struct EdgeDto {
    /// Transaction hash (64 hex chars, no 0x prefix)
    #[schema(example = "a1b2c3...")]
    tx_hash: String,
    /// Log index within the transaction (0 for native transfers)
    index: u32,
    /// Sender address (40 hex chars, no 0x prefix)
    from: String,
    /// Recipient address (40 hex chars, no 0x prefix)
    to: String,
    /// Raw amount (integer, no decimals applied)
    #[schema(example = "1000000000000000000")]
    amount: String,
    /// Token decimals (18 for ETH and most ERC-20 tokens)
    decimals: u8,
    /// Block height
    block: u64,
    /// Unix timestamp (seconds)
    ts: i64,
    /// Transfer kind: `"Native"` or `"Token { contract, standard }"`
    kind: String,
    chain_id: u32,
}

impl EdgeDto {
    pub fn new(
        tx_hash: String,
        index: u32,
        from: String,
        to: String,
        amount: String,
        decimals: u8,
        block: u64,
        ts: i64,
        kind: String,
        chain_id: u32,
    ) -> Self {
        Self { tx_hash, index, from, to, amount, decimals, block, ts, kind, chain_id }
    }
}

/// Paginated transfer graph response.
#[derive(Serialize, utoipa::ToSchema)]
pub struct GraphPage {
    /// Total number of unique addresses (nodes) in the full graph
    total_nodes: usize,
    /// Total number of transfers (edges) in the full graph
    total_edges: usize,
    /// Current page index (0-based)
    page: u32,
    /// Edges returned per page
    page_size: usize,
    /// Total number of pages
    total_pages: u32,
    /// Whether a next page exists
    has_next: bool,
    /// All unique addresses in the graph (returned on every page for convenience)
    nodes: Vec<String>,
    /// Transfers on the current page
    edges: Vec<EdgeDto>,
}

impl GraphPage {
    pub fn new(
        total_nodes: usize,
        total_edges: usize,
        page: u32,
        page_size: usize,
        total_pages: u32,
        has_next: bool,
        nodes: Vec<String>,
        edges: Vec<EdgeDto>,
    ) -> Self {
        Self { total_nodes, total_edges, page, page_size, total_pages, has_next, nodes, edges }
    }
}

/// A risk signal attached to an address score.
#[derive(Serialize, utoipa::ToSchema)]
pub struct SignalDto {
    /// Signal category (e.g. `SanctionedCounterparty`, `MixerInteraction`)
    kind: String,
    /// Severity 0–100 (≥70 = HIGH, ≥90 = CRITICAL)
    severity: u8,
    /// Human-readable explanation
    description: String,
}

impl SignalDto {
    pub fn new(kind: String, severity: u8, description: String) -> Self {
        Self { kind, severity, description }
    }
}

/// Risk score report for an address.
#[derive(Serialize, utoipa::ToSchema)]
pub struct ScoreResponse {
    address: String,
    chain_id: u32,
    /// Aggregate risk score 0–100
    score: u8,
    /// `true` when score ≥ 70
    is_high_risk: bool,
    /// Individual risk signals that contributed to the score
    signals: Vec<SignalDto>,
    /// ISO-8601 timestamp when the report was generated
    generated_at: String,
}

impl ScoreResponse {
    pub fn new(
        address: String,
        chain_id: u32,
        score: u8,
        is_high_risk: bool,
        signals: Vec<SignalDto>,
        generated_at: String,
    ) -> Self {
        Self { address, chain_id, score, is_high_risk, signals, generated_at }
    }
}

/// OFAC / sanctions screening result.
#[derive(Serialize, utoipa::ToSchema)]
pub struct SanctionsResponse {
    address: String,
    chain_id: u32,
    /// Whether the address appears on a sanctions list
    is_sanctioned: bool,
    /// Sanctions list name, if applicable
    sanction_list: Option<String>,
    /// Known entity label, if applicable
    label: Option<String>,
}

impl SanctionsResponse {
    pub fn new(
        address: String,
        chain_id: u32,
        is_sanctioned: bool,
        sanction_list: Option<String>,
        label: Option<String>,
    ) -> Self {
        Self { address, chain_id, is_sanctioned, sanction_list, label }
    }
}

/// Aggregate statistics for a trace run.
#[derive(Serialize, utoipa::ToSchema)]
pub struct TraceStatsDto {
    addresses_visited: usize,
    transfers_evaluated: usize,
    paths_found: usize,
    /// Deepest hop reached
    depth_reached: u32,
    /// `true` when the run hit a limit (max_hops, max_addresses, or max_paths)
    truncated: bool,
}

impl TraceStatsDto {
    pub fn new(
        addresses_visited: usize,
        transfers_evaluated: usize,
        paths_found: usize,
        depth_reached: u32,
        truncated: bool,
    ) -> Self {
        Self { addresses_visited, transfers_evaluated, paths_found, depth_reached, truncated }
    }
}

/// A terminal sink discovered during tracing.
#[derive(Serialize, utoipa::ToSchema)]
pub struct SinkDto {
    address: String,
    /// Sink category: `Exchange`, `Bridge`, `Mixer`, `Sanctioned`, `Darknet`, `Unresolved`
    kind: String,
    /// Risk score of this sink 0–100
    risk_score: u8,
    /// Raw tainted amount that reached this sink
    tainted_amount: String,
    /// Fraction of the sink's incoming funds that are tainted (0.0–1.0)
    taint_ratio: f64,
}

impl SinkDto {
    pub fn new(
        address: String,
        kind: String,
        risk_score: u8,
        tainted_amount: String,
        taint_ratio: f64,
    ) -> Self {
        Self { address, kind, risk_score, tainted_amount, taint_ratio }
    }
}

/// One traced fund flow path from origin to a sink.
#[derive(Serialize, utoipa::ToSchema)]
pub struct PathDto {
    /// Number of hops in this path
    depth: u32,
    /// Raw tainted amount along this path
    tainted_amount: String,
    /// Taint ratio at the end of the path (0.0–1.0)
    taint_ratio: f64,
    /// Number of transfers in this path
    hops: usize,
    origin: Option<String>,
    destination: Option<String>,
}

impl PathDto {
    pub fn new(
        depth: u32,
        tainted_amount: String,
        taint_ratio: f64,
        hops: usize,
        origin: Option<String>,
        destination: Option<String>,
    ) -> Self {
        Self { depth, tainted_amount, taint_ratio, hops, origin, destination }
    }
}

/// Fund trace result.
#[derive(Serialize, utoipa::ToSchema)]
pub struct TraceResponse {
    stats: TraceStatsDto,
    sinks: Vec<SinkDto>,
    paths: Vec<PathDto>,
}

impl TraceResponse {
    pub fn new(stats: TraceStatsDto, sinks: Vec<SinkDto>, paths: Vec<PathDto>) -> Self {
        Self { stats, sinks, paths }
    }
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ChainDto {
    id: u32,
    name: String,
    family: String,
    address_model: String,
    address_encoding: String,
    native_symbol: String,
    native_decimals: u8,
    confirmation_depth: u64,
    source_registered: bool,
}

impl ChainDto {
    pub fn new(
        id: u32,
        name: String,
        family: String,
        address_model: String,
        address_encoding: String,
        native_symbol: String,
        native_decimals: u8,
        confirmation_depth: u64,
        source_registered: bool,
    ) -> Self {
        Self {
            id, name, family, address_model, address_encoding,
            native_symbol, native_decimals, confirmation_depth, source_registered,
        }
    }
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ChainsResponse {
    chains: Vec<ChainDto>,
}

impl ChainsResponse {
    pub fn new(chains: Vec<ChainDto>) -> Self {
        Self { chains }
    }
}

/// Error response body.
#[derive(Serialize, utoipa::ToSchema)]
pub struct ErrorResponse {
    error: String,
}

impl ErrorResponse {
    pub fn new(error: String) -> Self {
        Self { error }
    }
}

// ── API error type ───────────────────────────────────────────────────────────

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

#[utoipa::path(
    get, path = "/chains",
    responses(
        (status = 200, description = "All chains known to the registry", body = ChainsResponse),
    ),
    tag = "Discovery"
)]
pub async fn list_chains(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    use domain::chain::{AddressEncoding, AddressModel, ChainFamily};
    let registered: std::collections::HashSet<ChainId> = {
        use domain::ports::ChainSourceRegistry;
        state.ingestion().sources().supported_chains().into_iter().collect()
    };
    let chains = state
        .chains()
        .all()
        .iter()
        .map(|m| ChainDto::new(
            m.id().value(),
            m.name().to_string(),
            match m.family() {
                ChainFamily::Evm => "evm",
                ChainFamily::Tron => "tron",
                ChainFamily::Bitcoin => "bitcoin",
                ChainFamily::Solana => "solana",
                ChainFamily::Other => "other",
            }
            .into(),
            match m.address_model() {
                AddressModel::Account => "account",
                AddressModel::Utxo => "utxo",
            }
            .into(),
            match m.address_encoding() {
                AddressEncoding::Hex20 => "hex20",
                AddressEncoding::TronBase58Check => "tron_base58_check",
                AddressEncoding::Bech32 => "bech32",
                AddressEncoding::Base58 => "base58",
            }
            .into(),
            m.native_asset_symbol().to_string(),
            m.native_asset_decimals(),
            m.confirmation_depth(),
            registered.contains(&m.id()),
        ))
        .collect();
    axum::Json(ChainsResponse::new(chains))
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct GraphQuery {
    /// Origin wallet address
    #[param(example = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")]
    address: String,

    /// Chain ID. Defaults to 1 (Ethereum mainnet).
    #[param(example = 1)]
    chain_id: Option<u32>,

    /// How many hops to traverse from the origin. Defaults to 3.
    #[param(example = 2, minimum = 1)]
    max_depth: Option<u32>,

    /// Hard cap on the total number of nodes in the graph. Defaults to 500.
    #[param(example = 500)]
    max_nodes: Option<usize>,

    /// Restrict transfers to blocks ≥ this height (inclusive).
    #[param(example = 19000000)]
    from_block: Option<u64>,

    /// Restrict transfers to blocks ≤ this height (inclusive).
    #[param(example = 20000000)]
    to_block: Option<u64>,

    /// Page index (0-based). Defaults to 0.
    #[param(example = 0)]
    page: Option<u32>,

    /// Number of edges per page. Defaults to 100, maximum 1000.
    #[param(example = 100)]
    page_size: Option<usize>,
}

#[utoipa::path(
    get, path = "/graph",
    params(GraphQuery),
    responses(
        (status = 200, description = "Paginated transfer graph", body = GraphPage),
        (status = 400, description = "Invalid address or parameters", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
    tag = "Graph"
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
        .ingestion()
        .build_graph(
            &addr,
            GraphRequest::new(
                range,
                q.max_depth.unwrap_or(3),
                q.max_nodes.unwrap_or(500),
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

    let nodes: Vec<String> = graph.nodes().iter().map(|a| a.canonical()).collect();
    let edges: Vec<EdgeDto> = edge_page
        .iter()
        .map(|t| EdgeDto::new(
            hex::encode(t.tx_ref().hash()),
            t.id().index(),
            t.from().canonical(),
            t.to().canonical(),
            t.amount().raw().to_string(),
            t.amount().decimals(),
            t.block().height(),
            t.timestamp().timestamp(),
            format!("{:?}", t.kind()),
            t.chain().value(),
        ))
        .collect();

    Ok(axum::Json(GraphPage::new(
        total_nodes,
        total_edges,
        page as u32,
        page_size,
        total_pages,
        page as u32 + 1 < total_pages,
        nodes,
        edges,
    )))
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ScoreQuery {
    #[param(example = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")]
    address: String,
    #[param(example = 1)]
    chain_id: Option<u32>,
}

#[utoipa::path(
    get, path = "/score",
    params(ScoreQuery),
    responses(
        (status = 200, description = "Risk score report for the address", body = ScoreResponse),
        (status = 400, description = "Invalid address", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
    tag = "Risk"
)]
pub async fn score_address(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ScoreQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let chain = ChainId::new(q.chain_id.unwrap_or(ChainId::ETH.value()));
    let addr = parse_address(&q.address, chain)?;

    let report = state
        .risk()
        .score(&addr)
        .await
        .map_err(ApiError::Internal)?;

    let signals: Vec<SignalDto> = report
        .signals()
        .iter()
        .map(|s| SignalDto::new(
            format!("{:?}", s.kind()),
            s.severity().value(),
            s.description().to_string(),
        ))
        .collect();

    Ok(axum::Json(ScoreResponse::new(
        report.subject().canonical(),
        report.subject().chain().value(),
        report.overall_score().value(),
        report.is_high_risk(),
        signals,
        report.generated_at().to_rfc3339(),
    )))
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SanctionsQuery {
    #[param(example = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")]
    address: String,
    #[param(example = 1)]
    chain_id: Option<u32>,
}

#[utoipa::path(
    get, path = "/sanctions",
    params(SanctionsQuery),
    responses(
        (status = 200, description = "OFAC / sanctions screening result", body = SanctionsResponse),
        (status = 400, description = "Invalid address", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
    tag = "Risk"
)]
pub async fn check_sanctions(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SanctionsQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let chain = ChainId::new(q.chain_id.unwrap_or(ChainId::ETH.value()));
    let addr = parse_address(&q.address, chain)?;

    let result = state
        .risk()
        .check_sanctions(&addr)
        .await
        .map_err(ApiError::Internal)?;

    Ok(axum::Json(SanctionsResponse::new(
        result.address().canonical(),
        result.address().chain().value(),
        result.is_sanctioned(),
        result.sanction_list().map(|l| format!("{:?}", l)),
        result.label().map(str::to_string),
    )))
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TraceQuery {
    #[param(example = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")]
    address: String,
    #[param(example = 1)]
    chain_id: Option<u32>,
    #[param(example = "forward")]
    direction: Option<String>,
    #[param(example = "haircut")]
    strategy: Option<String>,
    #[param(example = 5)]
    max_hops: Option<u32>,
    #[param(example = 500)]
    max_addresses: Option<usize>,
}

#[utoipa::path(
    get, path = "/trace",
    params(TraceQuery),
    responses(
        (status = 200, description = "Fund trace result with paths and terminal sinks", body = TraceResponse),
        (status = 400, description = "Invalid address or parameter value", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
    tag = "Risk"
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
        .risk()
        .trace(TraceRequest::new(
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

    let sinks: Vec<SinkDto> = result
        .terminal_sinks()
        .iter()
        .map(|s| SinkDto::new(
            s.address().canonical(),
            format!("{:?}", s.kind()),
            s.risk_score(),
            s.tainted_amount().raw().to_string(),
            s.taint_ratio().as_f64(),
        ))
        .collect();

    let paths: Vec<PathDto> = result
        .paths()
        .iter()
        .map(|p| PathDto::new(
            p.depth(),
            p.tainted_amount().raw().to_string(),
            p.taint_ratio().as_f64(),
            p.hops().len(),
            p.origin().map(|a| a.canonical()),
            p.destination().map(|a| a.canonical()),
        ))
        .collect();

    Ok(axum::Json(TraceResponse::new(
        TraceStatsDto::new(
            result.stats().addresses_visited(),
            result.stats().transfers_evaluated(),
            result.stats().paths_found(),
            result.stats().depth_reached(),
            result.stats().truncated(),
        ),
        sinks,
        paths,
    )))
}
