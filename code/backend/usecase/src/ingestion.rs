use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use async_trait::async_trait;
use chrono::Utc;
use domain::chain::{ChainId, ChainRegistry};
use domain::error::{DomainError, DomainResult};
use domain::graph::{GraphRequest, TransferGraph};
use domain::entity::AddressKind;
use domain::ports::{
    AddressKindRepository, Alert, AlertSink, BlockRange, ChainSource, ChainSourceRegistry,
    EntityRepository, IngestionPort, PricePort, TagHistoryRepository, TagProvider,
    TransferCursor, TransferRepository, WatchlistRepository,
};
use domain::primitives::Address;
use domain::transfer::Transfer;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::labels::{TagApplyInput, apply_tag};

/// Chain-scoped external tag sources + the repos needed to apply what they
/// find, wired in via `IngestionService::with_auto_tagging`. Best-effort:
/// every failure is logged and swallowed so a flaky third-party API never
/// fails ingestion itself.
pub struct AutoTagging {
    pub providers: HashMap<ChainId, Arc<dyn TagProvider>>,
    pub entities: Arc<dyn EntityRepository>,
    pub history: Arc<dyn TagHistoryRepository>,
}

/// Adaptive limiter for concurrent `transfers_for_address` calls.
///
/// Concurrent /build_graph requests share a single `IngestionService`; when
/// they collectively saturate the chain source(s) we observe rate-limit
/// errors. The gate detects this and dynamically shrinks the in-flight cap
/// so the next batch of work doesn't keep hammering the upstream.
///
/// Mechanism:
/// * On `RateLimited` → the permit being held is `.forget()`'d, which
///   *permanently* reduces the semaphore's effective capacity by one. The
///   logical `permits_current` counter tracks this so logs are accurate.
/// * On `success_streak >= grow_after_successes` consecutive Ok's →
///   `add_permits(1)` (up to `permits_max`) and reset the streak.
/// * Shrinks bottom out at `permits_min` (≥ 1), grows top out at `permits_max`.
///
/// Both directions emit a WARN with `from`/`to` permit counts so the
/// pressure profile is visible in logs.
#[derive(Debug)]
pub struct AdaptiveConcurrency {
    semaphore: Arc<Semaphore>,
    permits_current: AtomicU32,
    permits_min: u32,
    permits_max: u32,
    success_streak: AtomicU32,
    grow_after_successes: u32,
}

impl AdaptiveConcurrency {
    pub fn new(initial: u32, min: u32, max: u32, grow_after_successes: u32) -> Self {
        let min = min.max(1);
        let max = max.max(min);
        let initial = initial.clamp(min, max);
        Self {
            semaphore: Arc::new(Semaphore::new(initial as usize)),
            permits_current: AtomicU32::new(initial),
            permits_min: min,
            permits_max: max,
            success_streak: AtomicU32::new(0),
            grow_after_successes: grow_after_successes.max(1),
        }
    }

    /// Sensible default for the transfers path: start at 4 concurrent
    /// fetches, never go below 1, never above 8, restore one slot every 20
    /// consecutive successful fetches.
    pub fn transfers_default() -> Self {
        Self::new(4, 1, 8, 20)
    }

    pub fn current_permits(&self) -> u32 {
        self.permits_current.load(Ordering::Relaxed)
    }

    pub async fn acquire(&self) -> Option<OwnedSemaphorePermit> {
        self.semaphore.clone().acquire_owned().await.ok()
    }

    /// The held permit is `forget()`'d (effective capacity −1) when we're
    /// above the floor, otherwise dropped normally. Resets the success
    /// streak either way.
    pub fn shrink_after_rate_limit(&self, permit: OwnedSemaphorePermit) {
        self.success_streak.store(0, Ordering::Relaxed);
        let cur = self.permits_current.load(Ordering::Relaxed);
        if cur > self.permits_min {
            permit.forget();
            let new = cur - 1;
            self.permits_current.store(new, Ordering::Relaxed);
            tracing::warn!(
                from = cur,
                to = new,
                min = self.permits_min,
                "adaptive concurrency: transfers rate-limited, reduced concurrency"
            );
        } else {
            drop(permit);
            tracing::warn!(
                permits = cur,
                min = self.permits_min,
                "adaptive concurrency: transfers rate-limited, concurrency already at floor"
            );
        }
    }

