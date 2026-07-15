using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Groups.Commands;

public sealed record AddGroupMemberCommand(Guid CaseId, Guid GroupId, String Address, Int32 ChainId)
    : IRequest, IRequirePermission {
    public Permission Required => Permission.GroupUpdate;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class AddGroupMemberCommandHandler(IAddressGroupRepository groups)
    : IRequestHandler<AddGroupMemberCommand> {
    private readonly IAddressGroupRepository groups = groups;

    public async Task Handle(AddGroupMemberCommand request, CancellationToken cancellationToken) {
        var group = await groups.GetByIdAsync(request.GroupId, cancellationToken);
        if (group is null || group.CaseId != request.CaseId) {
            throw new GroupNotFoundException(request.GroupId);
        }

        _ = group.AddMember(request.Address, request.ChainId, DateTimeOffset.UtcNow);
        await groups.SaveChangesAsync(cancellationToken);
    }
}
