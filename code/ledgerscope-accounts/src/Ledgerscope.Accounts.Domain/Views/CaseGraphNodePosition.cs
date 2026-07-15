namespace Ledgerscope.Accounts.Domain.Views;

/// <summary>
/// A single manually-pinned node inside a <see cref="CaseGraphView"/>. Only
/// pinned nodes are persisted — everything else auto-lays-out on the client;
/// these coordinates are the constraints the client's force layout honours.
/// </summary>
public sealed class CaseGraphNodePosition {
    private CaseGraphNodePosition() {
    }

    public CaseGraphNodePosition(
        Guid id, Guid viewId, String address, Double x, Double y,
        Guid pinnedBy, DateTimeOffset pinnedAt) {
        Id = id;
        ViewId = viewId;
        Address = address;
        X = x;
        Y = y;
        PinnedBy = pinnedBy;
        PinnedAt = pinnedAt;
    }

    public Guid Id { get; private set; }
    public Guid ViewId { get; private set; }
    public String Address { get; private set; } = String.Empty;
    public Double X { get; private set; }
    public Double Y { get; private set; }
    public Guid PinnedBy { get; private set; }
    public DateTimeOffset PinnedAt { get; private set; }

    public void MoveTo(Double x, Double y, Guid pinnedBy, DateTimeOffset when) {
        X = x;
        Y = y;
        PinnedBy = pinnedBy;
        PinnedAt = when;
    }
}
