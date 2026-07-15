namespace Ledgerscope.Accounts.Application.Groups;

/// <summary>
/// Thrown when a command/query targets an address-group id that does not exist
/// under the request's case. The API layer maps this to 404.
/// </summary>
public sealed class GroupNotFoundException(Guid groupId) : Exception($"Address group {groupId} was not found.") {
    public Guid GroupId { get; } = groupId;
}
