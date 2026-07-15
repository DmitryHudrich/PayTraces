using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Groups.Commands;

public sealed record RemoveGroupMemberCommand(Guid CaseId, Guid GroupId, String Address, Int32 ChainId)
    : IRequest, IRequirePermission {
    public Permission Required => Permission.GroupUpdate;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class RemoveGroupMemberCommandHandler(IAddressGroupRepository groups)
    : IRequestHandler<RemoveGroupMemberCommand> {
    private readonly IAddressGroupRepository groups = groups;

    public async Task Handle(RemoveGroupMemberCommand request, CancellationToken cancellationToken) {
        var group = await groups.GetByIdAsync(request.GroupId, cancellationToken);
        if (group is null || group.CaseId != request.CaseId) {
            throw new GroupNotFoundException(request.GroupId);
        }

        group.RemoveMember(request.Address, request.ChainId);
        await groups.SaveChangesAsync(cancellationToken);
    }
}
