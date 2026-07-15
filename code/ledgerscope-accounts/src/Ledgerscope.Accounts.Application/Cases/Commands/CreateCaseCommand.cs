using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using Ledgerscope.Accounts.Domain.Cases;
using MediatR;

namespace Ledgerscope.Accounts.Application.Cases.Commands;

public sealed record CreateCaseCommand(
    String Title, String Description, CasePriority Priority, Guid OrganizationId)
    : IRequest<Guid>, IRequirePermission {
    public Permission Required => Permission.CaseCreate;

    public Guid? CaseId => null; // global permission check
}

public sealed class CreateCaseCommandHandler(ICaseRepository cases, IUserContext user, IPublisher publisher) : IRequestHandler<CreateCaseCommand, Guid> {
    private readonly ICaseRepository cases = cases;
    private readonly IUserContext user = user;
    private readonly IPublisher publisher = publisher;

    public async Task<Guid> Handle(CreateCaseCommand request, CancellationToken cancellationToken) {
        var now = DateTimeOffset.UtcNow;
        var entity = new Case(
            Guid.NewGuid(), request.Title, request.Description, request.Priority,
            request.OrganizationId, user.UserId, now);

        // The creator gets case write access immediately as the Lead.
        _ = entity.Assign(user.UserId, CaseRoles.Lead, user.UserId, now);

        await cases.AddAsync(entity, cancellationToken);
        await cases.SaveChangesAsync(cancellationToken);

        // The creator's effective permissions on this new case just changed.
        await publisher.Publish(new CasePermissionsChangedEvent(user.UserId, entity.Id), cancellationToken);

        return entity.Id;
    }
}
