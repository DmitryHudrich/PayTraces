# Ledgerscope

> Blockchain forensics and AML platform for tracing illicit fund flows across multiple chains.

*(Formerly known as PayTraces.)*

## Overview

Ledgerscope is a transaction graph analysis engine built for detecting and tracing illicit fund flows on-chain. It ingests raw blockchain data, constructs enriched address graphs, applies forensic heuristics, and surfaces actionable risk signals — with the goal of being competitive in capability with commercial tools like Chainalysis or TRM Labs, built independently.

Core capabilities:

- Multi-chain transaction graph construction
- Taint propagation and risk scoring
- Entity clustering (Union-Find)
- Heuristic-based pattern detection (smurfing, fan-in/fan-out, temporal bursts, fixed-amount clustering)

## Chains in Scope

| Chain | Status | Notes |
|---|---|---|
| Ethereum / EVM | Active | Alchemy/Infura, with self-hosted Erigon/Reth planned for production independence |
| Tron (TRC-20) | Active | TronGrid for raw data, Tronscan for entity labeling |
| Bitcoin | Planned | UTXO model requires a separate paradigm from account-based chains |
| Other EVM (BSC, Polygon, Avalanche) | Considered | Heuristics transfer with calibration |
| Solana | Out of scope (for now) | Instruction-based architecture requires separate handling |

## Architecture

Rust workspace using hexagonal architecture (ports & adapters):

```
domain/    — core business types and logic, no infrastructure concerns
usecase/   — application logic orchestrating domain and ports
infra/     — adapters: chain sources, database, external APIs
api/       — HTTP/gRPC surface
```

- **Source of truth:** PostgreSQL
- **Indexing:** demand-driven / lazy, not full-chain sync
- **Service split (in progress):** Rust engine retains tracking/heuristics; accounts/users/statistics are being extracted into a separate C# service, connected via gRPC (tonic ↔ Grpc.Net.Client)
- **Observability:** OpenTelemetry/OTLP across both runtimes, Jaeger for tracing

## Current State

### Risk & Heuristics Engine (`RiskService`)
- Taint propagation: Haircut strategy implemented; FIFO/LIFO pending
- Risk scoring, sanctions checking, entity clustering (Union-Find)
- Known open bugs: bidirectional traversal handling, sink deduplication after sort, early-termination logic in bounded traversal

### Label System
Moving from single-`category`-per-entity to a multi-tag model:
- `LabelTag` domain model (`source`, `confidence`, `active`, `superseded_by`, `expires_at`)
- Append-only `tag_history` event log for audit trail
- Multiple sources (automated, manual) coexist rather than overwrite
- Phased migration with backward compatibility via `legacy_import` source tag

### Graph API
- Enriched `NodeDto` (kind, service name, risk score, primary tag, degree, tx count) via SQL joins, avoiding N+1 round-trips
- `?enrich=full|minimal` query parameter
- Batch node lookup endpoint for address-list responses
- GraphQL planned for a later phase (`async-graphql` + Axum), with mandatory query depth/complexity limiting

### Tron Integration (`TronGridSource`)
Known open bugs: transaction finality misclassification on failed/reverted transactions, data loss on multi-contract transactions, hardcoded transfer index causing collision risk.

### BigQuery Integration
Working Rust client (`yup-oauth2` + `reqwest`) against `bigquery-public-data.crypto_ethereum` / `crypto_bitcoin`. Partition filtering on `block_timestamp` is mandatory to avoid excessive byte scanning.

## Roadmap

1. MVP correctness: graph traversal bug fixes, USD price normalization, core heuristics, watchlist alerting
2. Sanctions screening (OFAC SDN)
3. GraphQL API phase
4. Tronscan adapter for entity clustering signals
5. C# service extraction (accounts/users/statistics)
6. Bitcoin support (UTXO-specific paradigm)
7. Solana support

## Tech Stack

- **Language/runtime:** Rust (tokio, axum, petgraph, sqlx, refinery)
- **Database:** PostgreSQL
- **Observability:** tracing, opentelemetry-otlp, Jaeger
- **Chain data:** TronGrid, Alchemy/Infura, Google BigQuery public datasets, Moralis
- **Labeling:** dawsbot/eth-labels, brianleect/etherscan-labels
- **Validation:** Etherscan, Tronscan, Elliptic dataset (Kaggle)

## Testing

Heuristics are validated using known reference addresses (Tornado Cash router for mixer detection, exchange hot wallets as negative controls).
