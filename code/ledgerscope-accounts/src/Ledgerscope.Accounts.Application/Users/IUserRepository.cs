using Ledgerscope.Accounts.Domain.Identity;
using Ledgerscope.Accounts.Domain.Users;

namespace Ledgerscope.Accounts.Application.Users;

/// <summary>
/// Persistence boundary for users and their global role assignments.
/// Implemented in Infrastructure over EF Core.
/// </summary>
public interface IUserRepository {
    Task<User?> GetByIdAsync(Guid id, CancellationToken ct);

    Task<User?> GetByEmailAsync(String email, CancellationToken ct);

    Task<Boolean> EmailExistsAsync(String email, CancellationToken ct);

    Task AddAsync(User user, CancellationToken ct);

    Task AddGlobalRoleAsync(UserRoleAssignment assignment, CancellationToken ct);

    Task SaveChangesAsync(CancellationToken ct);
}
