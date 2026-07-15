using Ledgerscope.Accounts.Application.Groups;
using Ledgerscope.Accounts.Domain.Groups;
using Microsoft.EntityFrameworkCore;

namespace Ledgerscope.Accounts.Infrastructure.Persistence;

public sealed class AddressGroupRepository(AccountsDbContext db) : IAddressGroupRepository {
    private readonly AccountsDbContext db = db;

    public async Task<AddressGroup?> GetByIdAsync(Guid id, CancellationToken ct) {
        return await db.AddressGroups
            .Include(entity => entity.Members)
            .FirstOrDefaultAsync(entity => entity.Id == id, ct);
    }

    public async Task AddAsync(AddressGroup entity, CancellationToken ct) {
        _ = await db.AddressGroups.AddAsync(entity, ct);
    }

    public async Task<IReadOnlyList<AddressGroup>> ListByCaseAsync(Guid caseId, CancellationToken ct) {
        return await db.AddressGroups
            .Include(entity => entity.Members)
            .Where(entity => entity.CaseId == caseId)
            .OrderByDescending(entity => entity.CreatedAt)
            .ToListAsync(ct);
    }

    public void Remove(AddressGroup entity) {
        _ = db.AddressGroups.Remove(entity);
    }

    public Task SaveChangesAsync(CancellationToken ct) {
        return db.SaveChangesAsync(ct);
    }
}
