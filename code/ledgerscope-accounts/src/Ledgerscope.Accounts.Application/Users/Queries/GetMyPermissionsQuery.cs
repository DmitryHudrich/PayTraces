using Ledgerscope.Accounts.Application.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Users.Queries;

/// <summary>
/// Returns the effective permission names for the current caller, optionally
/// within a case. Useful for the frontend to enable/disable UI, and it
/// dogfoods the whole authorization stack end-to-end.
/// </summary>
public sealed record GetMyPermissionsQuery(Guid? CaseId) : IRequest<IReadOnlyCollection<String>>;

public sealed class GetMyPermissionsQueryHandler(IUserContext user, IPermissionResolver permissions)
        : IRequestHandler<GetMyPermissionsQuery, IReadOnlyCollection<String>> {
    private readonly IUserContext user = user;
    private readonly IPermissionResolver permissions = permissions;

    public async Task<IReadOnlyCollection<String>> Handle(
        GetMyPermissionsQuery request, CancellationToken cancellationToken) {
        var effective = await permissions.GetEffectivePermissionsAsync(
            user.UserId, request.CaseId, cancellationToken);

        return [.. effective.Select(permission => permission.ToString()).OrderBy(name => name)];
    }
}
