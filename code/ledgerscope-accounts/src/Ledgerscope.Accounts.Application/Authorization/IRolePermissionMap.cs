using Ledgerscope.Accounts.Domain.Authorization;

namespace Ledgerscope.Accounts.Application.Authorization;

/// <summary>
/// Precomputed, validated view over <see cref="AuthorizationOptions"/>:
/// resolves a configured role name to its concrete permission set, expanding
/// the <c>"*"</c> wildcard and rejecting unknown permission names. Reflects
/// the current config, which may reload when the file changes.
/// </summary>
public interface IRolePermissionMap {
    IReadOnlySet<Permission> PermissionsFor(String roleName, RoleScope scope);

    Boolean RoleExists(String roleName);
}
