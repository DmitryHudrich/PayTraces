namespace Ledgerscope.Accounts.Application.Graph;

/// <summary>
/// One enriched graph node as returned by the Rust engine (its <c>NodeDto</c>).
/// Property names map from the engine's snake_case JSON via the client's
/// serializer options.
/// </summary>
public sealed record GraphNode(
    String Address,
    String? Kind,
    String? ServiceName,
    Int32? RiskScore,
    Boolean? IsHighRisk,
    Int32? InDegree,
    Int32? OutDegree,
    Int32? TxCount,
    Boolean? IsViewBoundary,
    Boolean? IsIngestBoundary);

/// <summary>
/// A case's graph view for the frontend: the addresses recorded on the case,
/// each enriched with the engine's live node data. This is the BFF shape —
/// case metadata (from Ledgerscope's DB) fused with graph data (from Rust).
/// </summary>
public sealed record CaseGraphDto(Guid CaseId, IReadOnlyList<GraphNode> Nodes);

/// <summary>
/// One transfer edge from the engine's <c>EdgeDto</c>.
/// </summary>
public sealed record GraphEdge(
    String TxHash,
    Int32 Index,
    String From,
    String To,
    String Raw,
    String Formatted,
    String Symbol,
    Int32 Decimals,
    Int64 Block,
    Int64 Ts,
    String Kind,
    String? Contract,
    Int32 ChainId);

/// <summary>
/// One page of the engine's paginated <c>/graph</c> BFS walk. The engine only
/// populates <see cref="Nodes"/> on page 0 (the node set is global per
/// request); later pages carry only more <see cref="Edges"/>. Drives the
/// SignalR hub's progressive loading.
/// </summary>
public sealed record GraphPageDto(
    Int32 TotalNodes,
    Int32 TotalEdges,
    Int32 Page,
    Int32 PageSize,
    Int32 TotalPages,
    Boolean HasNext,
    IReadOnlyList<GraphNode> Nodes,
    IReadOnlyList<GraphEdge> Edges);
