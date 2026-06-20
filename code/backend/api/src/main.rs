use std::sync::Arc;

#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    middleware::{self, Next},
    response::IntoResponse,
    routing::{get, post},
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::{AppConfig, Cli, TelemetryConfig};
use domain::chain::{ChainId, ChainRegistry};
use domain::entity::SanctionList;
use domain::error::DomainError;
use domain::graph::GraphRequest;
use domain::ports::{BlockRange, IngestionPort, RiskPort};
use domain::primitives::{Address, Ratio, U256};
use domain::risk::RiskSignalKind;
use domain::trace::{
    SinkKind, TaintStrategy, TraceDirection, TraceLimits, TraceOrigin, TraceRequest,
};
use domain::transfer::TransferKind;
use infra::fetch_wallet_api::MoralisEthSource;
use infra::{
    ChainSources, JobRepository, PostgresEntityRepository, PostgresTransferRepository,
    TronGridSource,
};
use usecase::{IngestionService, RiskService};

mod config;

pub struct AppState {
    ingestion: IngestionService<ChainSources, PostgresTransferRepository>,
    risk: RiskService<PostgresTransferRepository, PostgresEntityRepository>,
    chains: ChainRegistry,
    jobs: JobRepository,
    api_key: Option<String>,
}

impl AppState {
    pub fn new(
        ingestion: IngestionService<ChainSources, PostgresTransferRepository>,
        risk: RiskService<PostgresTransferRepository, PostgresEntityRepository>,
        chains: ChainRegistry,
        jobs: JobRepository,
        api_key: Option<String>,
    ) -> Self {
        Self {
            ingestion,
            risk,
            chains,
            jobs,
            api_key,
        }
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

    pub fn jobs(&self) -> &JobRepository {
        &self.jobs
    }

    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }
}

