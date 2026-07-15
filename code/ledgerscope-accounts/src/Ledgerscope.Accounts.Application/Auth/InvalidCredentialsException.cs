namespace Ledgerscope.Accounts.Application.Auth;

/// <summary>
/// Thrown when login fails (unknown email, wrong password, or inactive user).
/// Deliberately non-specific so it doesn't reveal which part failed. The API
/// layer maps this to 401.
/// </summary>
public sealed class InvalidCredentialsException : Exception {
    public InvalidCredentialsException() : base("Invalid email or password.") {
    }
}
