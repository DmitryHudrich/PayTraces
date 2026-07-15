using Ledgerscope.Accounts.Application.Cases;
using Ledgerscope.Accounts.Domain.Cases;
using Microsoft.EntityFrameworkCore;

namespace Ledgerscope.Accounts.Infrastructure.Persistence;

public sealed class CaseRepository(AccountsDbContext db) : ICaseRepository {
    private readonly AccountsDbContext db = db;

    public async Task<Case?> GetByIdAsync(Guid id, CancellationToken ct) {
        return await db.Cases
            .Include(entity => entity.Addresses)
            .Include(entity => entity.Assignments)
            .Include(entity => entity.Notes)
            .FirstOrDefaultAsync(entity => entity.Id == id, ct);
    }

    public async Task AddAsync(Case entity, CancellationToken ct) {
        await db.Cases.AddAsync(entity, ct);
    }

    public async Task<IReadOnlyList<Case>> ListByOrganizationAsync(Guid organizationId, CancellationToken ct) {
        return await db.Cases
            .Where(entity => entity.OrganizationId == organizationId)
            .OrderByDescending(entity => entity.CreatedAt)
            .ToListAsync(ct);
    }

    public Task SaveChangesAsync(CancellationToken ct) {
        return db.SaveChangesAsync(ct);
    }
}
