namespace Ledgerscope.Accounts.Application.Users;

/// <summary>
/// Thrown when registering with an email that already exists. The API layer
/// maps this to 409.
/// </summary>
public sealed class EmailAlreadyInUseException(String email) : Exception($"Email '{email}' is already in use.") {
}
