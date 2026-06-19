Refactor the Axum API in the Rust project at /home/eblan/work/github/PayTraces/code.
Do NOT touch domain/ at all. Changes are limited to api/, usecase/, infra/.

────────────────────────────────────────
1. STABLE ENUM STRINGS  (api/src/main.rs)
────────────────────────────────────────
Replace every `format!("{:?}", x)` with explicit match that returns
lowercase snake_case strings stable as API contract:

- Transfer kind (EdgeDto):
    Native          → "native"
    Token { .. }    → "token"
    Internal        → "internal"
    Fee             → "fee"
    UtxoEdge        → "utxo_edge"
  Add separate field `contract: Option<String>` to EdgeDto for the token
  contract address when kind == "token".

- SinkKind (SinkDto):
    Exchange { name, .. }   → kind="exchange", add field `name: Option<String>`
    Bridge { .. }           → "bridge"
    Mixer                   → "mixer"
    Sanctioned              → "sanctioned"
    Darknet                 → "darknet"
    Unresolved              → "unresolved"

- SignalKind (SignalDto): same approach, snake_case string.

- SanctionList (SanctionsResponse.sanction_list): snake_case string.

────────────────────────────────────────
2. FORMATTED AMOUNTS  (api/src/main.rs)
────────────────────────────────────────
In EdgeDto replace `amount: String` with:
    raw: String          -- unchanged big integer string
    formatted: String    -- raw / 10^decimals, up to 8 significant decimal places
    symbol: String       -- native asset symbol from ChainRegistry (pass chain meta into handler)

Same for SinkDto: add `formatted: String` next to `tainted_amount`.

────────────────────────────────────────
3. NODES ONLY ON FIRST PAGE  (api/src/main.rs)
────────────────────────────────────────
In get_graph handler: return `nodes: Vec<String>` only when page == 0,
otherwise return `nodes: []`. Document this in the utoipa schema comment.

────────────────────────────────────────
4. API VERSIONING  (api/src/main.rs)
────────────────────────────────────────
Prefix all routes with /v1:
  /v1/chains, /v1/graph, /v1/score, /v1/sanctions, /v1/trace
  /v1/jobs  (new, see §6)
  /v1/sanctions/batch  (new, see §7)
  /v1/score/batch      (new, see §8)
Swagger stays at /swagger-ui and /api-docs/openapi.json (no version prefix).

────────────────────────────────────────
5. API KEY AUTH  (api/src/main.rs + api/src/config.rs)
────────────────────────────────────────
Add `api_key: Option<String>` to ServerConfig (serde field "api_key").
Add to config.yaml: `api_key: ""` (empty = auth disabled).

Add an Axum middleware function `auth_middleware` that:
- If configured key is empty/None → passes through (auth disabled).
- Otherwise checks header `Authorization: Bearer <key>` OR `X-Api-Key: <key>`.
- Returns 401 JSON `{"error":"unauthorized"}` on mismatch.

Apply the middleware only to /v1/* routes (not swagger).

────────────────────────────────────────
6. ASYNC INGESTION JOBS  (infra/ + api/src/main.rs)
────────────────────────────────────────
Postgres migration infra/src/migrations/V4__jobs.sql:
  CREATE TABLE jobs (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind        TEXT NOT NULL,          -- 'ingest'
    status      TEXT NOT NULL,          -- 'pending','running','done','failed'
    payload     JSONB NOT NULL,
    error       TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
  );

Add JobRepository to infra (struct + impl):
  - create_job(kind, payload) -> Uuid
  - set_running(id)
  - set_done(id)
  - set_failed(id, error)
  - get_job(id) -> JobRow

New API endpoints:

POST /v1/jobs/ingest
  Body: { "address": "0x...", "chain_id": 1, "max_depth": 3,
          "max_nodes": 500, "from_block": null, "to_block": null }
  Response 202: { "job_id": "<uuid>" }
  Action: insert job row, spawn tokio::task that calls
          ingestion.build_graph(...), updates job status to done/failed.

GET /v1/jobs/{id}
  Response: { "id": "...", "status": "running"|"done"|"failed",
              "error": null, "created_at": "...", "updated_at": "..." }

GET /v1/graph now only reads from Postgres (no ingestion trigger).
Keep the old synchronous behaviour as fallback only if job_id query
param is absent AND address data already exists in DB.

Add JobRepository to AppState (new field + getter).

────────────────────────────────────────
7. BATCH SANCTIONS  (api/src/main.rs)
────────────────────────────────────────
POST /v1/sanctions/batch
  Body:    { "items": [ { "address": "0x...", "chain_id": 1 }, ... ] }
  Response: [ { SanctionsResponse }, ... ]

RiskPort::check_sanctions_batch already exists in domain — call it directly
from the handler. Max 100 items; return 400 if exceeded.

────────────────────────────────────────
8. BATCH SCORE  (usecase/src/risk.rs + api/src/main.rs)
────────────────────────────────────────
Add to RiskService (usecase/src/risk.rs):
  pub async fn score_batch(&self, addresses: &[Address])
      -> Result<Vec<RiskReport>, DomainError>
  Implementation: FuturesUnordered over self.score(addr) for each address.

POST /v1/score/batch
  Body:     { "items": [ { "address": "0x...", "chain_id": 1 }, ... ] }
  Response: [ { ScoreResponse }, ... ]
  Max 50 items; return 400 if exceeded.

────────────────────────────────────────
FINAL CHECK
────────────────────────────────────────
Run `cargo check --workspace 2>&1` and fix all errors.
Run `cargo clippy --workspace 2>&1 | grep "^error"` and fix errors (not warnings).
domain/ must have zero modifications — verify with `git diff domain/`.
