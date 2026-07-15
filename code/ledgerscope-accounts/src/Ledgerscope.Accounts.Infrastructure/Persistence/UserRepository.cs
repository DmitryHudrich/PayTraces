using Ledgerscope.Accounts.Application.Users;
using Ledgerscope.Accounts.Domain.Identity;
using Ledgerscope.Accounts.Domain.Users;
using Microsoft.EntityFrameworkCore;

namespace Ledgerscope.Accounts.Infrastructure.Persistence;

public sealed class UserRepository(AccountsDbContext db) : IUserRepository {
    private readonly AccountsDbContext db = db;

    public async Task<User?> GetByIdAsync(Guid id, CancellationToken ct) {
        return await db.Users.FirstOrDefaultAsync(user => user.Id == id, ct);
    }

    public async Task<User?> GetByEmailAsync(String email, CancellationToken ct) {
        return await db.Users.FirstOrDefaultAsync(user => user.Email == email, ct);
    }

    public async Task<Boolean> EmailExistsAsync(String email, CancellationToken ct) {
        return await db.Users.AnyAsync(user => user.Email == email, ct);
    }

    public async Task AddAsync(User user, CancellationToken ct) {
        await db.Users.AddAsync(user, ct);
    }

    public async Task AddGlobalRoleAsync(UserRoleAssignment assignment, CancellationToken ct) {
        await db.UserRoleAssignments.AddAsync(assignment, ct);
    }

    public Task SaveChangesAsync(CancellationToken ct) {
        return db.SaveChangesAsync(ct);
    }
}