    /// Successful call — bumps the streak, and after `grow_after_successes`
    /// of them adds a permit back (up to ceiling). Permit always dropped.
    pub fn release_after_success(&self, permit: OwnedSemaphorePermit) {
        drop(permit);
        let streak = self.success_streak.fetch_add(1, Ordering::Relaxed) + 1;
        if streak < self.grow_after_successes {
            return;
        }
        let cur = self.permits_current.load(Ordering::Relaxed);
        if cur < self.permits_max {
            self.semaphore.add_permits(1);
            let new = cur + 1;
            self.permits_current.store(new, Ordering::Relaxed);
            tracing::warn!(
                from = cur,
                to = new,
                max = self.permits_max,
                successes_in_a_row = streak,
                "adaptive concurrency: transfers stable, restored concurrency"
            );
        }
        // Reset streak regardless so the next grow waits for a fresh window.
        self.success_streak.store(0, Ordering::Relaxed);
    }

    /// Non-rate-limit errors do not change concurrency state (real bugs
    /// shouldn't shrink our throughput) — just return the permit.
    pub fn release_after_other_error(&self, permit: OwnedSemaphorePermit) {
        drop(permit);
    }
}

pub struct IngestionService<S, R> {
    sources: S,
    repo: R,
    chains: ChainRegistry,
    prices: Option<Arc<dyn PricePort>>,
    watchlist: Option<Arc<dyn WatchlistRepository>>,
    alerts: Option<Arc<dyn AlertSink>>,
    address_kinds: Option<Arc<dyn AddressKindRepository>>,
    auto_tagging: Option<AutoTagging>,
    transfers_gate: Arc<AdaptiveConcurrency>,
    classify_chain_batch_size: usize,
}

impl<S, R> IngestionService<S, R> {
    pub fn new(sources: S, repo: R, chains: ChainRegistry) -> Self {
        Self {
            sources,
            repo,
            chains,
            prices: None,
            watchlist: None,
            alerts: None,
            address_kinds: None,
            auto_tagging: None,
            transfers_gate: Arc::new(AdaptiveConcurrency::transfers_default()),
            // 100 fits inside Alchemy's batch limit and typical Postgres
            // ARRAY parameter sizes comfortably. Tunable via config.
            classify_chain_batch_size: 100,
        }
    }

    pub fn with_transfers_concurrency(mut self, gate: Arc<AdaptiveConcurrency>) -> Self {
        self.transfers_gate = gate;
        self
    }

    pub fn with_classify_chain_batch_size(mut self, n: usize) -> Self {
        self.classify_chain_batch_size = n.max(1);
        self
    }

    pub fn with_address_kinds(mut self, kinds: Arc<dyn AddressKindRepository>) -> Self {
        self.address_kinds = Some(kinds);
        self
    }

    pub fn with_auto_tagging(mut self, auto_tagging: AutoTagging) -> Self {
        self.auto_tagging = Some(auto_tagging);
        self
    }

    pub fn with_prices(mut self, prices: Arc<dyn PricePort>) -> Self {
        self.prices = Some(prices);
        self
    }

    pub fn with_watchlist(
        mut self,
        watchlist: Arc<dyn WatchlistRepository>,
        alerts: Arc<dyn AlertSink>,
    ) -> Self {
        self.watchlist = Some(watchlist);
        self.alerts = Some(alerts);
        self
    }

    pub fn sources(&self) -> &S {
        &self.sources
    }

    pub fn repo(&self) -> &R {
        &self.repo
    }

    pub fn chains(&self) -> &ChainRegistry {
        &self.chains
    }
}

