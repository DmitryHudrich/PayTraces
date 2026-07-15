using Ledgerscope.Accounts.Domain.Groups;

namespace Ledgerscope.Accounts.Application.Groups;

public sealed record AddressGroupSummaryDto(
    Guid Id, Guid CaseId, String Name, Guid CreatedBy, DateTimeOffset CreatedAt, Int32 MemberCount);

public sealed record AddressGroupDto(
    Guid Id, Guid CaseId, String Name, Guid CreatedBy, DateTimeOffset CreatedAt,
    IReadOnlyList<GroupMemberDto> Members);

public sealed record GroupMemberDto(String Address, Int32 ChainId, DateTimeOffset AddedAt);

public static class GroupMappings {
    public static AddressGroupSummaryDto ToSummary(this AddressGroup source) {
        return new(source.Id, source.CaseId, source.Name, source.CreatedBy,
            source.CreatedAt, source.Members.Count);
    }

    public static AddressGroupDto ToDto(this AddressGroup source) {
        return new(
            source.Id, source.CaseId, source.Name, source.CreatedBy, source.CreatedAt,
            [.. source.Members.Select(m => new GroupMemberDto(m.Address, m.ChainId, m.AddedAt))]);
    }
}
