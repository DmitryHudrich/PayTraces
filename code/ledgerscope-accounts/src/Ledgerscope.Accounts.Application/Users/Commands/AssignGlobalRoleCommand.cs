using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using Ledgerscope.Accounts.Domain.Identity;
using MediatR;

namespace Ledgerscope.Accounts.Application.Users.Commands;

public sealed record AssignGlobalRoleCommand(Guid UserId, String RoleName) : IRequest, IRequirePermission {
    public Permission Required => Permission.RoleManage;

    public Guid? CaseId => null; // global permission check
}

public sealed class AssignGlobalRoleCommandHandler(IUserRepository users, IPublisher publisher) : IRequestHandler<AssignGlobalRoleCommand> {
    private readonly IUserRepository users = users;
    private readonly IPublisher publisher = publisher;

    public async Task Handle(AssignGlobalRoleCommand request, CancellationToken cancellationToken) {
        var user = await users.GetByIdAsync(request.UserId, cancellationToken)
            ?? throw new UserNotFoundException(request.UserId);

        await users.AddGlobalRoleAsync(
            new UserRoleAssignment(
                Guid.NewGuid(), user.Id, request.RoleName, user.OrganizationId, user.Id, DateTimeOffset.UtcNow),
            cancellationToken);
        await users.SaveChangesAsync(cancellationToken);

        // The user's global effective permissions changed.
        await publisher.Publish(new CasePermissionsChangedEvent(user.Id, null), cancellationToken);
    }
}
