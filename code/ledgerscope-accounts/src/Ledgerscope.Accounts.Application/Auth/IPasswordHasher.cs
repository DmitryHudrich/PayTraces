namespace Ledgerscope.Accounts.Application.Auth;

/// <summary>
/// Hashes and verifies user passwords. Implemented in Infrastructure over
/// ASP.NET Core Identity's password hasher.
/// </summary>
public interface IPasswordHasher {
    String Hash(String password);

    Boolean Verify(String passwordHash, String providedPassword);
}