struct ApiSecurity;
impl utoipa::Modify for ApiSecurity {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
        let components = openapi
            .components
            .get_or_insert_with(utoipa::openapi::Components::new);
        components.add_security_scheme(
            "api_version",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                "X-API-Version",
                "Required for every request. Supported value: `1`.",
            ))),
        );
        components.add_security_scheme(
            "api_key",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                "X-Api-Key",
                "Required only when server.api_key is configured. Bearer-token form \
                 (Authorization: Bearer <key>) is also accepted.",
            ))),
        );
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "PayTraces — Crypto Forensics API",
        version = "1.0.0",
        description = "On-chain transfer graph construction, fund tracing and risk scoring \
                       for EVM-compatible blockchains.\n\n\
                       **Versioning.** All API requests MUST include the header \
                       `X-API-Version: 1`. Use the Swagger \"Authorize\" button to set it \
                       once per session. Requests without it return HTTP 400; requests \
                       with an unsupported value return HTTP 400 as well. The Swagger UI \
                       and `/api-docs/openapi.json` are exempt.\n\n\
                       **Data source:** Moralis Deep Index API (Ethereum mainnet).\n\
                       Graph endpoints read from PostgreSQL only — call \
                       POST /jobs/ingest to populate counterparty data asynchronously."
    ),
    modifiers(&ApiSecurity),
    security(
        ("api_version" = []),
        ("api_key" = []),
    ),
    paths(
        get_graph, score_address, check_sanctions, trace_funds, list_chains,
        create_ingest_job, get_job_status,
        sanctions_batch, score_batch
    ),
    components(schemas(
        GraphPage, EdgeDto,
        ScoreResponse, SignalDto,
        SanctionsResponse,
        TraceResponse, TraceStatsDto, SinkDto, PathDto,
        ChainsResponse, ChainDto,
        ErrorResponse,
        IngestJobRequest, JobAcceptedResponse, JobStatusResponse,
        SanctionsBatchRequest, ScoreBatchRequest, BatchItem,
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
        tracing::info!(proxy = %url, "SOCKS proxy enabled");
        client_builder = client_builder.proxy(reqwest::Proxy::all(url)?);
    } else {
        tracing::info!("SOCKS proxy disabled");
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

    let api_key = cfg.server().api_key().map(str::to_owned);

    let state = Arc::new(AppState::new(
        IngestionService::new(
            sources,
            PostgresTransferRepository::new(pool.clone()),
            chain_registry.clone(),
        ),
        RiskService::new(
            PostgresTransferRepository::new(pool.clone()),
            PostgresEntityRepository::new(pool.clone()),
            cfg.risk_cache().clone().into_domain(),
        ),
        chain_registry,
        JobRepository::new(pool.clone()),
        api_key,
    ));

    let addr = format!("{}:{}", cfg.server().host(), cfg.server().port());

    let api = Router::<Arc<AppState>>::new()
        .route("/chains", get(list_chains))
        .route("/graph", get(get_graph))
        .route("/score", get(score_address))
        .route("/score/batch", post(score_batch))
        .route("/sanctions", get(check_sanctions))
        .route("/sanctions/batch", post(sanctions_batch))
        .route("/trace", get(trace_funds))
        .route("/jobs/ingest", post(create_ingest_job))
        .route("/jobs/{id}", get(get_job_status))
        .layer(middleware::from_fn_with_state(
            Arc::clone(&state),
            auth_middleware,
        ))
        .layer(middleware::from_fn(version_middleware));

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(api)
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
        .with_attribute(KeyValue::new(
            "service.name",
            cfg.service_name().to_string(),
        ))
        .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")))
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

fn format_amount(raw: U256, decimals: u8) -> String {
    let s = raw.to_string();
    if decimals == 0 {
        return s;
    }
    let dec = decimals as usize;
    let (int_part, frac_part) = if s.len() <= dec {
        (String::from("0"), format!("{:0>w$}", s, w = dec))
    } else {
        let split = s.len() - dec;
        (s[..split].to_string(), s[split..].to_string())
    };
    let frac_trunc = if frac_part.len() > 8 {
        &frac_part[..8]
    } else {
        &frac_part[..]
    };
    let frac_trim = frac_trunc.trim_end_matches('0');
    if frac_trim.is_empty() {
        int_part
    } else {
        format!("{int_part}.{frac_trim}")
    }
}

fn native_symbol(chains: &ChainRegistry, chain: ChainId) -> String {
    chains
        .get(chain)
        .map(|m| m.native_asset_symbol().to_string())
        .unwrap_or_default()
}

fn transfer_kind_str(k: &TransferKind) -> (&'static str, Option<String>) {
    match k {
        TransferKind::Native => ("native", None),
        TransferKind::Token { contract, .. } => ("token", Some(contract.canonical())),
        TransferKind::Internal => ("internal", None),
        TransferKind::Fee => ("fee", None),
        TransferKind::UtxoEdge { .. } => ("utxo_edge", None),
    }
}

fn sink_kind_str(k: &SinkKind) -> (&'static str, Option<String>) {
    match k {
        SinkKind::Exchange { name, .. } => ("exchange", Some(name.clone())),
        SinkKind::Bridge { .. } => ("bridge", None),
        SinkKind::Mixer => ("mixer", None),
        SinkKind::Sanctioned => ("sanctioned", None),
        SinkKind::Darknet => ("darknet", None),
        SinkKind::Unresolved => ("unresolved", None),
    }
}

fn signal_kind_str(k: &RiskSignalKind) -> &'static str {
    match k {
        RiskSignalKind::DirectExposure => "direct_exposure",
        RiskSignalKind::IndirectExposure { .. } => "indirect_exposure",
        RiskSignalKind::SanctionedCounterparty => "sanctioned_counterparty",
        RiskSignalKind::MixerInteraction => "mixer_interaction",
        RiskSignalKind::DarknetMarket => "darknet_market",
        RiskSignalKind::RapidLayering => "rapid_layering",
        RiskSignalKind::HighVelocity => "high_velocity",
        RiskSignalKind::NewAddress => "new_address",
        RiskSignalKind::NoKyc => "no_kyc",
    }
}

fn sanction_list_str(s: &SanctionList) -> String {
    match s {
        SanctionList::Ofac => "ofac".into(),
        SanctionList::Eu => "eu".into(),
        SanctionList::Un => "un".into(),
        SanctionList::Other(s) => s.to_lowercase().replace([' ', '-'], "_"),
    }
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
    /// Sender address
    from: String,
    /// Recipient address
    to: String,
    /// Raw amount (integer big-int, no decimals applied)
    #[schema(example = "1000000000000000000")]
    raw: String,
    /// Decimal-shifted human amount (raw / 10^decimals, up to 8 fractional digits)
    #[schema(example = "1.0")]
    formatted: String,
    /// Native asset symbol for the chain
    #[schema(example = "ETH")]
    symbol: String,
    /// Token decimals (18 for ETH and most ERC-20 tokens)
    decimals: u8,
    /// Block height
    block: u64,
    /// Unix timestamp (seconds)
    ts: i64,
    /// Stable transfer kind: `native`, `token`, `internal`, `fee`, `utxo_edge`
    kind: String,
    /// Token contract address when `kind == "token"`, else null
    contract: Option<String>,
    chain_id: u32,
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
    /// All unique addresses in the graph. Returned only on the first page (`page == 0`);
    /// subsequent pages return an empty array.
    nodes: Vec<String>,
    /// Transfers on the current page
    edges: Vec<EdgeDto>,
}

/// A risk signal attached to an address score.
#[derive(Serialize, utoipa::ToSchema)]
pub struct SignalDto {
    /// Stable snake_case signal category (e.g. `sanctioned_counterparty`, `mixer_interaction`)
    kind: String,
    /// Severity 0–100 (≥70 = HIGH, ≥90 = CRITICAL)
    severity: u8,
    /// Human-readable explanation
    description: String,
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

/// OFAC / sanctions screening result.
#[derive(Serialize, utoipa::ToSchema)]
pub struct SanctionsResponse {
    address: String,
    chain_id: u32,
    /// Whether the address appears on a sanctions list
    is_sanctioned: bool,
    /// Sanctions list name, snake_case if applicable
    sanction_list: Option<String>,
    /// Known entity label, if applicable
    label: Option<String>,
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

/// A terminal sink discovered during tracing.
#[derive(Serialize, utoipa::ToSchema)]
pub struct SinkDto {
    address: String,
    /// Stable sink category: `exchange`, `bridge`, `mixer`, `sanctioned`, `darknet`, `unresolved`
    kind: String,
    /// Exchange name when `kind == "exchange"`, else null
    name: Option<String>,
    /// Risk score of this sink 0–100
    risk_score: u8,
    /// Raw tainted amount that reached this sink (big-int)
    tainted_amount: String,
    /// Decimal-shifted tainted amount (up to 8 fractional digits)
    formatted: String,
    /// Fraction of the sink's incoming funds that are tainted (0.0–1.0)
    taint_ratio: f64,
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

/// Fund trace result.
#[derive(Serialize, utoipa::ToSchema)]
pub struct TraceResponse {
    stats: TraceStatsDto,
    sinks: Vec<SinkDto>,
    paths: Vec<PathDto>,
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

#[derive(Serialize, utoipa::ToSchema)]
pub struct ChainsResponse {
    chains: Vec<ChainDto>,
}

/// Error response body.
#[derive(Serialize, utoipa::ToSchema)]
pub struct ErrorResponse {
    error: String,
}

// ── API error type ───────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ErrBody {
    error: String,
}

pub enum ApiError {
    BadRequest(String),
    Unauthorized,
    Internal(DomainError),
    InternalMsg(String),
}

impl ApiError {
    fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, Json(ErrBody { error: msg })).into_response()
            }
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                Json(ErrBody {
                    error: "unauthorized".into(),
                }),
            )
                .into_response(),
            Self::Internal(e) => {
                tracing::error!(error = %e, "domain error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrBody {
                        error: "internal server error".into(),
                    }),
                )
                    .into_response()
            }
            Self::InternalMsg(msg) => {
                tracing::error!(error = %msg, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrBody {
                        error: "internal server error".into(),
                    }),
                )
                    .into_response()
            }
        }
    }
}

