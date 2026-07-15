using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Graph;

// Every request is case-scoped: the authorization behaviour checks the caller's
// permission on CaseId before the handler talks to the engine. Reads need
// CaseRead; kicking off an ingest needs CaseAddressAdd (it pulls data for the
// case's addresses).

public sealed record StartIngestCommand(
    Guid CaseId, String Address, Int32 ChainId, Int64? FromBlock, Int64? ToBlock, Int32? MaxDepth, Int32? MaxNodes)
    : IRequest<IngestAcceptedDto>, IRequirePermission {
    public Permission Required => Permission.CaseAddressAdd;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class StartIngestCommandHandler(IGraphEngineClient engine)
        : IRequestHandler<StartIngestCommand, IngestAcceptedDto> {
    private readonly IGraphEngineClient engine = engine;

    public Task<IngestAcceptedDto> Handle(StartIngestCommand request, CancellationToken cancellationToken) =>
        engine.CreateIngestJobAsync(
            request.Address, request.ChainId, request.FromBlock, request.ToBlock,
            request.MaxDepth, request.MaxNodes, cancellationToken);
}

public sealed record GetIngestJobQuery(Guid CaseId, String JobId) : IRequest<JobStatusDto>, IRequirePermission {
    public Permission Required => Permission.CaseRead;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class GetIngestJobQueryHandler(IGraphEngineClient engine)
        : IRequestHandler<GetIngestJobQuery, JobStatusDto> {
    private readonly IGraphEngineClient engine = engine;

    public Task<JobStatusDto> Handle(GetIngestJobQuery request, CancellationToken cancellationToken) =>
        engine.GetJobStatusAsync(request.JobId, cancellationToken);
}

public sealed record GetAddressScoreQuery(Guid CaseId, String Address, Int32 ChainId)
    : IRequest<ScoreDto>, IRequirePermission {
    public Permission Required => Permission.CaseRead;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class GetAddressScoreQueryHandler(IGraphEngineClient engine)
        : IRequestHandler<GetAddressScoreQuery, ScoreDto> {
    private readonly IGraphEngineClient engine = engine;

    public Task<ScoreDto> Handle(GetAddressScoreQuery request, CancellationToken cancellationToken) =>
        engine.GetScoreAsync(request.Address, request.ChainId, cancellationToken);
}

public sealed record GetAddressHeuristicsQuery(Guid CaseId, String Address, Int32 ChainId)
    : IRequest<HeuristicsDto>, IRequirePermission {
    public Permission Required => Permission.CaseRead;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class GetAddressHeuristicsQueryHandler(IGraphEngineClient engine)
        : IRequestHandler<GetAddressHeuristicsQuery, HeuristicsDto> {
    private readonly IGraphEngineClient engine = engine;

    public Task<HeuristicsDto> Handle(GetAddressHeuristicsQuery request, CancellationToken cancellationToken) =>
        engine.GetHeuristicsAsync(request.Address, request.ChainId, cancellationToken);
}

public sealed record GetAddressClusterQuery(Guid CaseId, String Address, Int32 ChainId)
    : IRequest<ClusterDto>, IRequirePermission {
    public Permission Required => Permission.CaseRead;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class GetAddressClusterQueryHandler(IGraphEngineClient engine)
        : IRequestHandler<GetAddressClusterQuery, ClusterDto> {
    private readonly IGraphEngineClient engine = engine;

    public Task<ClusterDto> Handle(GetAddressClusterQuery request, CancellationToken cancellationToken) =>
        engine.GetClusterAsync(request.Address, request.ChainId, cancellationToken);
}

public sealed record GetAddressEntityQuery(Guid CaseId, String Address, Int32 ChainId)
    : IRequest<AddressEntityDto?>, IRequirePermission {
    public Permission Required => Permission.CaseRead;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class GetAddressEntityQueryHandler(IGraphEngineClient engine)
        : IRequestHandler<GetAddressEntityQuery, AddressEntityDto?> {
    private readonly IGraphEngineClient engine = engine;

    public Task<AddressEntityDto?> Handle(GetAddressEntityQuery request, CancellationToken cancellationToken) =>
        engine.GetAddressEntityAsync(request.Address, request.ChainId, cancellationToken);
}
