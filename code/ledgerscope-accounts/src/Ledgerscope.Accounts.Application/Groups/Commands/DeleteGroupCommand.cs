using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Groups.Commands;

public sealed record DeleteGroupCommand(Guid CaseId, Guid GroupId)
    : IRequest, IRequirePermission {
    public Permission Required => Permission.GroupDelete;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class DeleteGroupCommandHandler(IAddressGroupRepository groups)
    : IRequestHandler<DeleteGroupCommand> {
    private readonly IAddressGroupRepository groups = groups;

    public async Task Handle(DeleteGroupCommand request, CancellationToken cancellationToken) {
        var group = await groups.GetByIdAsync(request.GroupId, cancellationToken);
        if (group is null || group.CaseId != request.CaseId) {
            throw new GroupNotFoundException(request.GroupId);
        }

        groups.Remove(group);
        await groups.SaveChangesAsync(cancellationToken);
    }
}