/// Probe the chain for each new address in the freshly-ingested batch and
/// persist its AddressKind (EOA vs Contract). Skips addresses whose kind is
/// already a non-Unknown label (preserves manual `KnownService` overrides).
///
/// Implementation is fully batched:
///  1. Collect unique addresses from `fetched`.
///  2. ONE Postgres roundtrip via `kind_batch` to fetch existing labels.
///  3. Partition into Unknown (need probe) vs already-labelled (skip).
///  4. Chunk Unknowns into batches of `chain_batch_size` and call
///     `source.is_contract_batch` on each. With the routed source +
///     Alchemy this becomes one JSON-RPC batch HTTP per chunk; with
///     Etherscan it's a parallel `join_all` over single calls. Either
///     way the round-trip cost drops by ~`chain_batch_size`×.
///  5. ONE Postgres roundtrip via `set_kind_batch` to persist results.
///  6. Log totals + elapsed_ms so the WARN log can answer "why is
///     classify taking so long".
async fn classify_address_kinds(
    source: &dyn ChainSource,
    kinds: Option<&Arc<dyn AddressKindRepository>>,
    fetched: &[Transfer],
    chain_batch_size: usize,
) {
    let Some(kinds) = kinds else { return };
    let start = std::time::Instant::now();

    let mut seen: HashSet<Address> = HashSet::new();
    for t in fetched {
        seen.insert(t.from().clone());
        seen.insert(t.to().clone());
    }
    let all: Vec<Address> = seen.into_iter().collect();
    let total = all.len();
    if total == 0 {
        return;
    }

    // Step 1: bulk fetch known kinds.
    let known = match kinds.kind_batch(&all).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, total, "address_kind: kind_batch failed");
            return;
        }
    };

    // Step 2: filter to Unknowns — those need a chain probe.
    let mut to_probe: Vec<Address> = Vec::new();
    let mut already_labelled = 0usize;
    for (addr, k) in all.iter().zip(known.iter()) {
        match k {
            AddressKind::Unknown => to_probe.push(addr.clone()),
            _ => already_labelled += 1,
        }
    }
    let probe_count = to_probe.len();

    // Step 3: chunked batched probe.
    let chunk_size = chain_batch_size.max(1);
    let mut contracts: Vec<(Address, AddressKind)> = Vec::new();
    let mut eoas: Vec<(Address, AddressKind)> = Vec::new();
    let mut unknown_after_probe = 0usize;

    for chunk in to_probe.chunks(chunk_size) {
        match source.is_contract_batch(chunk).await {
            Ok(results) => {
                for (addr, r) in chunk.iter().zip(results.into_iter()) {
                    match r {
                        Some(true) => contracts.push((addr.clone(), AddressKind::Contract)),
                        Some(false) => eoas.push((addr.clone(), AddressKind::Eoa)),
                        None => unknown_after_probe += 1,
                    }
                }
            }
            Err(e) => {
                tracing::debug!(
                    error = %e,
                    batch = chunk.len(),
                    "is_contract_batch failed for chunk; leaving as Unknown"
                );
                unknown_after_probe += chunk.len();
            }
        }
    }

    // Step 4: bulk persist (one INSERT for contracts + eoas combined).
    let mut to_persist: Vec<(Address, AddressKind)> =
        Vec::with_capacity(contracts.len() + eoas.len());
    to_persist.append(&mut contracts);
    to_persist.append(&mut eoas);
    if !to_persist.is_empty() {
        if let Err(e) = kinds.set_kind_batch(&to_persist).await {
            tracing::warn!(error = %e, rows = to_persist.len(), "set_kind_batch failed");
        }
    }

    let elapsed_ms = start.elapsed().as_millis() as u64;
    tracing::info!(
        total,
        already_labelled,
        probed = probe_count,
        contracts = to_persist.iter().filter(|(_, k)| matches!(k, AddressKind::Contract)).count(),
        eoas = to_persist.iter().filter(|(_, k)| matches!(k, AddressKind::Eoa)).count(),
        unknown_after_probe,
        chain_batch_size = chunk_size,
        elapsed_ms,
        "address kinds classified"
    );
}

