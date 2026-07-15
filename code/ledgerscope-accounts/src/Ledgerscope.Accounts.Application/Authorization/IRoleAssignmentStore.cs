namespace Ledgerscope.Accounts.Application.Authorization;

/// <summary>
/// Reads a user's <em>role assignments</em> — the only RBAC data kept in the
/// database (the role definitions themselves live in config). Implemented in
/// Infrastructure over EF Core.
/// </summary>
public interface IRoleAssignmentStore {
    /// <summary>Global role names granted to the user organization-wide.</summary>
    Task<IReadOnlyCollection<String>> GetGlobalRoleNamesAsync(Guid userId, CancellationToken ct);

    /// <summary>Case-scoped role names the user holds on a specific case (empty if none).</summary>
    Task<IReadOnlyCollection<String>> GetCaseRoleNamesAsync(Guid userId, Guid caseId, CancellationToken ct);
}