// ── Versioning middleware ────────────────────────────────────────────────────

const API_VERSION_HEADER: &str = "X-API-Version";
const SUPPORTED_API_VERSION: &str = "1";

async fn version_middleware(
    req: axum::extract::Request,
    next: Next,
) -> Result<axum::response::Response, ApiError> {
    match req
        .headers()
        .get(API_VERSION_HEADER)
        .and_then(|v| v.to_str().ok())
    {
        Some(v) if v == SUPPORTED_API_VERSION => Ok(next.run(req).await),
        Some(other) => Err(ApiError::bad_request(format!(
            "unsupported {API_VERSION_HEADER}: {other}; supported: {SUPPORTED_API_VERSION}"
        ))),
        None => Err(ApiError::bad_request(format!(
            "missing required header: {API_VERSION_HEADER}"
        ))),
    }
}

// ── Auth middleware ──────────────────────────────────────────────────────────

async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: axum::extract::Request,
    next: Next,
) -> Result<axum::response::Response, ApiError> {
    let Some(expected) = state.api_key() else {
        return Ok(next.run(req).await);
    };

    let headers = req.headers();
    let provided = headers
        .get("X-Api-Key")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .or_else(|| {
            headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.strip_prefix("Bearer "))
                .map(str::to_string)
        });

    match provided {
        Some(p) if p == expected => Ok(next.run(req).await),
        _ => Err(ApiError::Unauthorized),
    }
}

