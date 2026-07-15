namespace Ledgerscope.Accounts.Domain.Cases;

/// <summary>
/// An address attached to a case — the bridge into the Rust engine's graph.
/// Ledgerscope stores only the (address, chain) reference and its
/// investigation metadata; the graph/score itself always comes live from the
/// Rust engine.
/// </summary>
public sealed class CaseAddress {
    private CaseAddress() {
    }

    public CaseAddress(
        Guid id, Guid caseId, String address, Int32 chainId,
        Guid addedBy, DateTimeOffset addedAt, String? note) {
        Id = id;
        CaseId = caseId;
        Address = address;
        ChainId = chainId;
        AddedBy = addedBy;
        AddedAt = addedAt;
        Note = note;
    }

    public Guid Id { get; private set; }
    public Guid CaseId { get; private set; }
    public String Address { get; private set; } = String.Empty;
    public Int32 ChainId { get; private set; }
    public Guid AddedBy { get; private set; }
    public DateTimeOffset AddedAt { get; private set; }
    public String? Note { get; private set; }
}
