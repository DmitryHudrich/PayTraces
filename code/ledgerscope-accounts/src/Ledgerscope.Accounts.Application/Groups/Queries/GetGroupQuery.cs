using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Groups.Queries;

public sealed record GetGroupQuery(Guid CaseId, Guid GroupId)
    : IRequest<AddressGroupDto>, IRequirePermission {
    public Permission Required => Permission.CaseRead;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class GetGroupQueryHandler(IAddressGroupRepository groups)
    : IRequestHandler<GetGroupQuery, AddressGroupDto> {
    private readonly IAddressGroupRepository groups = groups;

    public async Task<AddressGroupDto> Handle(GetGroupQuery request, CancellationToken cancellationToken) {
        var group = await groups.GetByIdAsync(request.GroupId, cancellationToken);
        if (group is null || group.CaseId != request.CaseId) {
            throw new GroupNotFoundException(request.GroupId);
        }

        return group.ToDto();
    }
}
