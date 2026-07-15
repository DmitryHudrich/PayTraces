namespace Ledgerscope.Accounts.Domain.Cases;

/// <summary>
/// A free-text note on a case — part of the investigation timeline.
/// </summary>
public sealed class CaseNote {
    private CaseNote() {
    }

    public CaseNote(Guid id, Guid caseId, Guid authorId, String text, DateTimeOffset createdAt) {
        Id = id;
        CaseId = caseId;
        AuthorId = authorId;
        Text = text;
        CreatedAt = createdAt;
    }

    public Guid Id { get; private set; }
    public Guid CaseId { get; private set; }
    public Guid AuthorId { get; private set; }
    public String Text { get; private set; } = String.Empty;
    public DateTimeOffset CreatedAt { get; private set; }
}
