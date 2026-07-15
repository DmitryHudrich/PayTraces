namespace Ledgerscope.Accounts.Application.Graph;

public sealed record GraphAddress(String Address, Int32 ChainId);

/// <summary>
/// Read-only client for the Rust engine's internal API. Ledgerscope authorizes
/// the caller and checks case access first, then reads graph data through this.
/// Implemented in Infrastructure as a typed HttpClient (shared secret / mTLS).
/// </summary>
public interface IGraphEngineClient {
    /// <summary>
    /// Batch-enrich addresses (engine <c>GET /nodes/batch</c>). All addresses
    /// must share a chain.
    /// </summary>
    Task<IReadOnlyList<GraphNode>> GetNodesBatchAsync(
        IReadOnlyCollection<String> addresses, Int32 chainId, CancellationToken ct);

    /// <summary>
    /// One page of the engine's BFS graph walk around <paramref name="address"/>
    /// (engine <c>GET /graph</c>). Nodes are only present on page 0.
    /// </summary>
    Task<GraphPageDto> GetGraphPageAsync(
        String address, Int32 chainId, Int32 maxDepth, Int32 page, Int32 pageSize, CancellationToken ct);

    /// <summary>
    /// Kick off an ingest job (engine <c>POST /jobs/ingest</c>) that pulls
    /// on-chain data from the external provider into the engine's store for the
    /// given address and optional block range.
    /// </summary>
    Task<IngestAcceptedDto> CreateIngestJobAsync(
        String address, Int32 chainId, Int64? fromBlock, Int64? toBlock, Int32? maxDepth, Int32? maxNodes,
        CancellationToken ct);

    /// <summary>Poll an ingest job's status (engine <c>GET /jobs/{id}</c>).</summary>
    Task<JobStatusDto> GetJobStatusAsync(String jobId, CancellationToken ct);

    /// <summary>Explainable 0–100 risk score (engine <c>GET /score</c>).</summary>
    Task<ScoreDto> GetScoreAsync(String address, Int32 chainId, CancellationToken ct);

    /// <summary>Behavioural-pattern heuristics (engine <c>GET /heuristics</c>).</summary>
    Task<HeuristicsDto> GetHeuristicsAsync(String address, Int32 chainId, CancellationToken ct);

    /// <summary>Co-ownership clustering (engine <c>GET /cluster</c>).</summary>
    Task<ClusterDto> GetClusterAsync(String address, Int32 chainId, CancellationToken ct);

    /// <summary>The engine's authoritative entity + automatic tags for an
    /// address (engine <c>GET /labels/{addr}</c>). Returns null when the engine
    /// has no entity for the address.</summary>
    Task<AddressEntityDto?> GetAddressEntityAsync(String address, Int32 chainId, CancellationToken ct);
}
