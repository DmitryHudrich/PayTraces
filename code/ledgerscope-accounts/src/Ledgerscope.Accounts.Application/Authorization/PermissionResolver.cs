using Ledgerscope.Accounts.Domain.Authorization;

namespace Ledgerscope.Accounts.Application.Authorization;

/// <summary>
/// Composes a user's effective permissions from their global role
/// assignments plus, when a case is in scope, their case role assignments —
/// mapping each assigned role name through the file-driven
/// <see cref="IRolePermissionMap"/>. Pure logic: persistence is reached only
/// via <see cref="IRoleAssignmentStore"/>. In Infrastructure a caching
/// decorator wraps this against Redis.
/// </summary>
public sealed class PermissionResolver(IRolePermissionMap map, IRoleAssignmentStore assignments) : IPermissionResolver {
    private readonly IRolePermissionMap map = map;
    private readonly IRoleAssignmentStore assignments = assignments;

    public async Task<IReadOnlySet<Permission>> GetEffectivePermissionsAsync(
        Guid userId, Guid? caseId, CancellationToken ct) {
        var effective = new HashSet<Permission>();

        foreach (var roleName in await assignments.GetGlobalRoleNamesAsync(userId, ct)) {
            effective.UnionWith(map.PermissionsFor(roleName, RoleScope.Global));
        }

        if (caseId is Guid id) {
            foreach (var roleName in await assignments.GetCaseRoleNamesAsync(userId, id, ct)) {
                effective.UnionWith(map.PermissionsFor(roleName, RoleScope.Case));
            }
        }

        return effective;
    }

    public async Task<Boolean> HasAsync(
        Guid userId, Permission permission, Guid? caseId, CancellationToken ct) {
        var effective = await GetEffectivePermissionsAsync(userId, caseId, ct);
        return effective.Contains(permission);
    }
}
