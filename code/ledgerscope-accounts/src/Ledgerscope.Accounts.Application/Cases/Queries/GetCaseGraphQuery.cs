using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Application.Graph;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Cases.Queries;

/// <summary>
/// BFF read: authorizes case access, then fuses the case's recorded addresses
/// (Ledgerscope DB) with live enrichment from the Rust engine. Addresses are
/// grouped by chain and enriched per chain via the engine's batch endpoint.
/// </summary>
public sealed record GetCaseGraphQuery(Guid CaseId) : IRequest<CaseGraphDto>, IRequirePermission {
    public Permission Required => Permission.CaseRead;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class GetCaseGraphQueryHandler(ICaseRepository cases, IGraphEngineClient engine) : IRequestHandler<GetCaseGraphQuery, CaseGraphDto> {
    private readonly ICaseRepository cases = cases;
    private readonly IGraphEngineClient engine = engine;

    public async Task<CaseGraphDto> Handle(GetCaseGraphQuery request, CancellationToken cancellationToken) {
        var entity = await cases.GetByIdAsync(request.CaseId, cancellationToken)
            ?? throw new CaseNotFoundException(request.CaseId);

        var nodes = new List<GraphNode>();
        foreach (var byChain in entity.Addresses.GroupBy(address => address.ChainId)) {
            var addresses = byChain.Select(address => address.Address).ToArray();
            nodes.AddRange(await engine.GetNodesBatchAsync(addresses, byChain.Key, cancellationToken));
        }

        return new CaseGraphDto(request.CaseId, nodes);
    }
}
