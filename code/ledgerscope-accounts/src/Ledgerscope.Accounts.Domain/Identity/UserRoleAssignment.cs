namespace Ledgerscope.Accounts.Domain.Identity;

/// <summary>
/// A user's global (organization-wide) role assignment. <see cref="RoleName"/>
/// references a role of scope <c>Global</c> defined in the authorization
/// config file (e.g. "OrgAdmin", "Investigator", "Viewer"). Case-scoped roles
/// are recorded separately as CaseAssignment.
/// </summary>
public sealed class UserRoleAssignment {
    private UserRoleAssignment() {
    }

    public UserRoleAssignment(
        Guid id, Guid userId, String roleName, Guid organizationId, Guid assignedBy, DateTimeOffset assignedAt) {
        Id = id;
        UserId = userId;
        RoleName = roleName;
        OrganizationId = organizationId;
        AssignedBy = assignedBy;
        AssignedAt = assignedAt;
    }

    public Guid Id { get; private set; }
    public Guid UserId { get; private set; }
    public String RoleName { get; private set; } = String.Empty;
    public Guid OrganizationId { get; private set; }
    public Guid AssignedBy { get; private set; }
    public DateTimeOffset AssignedAt { get; private set; }
}
