using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Groups.Commands;

public sealed record RenameGroupCommand(Guid CaseId, Guid GroupId, String Name)
    : IRequest, IRequirePermission {
    public Permission Required => Permission.GroupUpdate;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class RenameGroupCommandHandler(IAddressGroupRepository groups)
    : IRequestHandler<RenameGroupCommand> {
    private readonly IAddressGroupRepository groups = groups;

    public async Task Handle(RenameGroupCommand request, CancellationToken cancellationToken) {
        var group = await groups.GetByIdAsync(request.GroupId, cancellationToken);
        if (group is null || group.CaseId != request.CaseId) {
            throw new GroupNotFoundException(request.GroupId);
        }

        group.Rename(request.Name);
        await groups.SaveChangesAsync(cancellationToken);
    }
}
