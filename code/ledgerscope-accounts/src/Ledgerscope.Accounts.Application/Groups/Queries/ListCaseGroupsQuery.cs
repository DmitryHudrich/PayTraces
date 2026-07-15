using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Groups.Queries;

public sealed record ListCaseGroupsQuery(Guid CaseId)
    : IRequest<IReadOnlyList<AddressGroupSummaryDto>>, IRequirePermission {
    public Permission Required => Permission.CaseRead;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class ListCaseGroupsQueryHandler(IAddressGroupRepository groups)
    : IRequestHandler<ListCaseGroupsQuery, IReadOnlyList<AddressGroupSummaryDto>> {
    private readonly IAddressGroupRepository groups = groups;

    public async Task<IReadOnlyList<AddressGroupSummaryDto>> Handle(
        ListCaseGroupsQuery request, CancellationToken cancellationToken) {
        var found = await groups.ListByCaseAsync(request.CaseId, cancellationToken);
        return [.. found.Select(group => group.ToSummary())];
    }
}