// ── Handlers ─────────────────────────────────────────────────────────────────

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
        state
            .ingestion()
            .sources()
            .supported_chains()
            .into_iter()
            .collect()
    };
    let chains = state
        .chains()
        .all()
        .iter()
        .map(|m| ChainDto {
            id: m.id().value(),
            name: m.name().to_string(),
            family: match m.family() {
                ChainFamily::Evm => "evm",
                ChainFamily::Tron => "tron",
                ChainFamily::Bitcoin => "bitcoin",
                ChainFamily::Solana => "solana",
                ChainFamily::Other => "other",
            }
            .into(),
            address_model: match m.address_model() {
                AddressModel::Account => "account",
                AddressModel::Utxo => "utxo",
            }
            .into(),
            address_encoding: match m.address_encoding() {
                AddressEncoding::Hex20 => "hex20",
                AddressEncoding::TronBase58Check => "tron_base58_check",
                AddressEncoding::Bech32 => "bech32",
                AddressEncoding::Base58 => "base58",
            }
            .into(),
            native_symbol: m.native_asset_symbol().to_string(),
            native_decimals: m.native_asset_decimals(),
            confirmation_depth: m.confirmation_depth(),
            source_registered: registered.contains(&m.id()),
        })
        .collect();
    Json(ChainsResponse { chains })
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

    /// Page index (0-based). Defaults to 0. Nodes are returned only on `page == 0`.
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
        (status = 200, description = "Paginated transfer graph (read-only from DB; \
                                       trigger ingestion with POST /jobs/ingest). \
                                       `nodes` is populated only when `page == 0`.", body = GraphPage),
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
        .build_graph_from_db(
            &addr,
            GraphRequest::new(range, q.max_depth.unwrap_or(3), q.max_nodes.unwrap_or(500)),
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

    let nodes: Vec<String> = if page == 0 {
        graph.nodes().iter().map(|a| a.canonical()).collect()
    } else {
        Vec::new()
    };

    let symbol = native_symbol(state.chains(), chain);

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
                symbol: symbol.clone(),
                decimals: t.amount().decimals(),
                block: t.block().height(),
                ts: t.timestamp().timestamp(),
                kind: kind.into(),
                contract,
                chain_id: t.chain().value(),
            }
        })
        .collect();

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

    Ok(Json(report_to_dto(&report)))
}

fn report_to_dto(report: &domain::risk::RiskReport) -> ScoreResponse {
    let signals: Vec<SignalDto> = report
        .signals()
        .iter()
        .map(|s| SignalDto {
            kind: signal_kind_str(s.kind()).into(),
            severity: s.severity().value(),
            description: s.description().to_string(),
        })
        .collect();

    ScoreResponse {
        address: report.subject().canonical(),
        chain_id: report.subject().chain().value(),
        score: report.overall_score().value(),
        is_high_risk: report.is_high_risk(),
        signals,
        generated_at: report.generated_at().to_rfc3339(),
    }
}

