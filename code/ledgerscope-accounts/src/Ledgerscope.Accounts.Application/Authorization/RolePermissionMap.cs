using Ledgerscope.Accounts.Domain.Authorization;
using Microsoft.Extensions.Options;

namespace Ledgerscope.Accounts.Application.Authorization;

public sealed class RolePermissionMap(IOptionsMonitor<AuthorizationOptions> options) : IRolePermissionMap {
    private static readonly IReadOnlySet<Permission> Empty = new HashSet<Permission>();

    private static readonly IReadOnlySet<Permission> AllPermissions =
        new HashSet<Permission>(Enum.GetValues<Permission>());

    private readonly IOptionsMonitor<AuthorizationOptions> options = options;

    public Boolean RoleExists(String roleName) {
        return options.CurrentValue.Roles.ContainsKey(roleName);
    }

    public IReadOnlySet<Permission> PermissionsFor(String roleName, RoleScope scope) {
        var roles = options.CurrentValue.Roles;
        return !roles.TryGetValue(roleName, out var definition) || definition.Scope != scope ? Empty : Resolve(roleName, definition);
    }

    private static IReadOnlySet<Permission> Resolve(String roleName, RoleDefinition definition) {
        if (definition.Permissions.Contains("*")) {
            return AllPermissions;
        }

        var set = new HashSet<Permission>();
        foreach (var name in definition.Permissions) {
            if (!Enum.TryParse<Permission>(name, ignoreCase: true, out var permission)) {
                throw new InvalidOperationException(
                    $"Authorization config: role '{roleName}' references unknown permission '{name}'.");
            }

            _ = set.Add(permission);
        }

        return set;
    }
}
