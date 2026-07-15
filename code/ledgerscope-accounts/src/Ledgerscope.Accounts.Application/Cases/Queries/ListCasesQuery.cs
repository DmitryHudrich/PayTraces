using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Cases.Queries;

public sealed record ListCasesQuery(Guid OrganizationId)
    : IRequest<IReadOnlyList<CaseSummaryDto>>, IRequirePermission {
    public Permission Required => Permission.CaseRead;

    public Guid? CaseId => null; // org-wide read governed by the global CaseRead permission
}

public sealed class ListCasesQueryHandler(ICaseRepository cases)
        : IRequestHandler<ListCasesQuery, IReadOnlyList<CaseSummaryDto>> {
    private readonly ICaseRepository cases = cases;

    public async Task<IReadOnlyList<CaseSummaryDto>> Handle(
        ListCasesQuery request, CancellationToken cancellationToken) {
        var items = await cases.ListByOrganizationAsync(request.OrganizationId, cancellationToken);
        return [.. items.Select(item => item.ToSummary())];
    }
}
