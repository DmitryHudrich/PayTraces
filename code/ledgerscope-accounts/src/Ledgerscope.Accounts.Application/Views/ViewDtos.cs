using Ledgerscope.Accounts.Domain.Views;

namespace Ledgerscope.Accounts.Application.Views;

public sealed record CaseGraphViewSummaryDto(
    Guid Id,
    Guid CaseId,
    String Name,
    Guid CreatedBy,
    Boolean IsShared,
    DateTimeOffset CreatedAt,
    Int32 PinnedCount);

public sealed record CaseGraphViewDto(
    Guid Id,
    Guid CaseId,
    String Name,
    Guid CreatedBy,
    Boolean IsShared,
    DateTimeOffset CreatedAt,
    IReadOnlyList<NodePositionDto> Positions);

public sealed record NodePositionDto(String Address, Double X, Double Y);

public static class CaseGraphViewMappings {
    public static CaseGraphViewSummaryDto ToSummary(this CaseGraphView source) {
        return new(source.Id, source.CaseId, source.Name, source.CreatedBy,
            source.IsShared, source.CreatedAt, source.Positions.Count);
    }

    public static CaseGraphViewDto ToDto(this CaseGraphView source) {
        return new(
            source.Id,
            source.CaseId,
            source.Name,
            source.CreatedBy,
            source.IsShared,
            source.CreatedAt,
            [.. source.Positions.Select(p => new NodePositionDto(p.Address, p.X, p.Y))]);
    }
}
