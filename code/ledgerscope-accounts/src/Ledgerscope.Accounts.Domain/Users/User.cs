namespace Ledgerscope.Accounts.Domain.Users;

/// <summary>
/// A user of the platform. Kept intentionally minimal for now; when ASP.NET
/// Identity is wired in, the credential/login concerns live in the Identity
/// store and this stays the domain-side profile keyed by the same id.
/// </summary>
public sealed class User {
    private User() {
    }

    public User(Guid id, String email, String displayName, Guid organizationId, DateTimeOffset createdAt) {
        Id = id;
        Email = email;
        DisplayName = displayName;
        OrganizationId = organizationId;
        CreatedAt = createdAt;
    }

    public Guid Id { get; private set; }
    public String Email { get; private set; } = String.Empty;
    public String DisplayName { get; private set; } = String.Empty;
    public Guid OrganizationId { get; private set; }
    public DateTimeOffset CreatedAt { get; private set; }
    public Boolean IsActive { get; private set; } = true;

    /// <summary>
    /// Hashed credential. Null for users provisioned without a local password
    /// (e.g. seeded/service accounts). The hash format is owned by the
    /// Infrastructure password hasher.
    /// </summary>
    public String? PasswordHash { get; private set; }

    public void SetPasswordHash(String passwordHash) {
        PasswordHash = passwordHash;
    }

    public void Deactivate() {
        IsActive = false;
    }
}
