namespace Ledgerscope.Accounts.Domain.Cases;

/// <summary>
/// A user's case-scoped role assignment. <see cref="RoleName"/> references a
/// role of scope <c>Case</c> defined in the authorization config file (e.g.
/// "Lead", "Collaborator"). This table is the source of truth for case write
/// access; org-wide read is governed separately by organization membership.
/// </summary>
public sealed class CaseAssignment {
    private CaseAssignment() {
    }

    public CaseAssignment(
        Guid id, Guid caseId, Guid userId, String roleName, Guid assignedBy, DateTimeOffset assignedAt) {
        Id = id;
        CaseId = caseId;
        UserId = userId;
        RoleName = roleName;
        AssignedBy = assignedBy;
        AssignedAt = assignedAt;
    }

    public Guid Id { get; private set; }
    public Guid CaseId { get; private set; }
    public Guid UserId { get; private set; }
    public String RoleName { get; private set; } = String.Empty;
    public Guid AssignedBy { get; private set; }
    public DateTimeOffset AssignedAt { get; private set; }

    public void ChangeRole(String roleName, Guid assignedBy, DateTimeOffset when) {
        RoleName = roleName;
        AssignedBy = assignedBy;
        AssignedAt = when;
    }
}
