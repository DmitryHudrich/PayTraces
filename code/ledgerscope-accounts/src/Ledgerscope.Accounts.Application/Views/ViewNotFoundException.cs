namespace Ledgerscope.Accounts.Application.Views;

/// <summary>
/// Thrown when a command/query targets a graph-view id that does not exist (or
/// exists under a different case than the request's). The API layer maps this
/// to 404.
/// </summary>
public sealed class ViewNotFoundException(Guid viewId) : Exception($"Graph view {viewId} was not found.") {
    public Guid ViewId { get; } = viewId;
}
