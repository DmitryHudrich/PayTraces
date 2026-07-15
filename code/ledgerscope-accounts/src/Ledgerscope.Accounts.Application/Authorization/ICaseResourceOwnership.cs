using Ledgerscope.Accounts.Domain.Authorization;

namespace Ledgerscope.Accounts.Application.Authorization;

/// <summary>
/// The "owner — unless you hold the case-wide override" rule for private
/// per-user case resources (canvas views). Used both by the
/// <see cref="Behaviors.OwnershipBehavior{TRequest,TResponse}"/> pipeline step
/// (where the resource state travels on the request) and directly by handlers
/// that must load an existing resource before its owner/shared flags are known.
/// </summary>
public interface ICaseResourceOwnership {
    Task EnsureCanMutateAsync(Guid ownerId, Boolean isShared, Guid caseId, CancellationToken ct);
}

public sealed class CaseResourceOwnership(IUserContext user, IPermissionResolver permissions) : ICaseResourceOwnership {
    private readonly IUserContext user = user;
    private readonly IPermissionResolver permissions = permissions;

    public async Task EnsureCanMutateAsync(Guid ownerId, Boolean isShared, Guid caseId, CancellationToken ct) {
        if (isShared || ownerId == user.UserId) {
            return;
        }

        var canOverride = await permissions.HasAsync(
            user.UserId, Permission.ViewManageSharing, caseId, ct);

        if (!canOverride) {
            throw new ForbiddenException(
                "This is another user's private resource; you are not its owner.");
        }
    }
}
