namespace Ledgerscope.Accounts.Application.Graph;

/// <summary>
/// Engine ingest job accepted (<c>POST /jobs/ingest</c>). The frontend polls
/// <see cref="JobStatusDto"/> until the job reaches a terminal state, then
/// streams the freshly-persisted graph.
/// </summary>
public sealed record IngestAcceptedDto(String JobId);

/// <summary>Engine job status (<c>GET /jobs/{id}</c>).</summary>
public sealed record JobStatusDto(
    String Id, String Status, String? Error, String CreatedAt, String UpdatedAt);

/// <summary>One explainable risk signal contributing to a score.</summary>
public sealed record SignalDto(String Kind, Int32 Severity, String Description, String? TagId);

/// <summary>Engine risk score with signals (<c>GET /score</c>).</summary>
public sealed record ScoreDto(
    String Address,
    Int32 ChainId,
    Int32 Score,
    Boolean IsHighRisk,
    IReadOnlyList<SignalDto> Signals,
    String GeneratedAt);

/// <summary>One behavioural-pattern detector that fired.</summary>
public sealed record HeuristicEvidenceDto(
    String Heuristic, String Confidence, IReadOnlyList<String> Addresses, String? Notes);

/// <summary>Engine behavioural heuristics (<c>GET /heuristics</c>). Each field
/// is null when that detector did not match.</summary>
public sealed record HeuristicsDto(
    String Address,
    HeuristicEvidenceDto? FanOut,
    HeuristicEvidenceDto? FanIn,
    HeuristicEvidenceDto? SmurfingCycle,
    HeuristicEvidenceDto? TemporalBurst,
    HeuristicEvidenceDto? FixedAmountClustering,
    HeuristicEvidenceDto? DwellTimePassThrough,
    HeuristicEvidenceDto? PeelingChain,
    HeuristicEvidenceDto? DepositAddressReuse);

/// <summary>Engine co-ownership clustering (<c>GET /cluster</c>). The queried
/// address is always in <c>components[0]</c>.</summary>
public sealed record ClusterDto(String Address, IReadOnlyList<IReadOnlyList<String>> Components);

/// <summary>One automatic risk tag attached to an address's entity.</summary>
public sealed record LabelTagDto(
    String TagId,
    String Category,
    String? LabelName,
    String Source,
    Int32 Confidence,
    Int32 RiskScore,
    String? SanctionList,
    Boolean Active,
    String? SupersededBy,
    String CreatedAt,
    String? ExpiresAt,
    String? EvidenceUrl);

public sealed record EntityAddressDto(String Address, Int32 ChainId, String AttachedAt);

/// <summary>The engine's authoritative entity for an address (<c>GET
/// /labels/{addr}</c>) — its sibling addresses and automatic risk tags. This is
/// distinct from the case's own <see cref="Labels.CustomLabelDto"/> annotations.</summary>
public sealed record AddressEntityDto(
    String EntityId,
    IReadOnlyList<EntityAddressDto> Addresses,
    IReadOnlyList<LabelTagDto> Tags,
    Int32 AggregateRiskScore);