/// Best-effort auto-enrichment: for each address touched by this batch that
/// has a `TagProvider` registered for its chain and no active tag yet,
/// resolve external candidates and apply them through the same
/// `apply_tag` resolution rules `POST /labels` uses (source is
/// `ThirdParty(...)`, so it never fights a manually-applied tag — per the
/// resolution rules a different source on the same category just
/// coexists). Every failure is logged at DEBUG and skipped; this must
/// never fail ingestion itself.
async fn auto_tag_addresses(auto_tagging: Option<&AutoTagging>, fetched: &[Transfer]) {
    let Some(cfg) = auto_tagging else { return };

    let mut seen: HashSet<Address> = HashSet::new();
    for t in fetched {
        seen.insert(t.from().clone());
        seen.insert(t.to().clone());
    }

    for addr in seen {
        let Some(provider) = cfg.providers.get(&addr.chain()) else {
            continue;
        };

        match cfg.entities.find_by_address(&addr).await {
            Ok(Some(entity)) if entity.active_tags().next().is_some() => continue,
            Ok(_) => {}
            Err(e) => {
                tracing::debug!(address = %addr, error = %e, "auto-tag: entity lookup failed, skipping");
                continue;
            }
        }

        let candidates = match provider.resolve_tags(&addr).await {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!(address = %addr, error = %e, "auto-tag: provider lookup failed, skipping");
                continue;
            }
        };
        if candidates.is_empty() {
            continue;
        }

        let count = candidates.len();
        for candidate in candidates {
            let input = TagApplyInput {
                category: candidate.category,
                label_name: candidate.label_name.or(Some(candidate.raw_label)),
                source: candidate.source,
                confidence: candidate.confidence,
                risk_score: crate::labels::default_risk_for(candidate.category),
                sanction_list: None,
                expires_at: None,
                evidence_url: candidate.evidence_url,
            };
            if let Err(e) = apply_tag(cfg.entities.as_ref(), cfg.history.as_ref(), &addr, input).await {
                tracing::debug!(address = %addr, error = %e, "auto-tag: apply_tag failed");
            }
        }
        tracing::debug!(address = %addr, tags = count, "auto-tag: applied external candidates");
    }
}

async fn enrich_and_alert(
    fetched: &mut Vec<Transfer>,
    prices: Option<&Arc<dyn PricePort>>,
    watchlist: Option<&Arc<dyn WatchlistRepository>>,
    alerts: Option<&Arc<dyn AlertSink>>,
) {
    if let Some(p) = prices {
        let mut day_cache: HashMap<(domain::asset::AssetId, i64), Option<domain::price::UnitPrice>> =
            HashMap::new();
        for t in fetched.iter_mut() {
            let key = (t.asset().clone(), t.timestamp().timestamp() / 86_400);
            let price = match day_cache.get(&key) {
                Some(v) => *v,
                None => {
                    let v = p.price_at(t.asset(), t.timestamp()).await.unwrap_or(None);
                    day_cache.insert(key, v);
                    v
                }
            };
            if let Some(price) = price {
                t.set_usd_value(price.apply(t.amount()));
            }
        }
    }

    if let (Some(w), Some(a)) = (watchlist, alerts) {
        for t in fetched.iter() {
            for addr in [t.from(), t.to()] {
                if w.contains(addr).await.unwrap_or(false) {
                    let alert = Alert {
                        address: addr.clone(),
                        triggered_by_tx: *t.id().tx_hash(),
                        triggered_by_idx: t.id().index(),
                        created_at: Utc::now(),
                        reason: Some(format!("watchlist address {} touched", addr)),
                    };
                    let _ = a.record(alert).await;
                }
            }
        }
    }
}

