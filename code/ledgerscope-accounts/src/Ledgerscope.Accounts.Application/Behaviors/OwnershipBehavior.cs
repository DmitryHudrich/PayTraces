using Ledgerscope.Accounts.Application.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Behaviors;

/// <summary>
/// Runs after <see cref="AuthorizationBehavior{TRequest,TResponse}"/> for
/// commands over per-user-owned case resources (private canvas views). The
/// role check already confirmed the caller <em>may</em> perform this kind of
/// action in the case; this adds: if the resource is private and owned by
/// someone else, allow it only when the caller holds the case-wide
/// <see cref="Permission.ViewManageSharing"/> override (e.g. the case Lead).
/// </summary>
public sealed class OwnershipBehavior<TRequest, TResponse>(ICaseResourceOwnership ownership)
    : IPipelineBehavior<TRequest, TResponse>
    where TRequest : notnull {
    private readonly ICaseResourceOwnership ownership = ownership;

    public async Task<TResponse> Handle(
        TRequest request, RequestHandlerDelegate<TResponse> next, CancellationToken cancellationToken) {
        if (request is IOwnedCaseResource resource) {
            await ownership.EnsureCanMutateAsync(
                resource.ResourceOwnerId, resource.IsShared, resource.CaseId, cancellationToken);
        }

        return await next();
    }
}
