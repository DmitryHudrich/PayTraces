namespace Ledgerscope.Accounts.Application.Labels;

/// <summary>
/// Thrown when a command/query targets a label id that does not exist under the
/// request's case. The API layer maps this to 404.
/// </summary>
public sealed class LabelNotFoundException(Guid labelId) : Exception($"Label {labelId} was not found.") {
    public Guid LabelId { get; } = labelId;
}
