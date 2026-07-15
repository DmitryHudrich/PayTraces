using Ledgerscope.Accounts.Domain.Users;

namespace Ledgerscope.Accounts.Application.Auth;

public sealed record AccessToken(String Token, DateTimeOffset ExpiresAt);

/// <summary>
/// Issues the JWT the frontend presents to Ledgerscope.Accounts. The token
/// carries identity only (subject/email/org) — authorization is resolved
/// server-side from role assignments, never trusted from token claims.
/// </summary>
public interface IJwtIssuer {
    AccessToken Issue(User user);
}
