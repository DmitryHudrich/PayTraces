using Ledgerscope.Accounts.Domain.Authorization;

namespace Ledgerscope.Accounts.Application.Authorization;

/// <summary>
/// Resolves a user's effective permissions by combining their global and
/// (optionally) case-scoped role assignments through the file-driven role
/// map. Infrastructure wraps this with a Redis caching decorator.
/// </summary>
public interface IPermissionResolver {
    Task<IReadOnlySet<Permission>> GetEffectivePermissionsAsync(
        Guid userId, Guid? caseId, CancellationToken ct);

    Task<Boolean> HasAsync(Guid userId, Permission permission, Guid? caseId, CancellationToken ct);
}