impl<S, R> IngestionService<S, R>
where
    R: TransferRepository,
{
    #[tracing::instrument(skip(self, origin), fields(
        address = %origin,
        max_depth = req.max_depth(),
        max_nodes = req.max_nodes(),
    ))]
    pub async fn build_graph_from_db(
        &self,
        origin: &Address,
        req: GraphRequest,
    ) -> DomainResult<TransferGraph> {
        tracing::info!(
            origin = %origin,
            chain = %origin.chain(),
            max_depth = req.max_depth(),
            max_nodes = req.max_nodes(),
            range = ?req.range(),
            "DB-only graph build started"
        );

        let mut nodes: HashSet<Address> = HashSet::new();
        let mut edges: Vec<Transfer> = Vec::new();
        let mut visited: HashSet<Address> = HashSet::new();
        let mut queue: VecDeque<(Address, u32)> = VecDeque::new();

        queue.push_back((origin.clone(), 0));
        visited.insert(origin.clone());
        nodes.insert(origin.clone());

        let range = req.range();

        while let Some((addr, depth)) = queue.pop_front() {
            let transfers =
                fetch_all_for_address(&self.repo, &addr, range, req.max_transfers_per_address())
                    .await?;

            let next_depth = depth + 1;
            let can_expand = next_depth < req.max_depth();
            let mut enqueued = 0usize;
            let mut kept = 0usize;
            let mut skipped_failed = 0usize;

            for t in transfers.iter() {
                if !t.is_confirmed() {
                    skipped_failed += 1;
                    continue;
                }
                kept += 1;

                let counterparty = if t.from() == &addr {
                    t.to().clone()
                } else {
                    t.from().clone()
                };

                nodes.insert(t.from().clone());
                nodes.insert(t.to().clone());

                if can_expand
                    && !visited.contains(&counterparty)
                    && visited.len() < req.max_nodes()
                {
                    visited.insert(counterparty.clone());
                    queue.push_back((counterparty, next_depth));
                    enqueued += 1;
                }
            }

            tracing::debug!(
                address = %addr,
                depth,
                transfers = transfers.len(),
                kept,
                skipped_failed,
                enqueued,
                can_expand,
                nodes = nodes.len(),
                visited = visited.len(),
                queue = queue.len(),
                "DB graph BFS step"
            );

            edges.extend(transfers.into_iter().filter(|t| t.is_confirmed()));
        }

        edges.sort_by(|a, b| {
            a.id()
                .tx_hash()
                .cmp(b.id().tx_hash())
                .then(a.id().index().cmp(&b.id().index()))
        });
        edges.dedup_by(|a, b| a.id() == b.id());

        tracing::info!(
            origin = %origin,
            nodes = nodes.len(),
            edges = edges.len(),
            "DB-only graph build complete"
        );

        Ok(TransferGraph::new(nodes, edges))
    }
}

