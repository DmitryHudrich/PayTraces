namespace Ledgerscope.Accounts.Application.Users;

/// <summary>
/// Thrown when a command targets a user id that does not exist. The API layer
/// maps this to 404.
/// </summary>
public sealed class UserNotFoundException(Guid userId) : Exception($"User {userId} was not found.") {
    public Guid UserId { get; } = userId;
}
