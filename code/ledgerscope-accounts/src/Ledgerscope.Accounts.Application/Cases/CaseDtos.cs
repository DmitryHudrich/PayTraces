using Ledgerscope.Accounts.Domain.Cases;

namespace Ledgerscope.Accounts.Application.Cases;

public sealed record CaseSummaryDto(
    Guid Id,
    String Title,
    CaseStatus Status,
    CasePriority Priority,
    Guid OrganizationId,
    DateTimeOffset CreatedAt);

public sealed record CaseDto(
    Guid Id,
    String Title,
    String Description,
    CaseStatus Status,
    CasePriority Priority,
    Guid OrganizationId,
    Guid CreatedBy,
    DateTimeOffset CreatedAt,
    DateTimeOffset? ClosedAt,
    IReadOnlyList<CaseAddressDto> Addresses,
    IReadOnlyList<CaseAssignmentDto> Assignments,
    IReadOnlyList<CaseNoteDto> Notes);

public sealed record CaseAddressDto(String Address, Int32 ChainId, Guid AddedBy, DateTimeOffset AddedAt, String? Note);

public sealed record CaseAssignmentDto(Guid UserId, String RoleName, DateTimeOffset AssignedAt);

public sealed record CaseNoteDto(Guid Id, Guid AuthorId, String Text, DateTimeOffset CreatedAt);

public static class CaseMappings {
    public static CaseSummaryDto ToSummary(this Case source) {
        return new(source.Id, source.Title, source.Status, source.Priority, source.OrganizationId, source.CreatedAt);
    }

    public static CaseDto ToDto(this Case source) {
        return new(
            source.Id,
            source.Title,
            source.Description,
            source.Status,
            source.Priority,
            source.OrganizationId,
            source.CreatedBy,
            source.CreatedAt,
            source.ClosedAt,
            [.. source.Addresses.Select(a => new CaseAddressDto(a.Address, a.ChainId, a.AddedBy, a.AddedAt, a.Note))],
            [.. source.Assignments.Select(a => new CaseAssignmentDto(a.UserId, a.RoleName, a.AssignedAt))],
            [.. source.Notes.Select(n => new CaseNoteDto(n.Id, n.AuthorId, n.Text, n.CreatedAt))]);
    }
}
