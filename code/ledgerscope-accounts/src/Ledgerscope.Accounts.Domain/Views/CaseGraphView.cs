namespace Ledgerscope.Accounts.Domain.Views;

/// <summary>
/// A named canvas arrangement over a case's graph. A case can hold several
/// views (different arrangements for different audiences). A view is private
/// to its creator until shared; the pinned node positions it owns are the only
/// canvas state Ledgerscope persists — the graph itself always comes live from
/// the Rust engine.
/// </summary>
public sealed class CaseGraphView {
    private readonly List<CaseGraphNodePosition> positions = [];

    private CaseGraphView() {
    }

    public CaseGraphView(Guid id, Guid caseId, String name, Guid createdBy, DateTimeOffset createdAt) {
        Id = id;
        CaseId = caseId;
        Name = name;
        CreatedBy = createdBy;
        CreatedAt = createdAt;
        IsShared = false;
    }

    public Guid Id { get; private set; }
    public Guid CaseId { get; private set; }
    public String Name { get; private set; } = String.Empty;
    public Guid CreatedBy { get; private set; }
    public DateTimeOffset CreatedAt { get; private set; }
    public Boolean IsShared { get; private set; }

    public IReadOnlyCollection<CaseGraphNodePosition> Positions => positions;

    public void Rename(String name) {
        Name = name;
    }

    public void SetShared(Boolean shared) {
        IsShared = shared;
    }

    public CaseGraphNodePosition PinNode(
        String address, Double x, Double y, Guid pinnedBy, DateTimeOffset when) {
        var existing = positions.FirstOrDefault(position => position.Address == address);
        if (existing is not null) {
            existing.MoveTo(x, y, pinnedBy, when);
            return existing;
        }

        var created = new CaseGraphNodePosition(Guid.NewGuid(), Id, address, x, y, pinnedBy, when);
        positions.Add(created);
        return created;
    }

    public void UnpinNode(String address) {
        positions.RemoveAll(position => position.Address == address);
    }
}
