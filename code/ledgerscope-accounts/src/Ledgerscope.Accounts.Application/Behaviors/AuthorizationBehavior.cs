using Ledgerscope.Accounts.Application.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Behaviors;

/// <summary>
/// Enforces <see cref="IRequirePermission"/> on every request that declares
/// it: resolves the caller's effective permissions (global ∪ case-scoped)
/// and rejects the request before the handler runs if the required
/// permission is absent. Requests that don't implement the marker pass
/// straight through.
/// </summary>
public sealed class AuthorizationBehavior<TRequest, TResponse>(IUserContext user, IPermissionResolver permissions)
    : IPipelineBehavior<TRequest, TResponse>
    where TRequest : notnull {
    private readonly IUserContext user = user;
    private readonly IPermissionResolver permissions = permissions;

    public async Task<TResponse> Handle(
        TRequest request, RequestHandlerDelegate<TResponse> next, CancellationToken cancellationToken) {
        if (request is IRequirePermission requirement) {
            var granted = await permissions.HasAsync(
                user.UserId, requirement.Required, requirement.CaseId, cancellationToken);

            if (!granted) {
                throw ForbiddenException.MissingPermission(requirement.Required, requirement.CaseId);
            }
        }

        return await next();
    }
}
