using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Cases.Commands;

public sealed record AssignCaseToUserCommand(Guid CaseId, Guid UserId, String RoleName)
    : IRequest, IRequirePermission {
    public Permission Required => Permission.CaseAssign;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class AssignCaseToUserCommandHandler(ICaseRepository cases, IUserContext user, IPublisher publisher) : IRequestHandler<AssignCaseToUserCommand> {
    private readonly ICaseRepository cases = cases;
    private readonly IUserContext user = user;
    private readonly IPublisher publisher = publisher;

    public async Task Handle(AssignCaseToUserCommand request, CancellationToken cancellationToken) {
        var entity = await cases.GetByIdAsync(request.CaseId, cancellationToken)
            ?? throw new CaseNotFoundException(request.CaseId);

        _ = entity.Assign(request.UserId, request.RoleName, user.UserId, DateTimeOffset.UtcNow);
        await cases.SaveChangesAsync(cancellationToken);

        // The assigned user's effective permissions on this case changed.
        await publisher.Publish(
            new CasePermissionsChangedEvent(request.UserId, request.CaseId), cancellationToken);
    }
}
