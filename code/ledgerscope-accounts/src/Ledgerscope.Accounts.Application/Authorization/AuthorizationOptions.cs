using Ledgerscope.Accounts.Domain.Authorization;

namespace Ledgerscope.Accounts.Application.Authorization;

/// <summary>
/// Bound from the "Authorization" config section (see authorization.json).
/// This is the file-driven "which role grants what" policy — it can change
/// without a redeploy. Only the role → permissions mapping lives here; the
/// vocabulary of possible permissions (<see cref="Permission"/>) and which
/// permission each command requires stay in code.
/// </summary>
public sealed class AuthorizationOptions {
    public const String SectionName = "Authorization";

    public Dictionary<String, RoleDefinition> Roles { get; set; } =
        new(StringComparer.OrdinalIgnoreCase);
}

public sealed class RoleDefinition {
    public RoleScope Scope { get; set; }

    /// <summary>
    /// Permission names, or the single wildcard <c>"*"</c> to grant every
    /// permission. Bound as strings so <c>"*"</c> is expressible and so an
    /// unknown name can fail loudly rather than bind to nothing.
    /// </summary>
    public List<String> Permissions { get; set; } = [];
}