#[async_trait]
impl<S, R> IngestionPort for IngestionService<S, R>
where
    S: ChainSourceRegistry,
    R: TransferRepository,
{
    #[tracing::instrument(skip(self, origin), fields(
        address = %origin,
        max_depth = req.max_depth(),
        max_nodes = req.max_nodes(),
    ))]
    async fn build_graph(
        &self,
        origin: &Address,
        req: GraphRequest,
    ) -> DomainResult<TransferGraph> {
        tracing::info!(
            origin = %origin,
            chain = %origin.chain(),
            max_depth = req.max_depth(),
            max_nodes = req.max_nodes(),
            "graph build started"
        );

        let mut nodes: HashSet<Address> = HashSet::new();
        let mut edges: Vec<Transfer> = Vec::new();
        let mut visited: HashSet<Address> = HashSet::new();
        let mut queue: VecDeque<(Address, u32)> = VecDeque::new();

        queue.push_back((origin.clone(), 0));
        visited.insert(origin.clone());
        nodes.insert(origin.clone());

        let user_range = req.range().unwrap_or_else(BlockRange::full);
        let user_from = user_range.from_height();
        let user_to = user_range.to_height();

        let mut latest_by_chain: HashMap<ChainId, u64> = HashMap::new();

        while let Some((addr, depth)) = queue.pop_front() {
            let source = match self.sources.source(addr.chain()) {
                Some(s) => s,
                None => {
                    tracing::warn!(
                        address = %addr,
                        chain = %addr.chain(),
                        "no source registered for chain, skipping node"
                    );
                    continue;
                }
            };

            let latest_height = match latest_by_chain.get(&addr.chain()).copied() {
                Some(h) => h,
                None => match source.latest_block().await {
                    Ok(b) => {
                        let h = b.height();
                        latest_by_chain.insert(addr.chain(), h);
                        h
                    }
                    Err(e) => {
                        tracing::warn!(
                            chain = %addr.chain(),
                            error = %e,
                            "latest_block fetch failed; using user range as-is"
                        );
                        u64::MAX
                    }
                },
            };

            let confirmation_depth = self
                .chains
                .get(addr.chain())
                .map(|m| m.confirmation_depth())
                .unwrap_or(12);

            let min_known = self
                .repo
                .min_block_height(&addr)
                .await
                .unwrap_or_default();
            let max_known = self
                .repo
                .max_block_height(&addr)
                .await
                .unwrap_or_default();

            let spans = missing_spans(
                user_from,
                user_to,
                latest_height,
                min_known,
                max_known,
                confirmation_depth,
            );

            tracing::debug!(
                address = %addr,
                user_from,
                user_to,
                latest_height,
                min_known,
                max_known,
                confirmation_depth,
                spans = ?spans,
                "incremental ingest plan"
            );

            for (from_h, to_h) in spans {
                let range = BlockRange::new(from_h, to_h);
                // Adaptive throttle: parallel build_graph requests share
                // this gate; the gate shrinks on RateLimited (held permit
                // is forget()'d) and grows back on a success streak. Both
                // shifts are surfaced at WARN by the gate itself.
                let permit = self.transfers_gate.acquire().await;
                let outcome = source
                    .transfers_for_address(&addr, range, req.max_transfers_per_address())
                    .await;
                if let Some(p) = permit {
                    match &outcome {
                        Ok(_) => self.transfers_gate.release_after_success(p),
                        Err(DomainError::RateLimited(_)) => {
                            self.transfers_gate.shrink_after_rate_limit(p)
                        }
                        Err(_) => self.transfers_gate.release_after_other_error(p),
                    }
                }
                match outcome {
                    Ok(mut fetched) => {
                        tracing::debug!(
                            address = %addr,
                            from_h,
                            to_h,
                            fetched = fetched.len(),
                            "span fetched from chain"
                        );
                        enrich_and_alert(
                            &mut fetched,
                            self.prices.as_ref(),
                            self.watchlist.as_ref(),
                            self.alerts.as_ref(),
                        )
                        .await;
                        // delete only the actually-fetched span so prior
                        // cold data outside the span (other ranges, other
                        // ingests) is preserved
                        if let Err(e) = self.repo.delete_in_range(&addr, from_h, to_h).await {
                            tracing::warn!(
                                address = %addr,
                                from_h,
                                to_h,
                                error = %e,
                                "delete-in-range failed; proceeding with save"
                            );
                        }
                        if !fetched.is_empty() {
                            if let Err(e) = self.repo.save(&fetched).await {
                                tracing::warn!(address = %addr, error = %e, "save failed");
                            }
                            classify_address_kinds(
                                source.as_ref(),
                                self.address_kinds.as_ref(),
                                &fetched,
                                self.classify_chain_batch_size,
                            )
                            .await;
                            auto_tag_addresses(self.auto_tagging.as_ref(), &fetched).await;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            address = %addr,
                            from_h,
                            to_h,
                            error = %e,
                            "chain fetch failed, falling back to DB"
                        );
                    }
                }
            }

            let combined = fetch_all_for_address(
                &self.repo,
                &addr,
                Some(user_range),
                req.max_transfers_per_address(),
            )
            .await
            .unwrap_or_default();

            let mut kept = 0usize;
            let mut skipped_failed = 0usize;

            let next_depth = depth + 1;
            let can_expand = next_depth < req.max_depth();

            for t in combined {
                if !t.is_confirmed() {
                    skipped_failed += 1;
                    continue;
                }
                kept += 1;

                let counterparty = if t.from() == &addr {
                    t.to().clone()
                } else {
                    t.from().clone()
                };

                nodes.insert(t.from().clone());
                nodes.insert(t.to().clone());
                edges.push(t);

                if can_expand && !visited.contains(&counterparty) && visited.len() < req.max_nodes()
                {
                    visited.insert(counterparty.clone());
                    queue.push_back((counterparty, next_depth));
                }
            }

            tracing::debug!(
                address = %addr, depth,
                kept, skipped_failed,
                visited = visited.len(),
                nodes = nodes.len(),
                "graph BFS step"
            );
        }

        edges.sort_by(|a, b| {
            a.id()
                .tx_hash()
                .cmp(b.id().tx_hash())
                .then(a.id().index().cmp(&b.id().index()))
        });
        edges.dedup_by(|a, b| a.id() == b.id());

        tracing::info!(
            origin = %origin,
            nodes = nodes.len(), edges = edges.len(),
            "graph build complete"
        );

        Ok(TransferGraph::new(nodes, edges))
    }
}

const FETCH_PAGE_SIZE: usize = 1_000;

