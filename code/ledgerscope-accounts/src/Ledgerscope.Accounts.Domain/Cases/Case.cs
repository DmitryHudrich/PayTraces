namespace Ledgerscope.Accounts.Domain.Cases;

/// <summary>
/// Investigation aggregate: a container of the addresses, assignments, and
/// notes that make up one case. The graph, risk score, and tracing always
/// live in the Rust engine — a case only records which addresses belong to
/// the investigation plus the metadata around them.
/// </summary>
public sealed class Case {
    private readonly List<CaseAddress> addresses = [];
    private readonly List<CaseAssignment> assignments = [];
    private readonly List<CaseNote> notes = [];

    private Case() {
    }

    public Case(
        Guid id, String title, String description, CasePriority priority,
        Guid organizationId, Guid createdBy, DateTimeOffset createdAt) {
        Id = id;
        Title = title;
        Description = description;
        Priority = priority;
        OrganizationId = organizationId;
        CreatedBy = createdBy;
        CreatedAt = createdAt;
        Status = CaseStatus.Open;
    }

    public Guid Id { get; private set; }
    public String Title { get; private set; } = String.Empty;
    public String Description { get; private set; } = String.Empty;
    public CaseStatus Status { get; private set; }
    public CasePriority Priority { get; private set; }
    public Guid OrganizationId { get; private set; }
    public Guid CreatedBy { get; private set; }
    public DateTimeOffset CreatedAt { get; private set; }
    public DateTimeOffset? ClosedAt { get; private set; }

    public IReadOnlyCollection<CaseAddress> Addresses => addresses;
    public IReadOnlyCollection<CaseAssignment> Assignments => assignments;
    public IReadOnlyCollection<CaseNote> Notes => notes;

    public void UpdateDetails(String title, String description, CasePriority priority) {
        Title = title;
        Description = description;
        Priority = priority;
    }

    public void Close(DateTimeOffset when) {
        Status = CaseStatus.Closed;
        ClosedAt = when;
    }

    public void Reopen() {
        Status = CaseStatus.Reopened;
        ClosedAt = null;
    }

    public CaseAssignment Assign(Guid userId, String roleName, Guid assignedBy, DateTimeOffset when) {
        var existing = assignments.FirstOrDefault(assignment => assignment.UserId == userId);
        if (existing is not null) {
            existing.ChangeRole(roleName, assignedBy, when);
            return existing;
        }

        var created = new CaseAssignment(Guid.NewGuid(), Id, userId, roleName, assignedBy, when);
        assignments.Add(created);
        return created;
    }

    public void Unassign(Guid userId) {
        assignments.RemoveAll(assignment => assignment.UserId == userId);
    }

    public CaseAddress AddAddress(
        String address, Int32 chainId, Guid addedBy, DateTimeOffset when, String? note) {
        var existing = addresses.FirstOrDefault(
            item => item.Address == address && item.ChainId == chainId);
        if (existing is not null) {
            return existing;
        }

        var created = new CaseAddress(Guid.NewGuid(), Id, address, chainId, addedBy, when, note);
        addresses.Add(created);
        return created;
    }

    public void RemoveAddress(String address, Int32 chainId) {
        addresses.RemoveAll(item => item.Address == address && item.ChainId == chainId);
    }

    public CaseNote AddNote(Guid authorId, String text, DateTimeOffset when) {
        var created = new CaseNote(Guid.NewGuid(), Id, authorId, text, when);
        notes.Add(created);
        return created;
    }
}
