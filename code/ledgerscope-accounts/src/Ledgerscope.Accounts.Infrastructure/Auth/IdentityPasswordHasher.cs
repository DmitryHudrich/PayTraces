using Ledgerscope.Accounts.Application.Auth;
using Ledgerscope.Accounts.Domain.Users;
using Microsoft.AspNetCore.Identity;

namespace Ledgerscope.Accounts.Infrastructure.Auth;

/// <summary>
/// Wraps ASP.NET Core Identity's <see cref="PasswordHasher{TUser}"/> — the
/// battle-tested PBKDF2 implementation — behind the Application's
/// <see cref="IPasswordHasher"/> abstraction.
/// </summary>
public sealed class IdentityPasswordHasher : IPasswordHasher {
    private readonly PasswordHasher<User> hasher = new();

    public String Hash(String password) {
        return hasher.HashPassword(user: default!, password);
    }

    public Boolean Verify(String passwordHash, String providedPassword) {
        var result = hasher.VerifyHashedPassword(user: default!, passwordHash, providedPassword);
        return result is PasswordVerificationResult.Success
            or PasswordVerificationResult.SuccessRehashNeeded;
    }
}
