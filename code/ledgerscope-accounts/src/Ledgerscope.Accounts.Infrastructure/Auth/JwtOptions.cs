namespace Ledgerscope.Accounts.Infrastructure.Auth;

/// <summary>
/// Bound from the "Jwt" config section. Symmetric (HS256) signing is fine
/// here because the only validator is Ledgerscope.Accounts itself — the Rust
/// engine no longer consumes these tokens (it trusts the shared secret / mTLS
/// boundary instead).
/// </summary>
public sealed class JwtOptions {
    public const String SectionName = "Jwt";

    public String Issuer { get; set; } = String.Empty;
    public String Audience { get; set; } = String.Empty;
    public String SigningKey { get; set; } = String.Empty;
    public Int32 ExpiryMinutes { get; set; } = 60;
}