/// Read all transfers for `addr` within `range` via keyset pagination, capped
/// by `cap`. Returns at most `cap` rows even if more exist downstream.
async fn fetch_all_for_address<R: TransferRepository>(
    repo: &R,
    addr: &Address,
    range: Option<BlockRange>,
    cap: usize,
) -> DomainResult<Vec<Transfer>> {
    let mut out: Vec<Transfer> = Vec::new();
    let mut cursor: Option<TransferCursor> = None;

    while out.len() < cap {
        let want = FETCH_PAGE_SIZE.min(cap - out.len());
        let page = repo.find_by_address(addr, range, cursor, want).await?;
        let got = page.len();
        if got == 0 {
            break;
        }
        if let Some(last) = page.last() {
            cursor = Some(TransferCursor {
                block_height: last.block().height(),
                idx: last.id().index(),
            });
        }
        out.extend(page);
        if got < want {
            break;
        }
    }

    Ok(out)
}

/// Compute which block-height spans an incremental ingest must actually fetch
/// from the chain, given what is already persisted in the repo.
///
/// Semantics:
/// * If the repo has no data for the address → fetch `[user_from, min(user_to, latest)]`.
/// * Otherwise refetch the "hot tail" `[max_known - confirmation_depth + 1, effective_to]`
///   (to recover from reorgs) and any prefix gap `[user_from, min_known - 1]` if the
///   caller widened `from_block` below previously persisted data.
/// Cold middle blocks `[min_known, max_known - confirmation_depth]` are NEVER refetched.
pub(crate) fn missing_spans(
    user_from: u64,
    user_to: u64,
    latest: u64,
    min_known: Option<u64>,
    max_known: Option<u64>,
    confirmation_depth: u64,
) -> Vec<(u64, u64)> {
    let effective_to = user_to.min(latest);
    if effective_to < user_from {
        return Vec::new();
    }

    match (min_known, max_known) {
        (Some(min_h), Some(max_h)) => {
            let mut spans = Vec::new();

            if user_from < min_h {
                let prefix_to = min_h.saturating_sub(1).min(effective_to);
                if prefix_to >= user_from {
                    spans.push((user_from, prefix_to));
                }
            }

            let hot_start = max_h
                .saturating_sub(confirmation_depth)
                .saturating_add(1)
                .max(user_from);
            if effective_to >= hot_start {
                spans.push((hot_start, effective_to));
            }

            spans
        }
        _ => vec![(user_from, effective_to)],
    }
}

#[cfg(test)]
mod tests {
    use super::missing_spans;

    #[test]
    fn no_data_yet_returns_full_user_range() {
        assert_eq!(
            missing_spans(100, 200, 500, None, None, 12),
            vec![(100, 200)]
        );
    }

    #[test]
    fn empty_range_when_user_to_below_user_from() {
        assert_eq!(
            missing_spans(200, 100, 500, None, None, 12),
            Vec::<(u64, u64)>::new()
        );
    }

    #[test]
    fn clamps_to_latest() {
        assert_eq!(
            missing_spans(100, 1000, 500, None, None, 12),
            vec![(100, 500)]
        );
    }

    #[test]
    fn fully_covered_only_refetches_hot_tail() {
        assert_eq!(
            missing_spans(100, 200, 500, Some(100), Some(200), 12),
            vec![(189, 200)]
        );
    }

    #[test]
    fn historical_lower_widening_fetches_prefix_only() {
        assert_eq!(
            missing_spans(60, 150, 500, Some(100), Some(200), 12),
            vec![(60, 99)]
        );
    }

    #[test]
    fn fully_covered_historical_inside_db_no_fetch() {
        assert_eq!(
            missing_spans(120, 150, 500, Some(100), Some(200), 12),
            Vec::<(u64, u64)>::new()
        );
    }

    #[test]
    fn extends_upper_bound_only_fetches_hot_tail_plus_new() {
        assert_eq!(
            missing_spans(100, 300, 500, Some(100), Some(200), 12),
            vec![(189, 300)]
        );
    }

    #[test]
    fn extends_both_lower_and_upper() {
        assert_eq!(
            missing_spans(40, 300, 500, Some(100), Some(200), 12),
            vec![(40, 99), (189, 300)]
        );
    }

