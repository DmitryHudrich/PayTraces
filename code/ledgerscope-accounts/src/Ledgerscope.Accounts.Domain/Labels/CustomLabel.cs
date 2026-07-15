namespace Ledgerscope.Accounts.Domain.Labels;

/// <summary>
/// A user-defined label an investigator can attach to addresses. Lives in
/// Ledgerscope, not the Rust engine — the engine owns authoritative/sanctions
/// labelling; these are the investigation's own annotations.
/// <see cref="CaseId"/> is nullable to leave room for personal labels, but for
/// now every label is case-scoped (creation requires a case-role permission).
/// </summary>
public sealed class CustomLabel {
    private CustomLabel() {
    }

    public CustomLabel(
        Guid id, Guid? caseId, Guid createdBy, String text, String? color, DateTimeOffset createdAt) {
        Id = id;
        CaseId = caseId;
        CreatedBy = createdBy;
        Text = text;
        Color = color;
        CreatedAt = createdAt;
    }

    public Guid Id { get; private set; }
    public Guid? CaseId { get; private set; }
    public Guid CreatedBy { get; private set; }
    public String Text { get; private set; } = String.Empty;
    public String? Color { get; private set; }
    public DateTimeOffset CreatedAt { get; private set; }

    public void Update(String text, String? color) {
        Text = text;
        Color = color;
    }
}