fn sanctions_to_dto(result: &domain::risk::SanctionsCheckResult) -> SanctionsResponse {
    SanctionsResponse {
        address: result.address().canonical(),
        chain_id: result.address().chain().value(),
        is_sanctioned: result.is_sanctioned(),
        sanction_list: result.sanction_list().map(sanction_list_str),
        label: result.label().map(str::to_string),
    }
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

    Ok(Json(sanctions_to_dto(&result)))
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
        .map(|s| {
            let (kind, name) = sink_kind_str(s.kind());
            SinkDto {
                address: s.address().canonical(),
                kind: kind.into(),
                name,
                risk_score: s.risk_score(),
                tainted_amount: s.tainted_amount().raw().to_string(),
                formatted: format_amount(s.tainted_amount().raw(), s.tainted_amount().decimals()),
                taint_ratio: s.taint_ratio().as_f64(),
            }
        })
        .collect();

    let paths: Vec<PathDto> = result
        .paths()
        .iter()
        .map(|p| PathDto {
            depth: p.depth(),
            tainted_amount: p.tainted_amount().raw().to_string(),
            taint_ratio: p.taint_ratio().as_f64(),
            hops: p.hops().len(),
            origin: p.origin().map(|a| a.canonical()),
            destination: p.destination().map(|a| a.canonical()),
        })
        .collect();

    Ok(Json(TraceResponse {
        stats: TraceStatsDto {
            addresses_visited: result.stats().addresses_visited(),
            transfers_evaluated: result.stats().transfers_evaluated(),
            paths_found: result.stats().paths_found(),
            depth_reached: result.stats().depth_reached(),
            truncated: result.stats().truncated(),
        },
        sinks,
        paths,
    }))
}