    #[test]
    fn user_from_above_max_known_only_suffix() {
        assert_eq!(
            missing_spans(250, 400, 500, Some(100), Some(200), 12),
            vec![(250, 400)]
        );
    }
}

#[cfg(test)]
mod adaptive_concurrency_tests {
    use super::AdaptiveConcurrency;

    #[tokio::test]
    async fn rate_limit_shrinks_one_permit_at_a_time() {
        let gate = AdaptiveConcurrency::new(4, 1, 8, 20);
        assert_eq!(gate.current_permits(), 4);

        // Each rate-limit eats one permit (forget) until we hit the floor.
        for expected_after in [3u32, 2, 1] {
            let p = gate.acquire().await.unwrap();
            gate.shrink_after_rate_limit(p);
            assert_eq!(gate.current_permits(), expected_after);
        }
        // At floor: shrink no longer changes the counter.
        let p = gate.acquire().await.unwrap();
        gate.shrink_after_rate_limit(p);
        assert_eq!(gate.current_permits(), 1);
    }

    #[tokio::test]
    async fn success_streak_grows_permit_then_resets() {
        let gate = AdaptiveConcurrency::new(2, 1, 4, 3);

        // Shrink first to leave room to grow back.
        let p = gate.acquire().await.unwrap();
        gate.shrink_after_rate_limit(p);
        assert_eq!(gate.current_permits(), 1);

        // First two successes don't reach the grow threshold yet.
        for _ in 0..2 {
            let p = gate.acquire().await.unwrap();
            gate.release_after_success(p);
        }
        assert_eq!(gate.current_permits(), 1);

        // Third success crosses the threshold → +1 permit, streak resets.
        let p = gate.acquire().await.unwrap();
        gate.release_after_success(p);
        assert_eq!(gate.current_permits(), 2);

        // Two more successes — not enough to grow again because streak reset.
        for _ in 0..2 {
            let p = gate.acquire().await.unwrap();
            gate.release_after_success(p);
        }
        assert_eq!(gate.current_permits(), 2);

        // Third post-grow success grows again.
        let p = gate.acquire().await.unwrap();
        gate.release_after_success(p);
        assert_eq!(gate.current_permits(), 3);
    }

    #[tokio::test]
    async fn growth_stops_at_ceiling() {
        let gate = AdaptiveConcurrency::new(2, 1, 2, 1);
        let p = gate.acquire().await.unwrap();
        gate.release_after_success(p);
        // Cannot grow above max.
        assert_eq!(gate.current_permits(), 2);
    }

    #[tokio::test]
    async fn rate_limit_resets_success_streak() {
        // Build up streak, then a rate-limit must reset it — so the next
        // success alone should NOT trigger growth.
        let gate = AdaptiveConcurrency::new(1, 1, 4, 3);
        for _ in 0..2 {
            let p = gate.acquire().await.unwrap();
            gate.release_after_success(p);
        }
        // Streak = 2. Rate-limit zeroes it; shrink no-ops because we're at floor.
        let p = gate.acquire().await.unwrap();
        gate.shrink_after_rate_limit(p);
        assert_eq!(gate.current_permits(), 1);

        // Single success after reset must not grow (would need 3 in a row).
        let p = gate.acquire().await.unwrap();
        gate.release_after_success(p);
        assert_eq!(gate.current_permits(), 1);
    }

    #[tokio::test]
    async fn other_errors_do_not_change_state() {
        let gate = AdaptiveConcurrency::new(4, 1, 8, 20);
        let p = gate.acquire().await.unwrap();
        gate.release_after_other_error(p);
        assert_eq!(gate.current_permits(), 4);
        // Streak isn't bumped either — a subsequent rate-limit still
        // shrinks normally without odd interactions.
        let p = gate.acquire().await.unwrap();
        gate.shrink_after_rate_limit(p);
        assert_eq!(gate.current_permits(), 3);
    }

    #[tokio::test]
    async fn invariants_held_when_initial_outside_bounds() {
        // initial above max → clamped to max; below min → clamped to min.
        let above = AdaptiveConcurrency::new(99, 2, 5, 10);
        assert_eq!(above.current_permits(), 5);
        let below = AdaptiveConcurrency::new(0, 2, 5, 10);
        assert_eq!(below.current_permits(), 2);
    }
}
