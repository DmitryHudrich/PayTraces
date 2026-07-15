namespace Ledgerscope.Accounts.Domain.Authorization;

/// <summary>
/// Whether a role is granted organization-wide (applies to every request the
/// user makes) or only within a specific case (applies when the request
/// targets that case id).
/// </summary>
public enum RoleScope {
    Global,
    Case,
}
