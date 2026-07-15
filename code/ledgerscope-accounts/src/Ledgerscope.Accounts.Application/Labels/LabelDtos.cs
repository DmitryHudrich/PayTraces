using Ledgerscope.Accounts.Domain.Labels;

namespace Ledgerscope.Accounts.Application.Labels;

public sealed record CustomLabelDto(
    Guid Id, Guid? CaseId, String Text, String? Color, Guid CreatedBy, DateTimeOffset CreatedAt);

public sealed record AppliedLabelDto(Guid LabelId, String Text, String? Color, DateTimeOffset AppliedAt);

public static class LabelMappings {
    public static CustomLabelDto ToDto(this CustomLabel source) {
        return new(source.Id, source.CaseId, source.Text, source.Color, source.CreatedBy, source.CreatedAt);
    }

    public static AppliedLabelDto ToDto(this AppliedLabel source) {
        return new(source.LabelId, source.Text, source.Color, source.AppliedAt);
    }
}