// ── Job endpoints ────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, utoipa::ToSchema)]
pub struct IngestJobRequest {
    #[schema(example = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")]
    address: String,
    #[schema(example = 1)]
    chain_id: Option<u32>,
    #[schema(example = 3)]
    max_depth: Option<u32>,
    #[schema(example = 500)]
    max_nodes: Option<usize>,
    from_block: Option<u64>,
    to_block: Option<u64>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct JobAcceptedResponse {
    job_id: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct JobStatusResponse {
    id: String,
    status: String,
    error: Option<String>,
    created_at: String,
    updated_at: String,
}

#[utoipa::path(
    post, path = "/jobs/ingest",
    request_body = IngestJobRequest,
    responses(
        (status = 202, description = "Ingestion job accepted", body = JobAcceptedResponse),
        (status = 400, description = "Invalid address or parameters", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
    tag = "Jobs"
)]
pub async fn create_ingest_job(
    State(state): State<Arc<AppState>>,
    Json(body): Json<IngestJobRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let chain = ChainId::new(body.chain_id.unwrap_or(ChainId::ETH.value()));
    let addr = parse_address(&body.address, chain)?;

    let max_depth = body.max_depth.unwrap_or(3);
    let max_nodes = body.max_nodes.unwrap_or(500);
    let range = match (body.from_block, body.to_block) {
        (None, None) => None,
        (from, to) => Some(BlockRange::new(from.unwrap_or(0), to.unwrap_or(u64::MAX))),
    };

    let payload = serde_json::to_value(&body).map_err(|e| ApiError::InternalMsg(e.to_string()))?;

    let job_id = state
        .jobs()
        .create_job("ingest", payload)
        .await
        .map_err(ApiError::Internal)?;

    let state_for_job = Arc::clone(&state);
    let addr_for_job = addr.clone();
    tokio::spawn(async move {
        if let Err(e) = state_for_job.jobs().set_running(job_id).await {
            tracing::error!(error = %e, job_id = %job_id, "failed to mark job running");
            return;
        }
        let req = GraphRequest::new(range, max_depth, max_nodes);
        match state_for_job
            .ingestion()
            .build_graph(&addr_for_job, req)
            .await
        {
            Ok(_) => {
                if let Err(e) = state_for_job.jobs().set_done(job_id).await {
                    tracing::error!(error = %e, job_id = %job_id, "failed to mark job done");
                }
            }
            Err(e) => {
                let msg = e.to_string();
                tracing::warn!(error = %msg, job_id = %job_id, "ingest job failed");
                if let Err(e2) = state_for_job.jobs().set_failed(job_id, &msg).await {
                    tracing::error!(error = %e2, job_id = %job_id, "failed to mark job failed");
                }
            }
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(JobAcceptedResponse {
            job_id: job_id.to_string(),
        }),
    ))
}

#[utoipa::path(
    get, path = "/jobs/{id}",
    params(("id" = String, Path, description = "Job UUID")),
    responses(
        (status = 200, description = "Current job status", body = JobStatusResponse),
        (status = 404, description = "Job not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
    tag = "Jobs"
)]
pub async fn get_job_status(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let uuid = uuid::Uuid::parse_str(&id)
        .map_err(|e| ApiError::bad_request(format!("invalid job id: {e}")))?;

    let job = state
        .jobs()
        .get_job(uuid)
        .await
        .map_err(ApiError::Internal)?;

    let Some(job) = job else {
        return Err(ApiError::BadRequest("job not found".into()));
    };

    Ok(Json(JobStatusResponse {
        id: job.id.to_string(),
        status: job.status,
        error: job.error,
        created_at: job.created_at.to_rfc3339(),
        updated_at: job.updated_at.to_rfc3339(),
    }))
}

// ── Batch endpoints ──────────────────────────────────────────────────────────

#[derive(Deserialize, utoipa::ToSchema)]
pub struct BatchItem {
    #[schema(example = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045")]
    address: String,
    #[schema(example = 1)]
    chain_id: Option<u32>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct SanctionsBatchRequest {
    items: Vec<BatchItem>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ScoreBatchRequest {
    items: Vec<BatchItem>,
}

const MAX_SANCTIONS_BATCH: usize = 100;
const MAX_SCORE_BATCH: usize = 50;

#[utoipa::path(
    post, path = "/sanctions/batch",
    request_body = SanctionsBatchRequest,
    responses(
        (status = 200, description = "Sanctions check per address", body = [SanctionsResponse]),
        (status = 400, description = "Invalid input or batch too large", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
    tag = "Risk"
)]
pub async fn sanctions_batch(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SanctionsBatchRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if body.items.len() > MAX_SANCTIONS_BATCH {
        return Err(ApiError::bad_request(format!(
            "batch too large: {} > {MAX_SANCTIONS_BATCH}",
            body.items.len()
        )));
    }

    let mut addrs: Vec<Address> = Vec::with_capacity(body.items.len());
    for it in &body.items {
        let chain = ChainId::new(it.chain_id.unwrap_or(ChainId::ETH.value()));
        addrs.push(parse_address(&it.address, chain)?);
    }

    let results = state
        .risk()
        .check_sanctions_batch(&addrs)
        .await
        .map_err(ApiError::Internal)?;

    let dtos: Vec<SanctionsResponse> = results.iter().map(sanctions_to_dto).collect();
    Ok(Json(dtos))
}

#[utoipa::path(
    post, path = "/score/batch",
    request_body = ScoreBatchRequest,
    responses(
        (status = 200, description = "Risk report per address", body = [ScoreResponse]),
        (status = 400, description = "Invalid input or batch too large", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse),
    ),
    tag = "Risk"
)]
pub async fn score_batch(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ScoreBatchRequest>,
) -> Result<impl IntoResponse, ApiError> {
    if body.items.len() > MAX_SCORE_BATCH {
        return Err(ApiError::bad_request(format!(
            "batch too large: {} > {MAX_SCORE_BATCH}",
            body.items.len()
        )));
    }

    let mut addrs: Vec<Address> = Vec::with_capacity(body.items.len());
    for it in &body.items {
        let chain = ChainId::new(it.chain_id.unwrap_or(ChainId::ETH.value()));
        addrs.push(parse_address(&it.address, chain)?);
    }

    let reports = state
        .risk()
        .score_batch(&addrs)
        .await
        .map_err(ApiError::Internal)?;

    let dtos: Vec<ScoreResponse> = reports.iter().map(report_to_dto).collect();
    Ok(Json(dtos))
}
