use std::collections::{HashMap, HashSet, VecDeque};

use async_trait::async_trait;
use domain::chain::{ChainId, ChainRegistry};
use domain::error::DomainResult;
use domain::graph::{GraphRequest, TransferGraph};
use domain::ports::{
    BlockRange, ChainSourceRegistry, IngestionPort, TransferCursor, TransferRepository,
};
use domain::primitives::Address;
use domain::transfer::Transfer;

pub struct IngestionService<S, R> {
    sources: S,
    repo: R,
    chains: ChainRegistry,
}

impl<S, R> IngestionService<S, R> {
    pub fn new(sources: S, repo: R, chains: ChainRegistry) -> Self {
        Self { sources, repo, chains }
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
                match source
                    .transfers_for_address(&addr, range, req.max_transfers_per_address())
                    .await
                {
                    Ok(fetched) => {
                        tracing::debug!(
                            address = %addr,
                            from_h,
                            to_h,
                            fetched = fetched.len(),
                            "span fetched from chain"
                        );
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
