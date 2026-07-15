using Ledgerscope.Accounts.Application.Views;
using Ledgerscope.Accounts.Domain.Views;
using Microsoft.EntityFrameworkCore;

namespace Ledgerscope.Accounts.Infrastructure.Persistence;

public sealed class CaseGraphViewRepository(AccountsDbContext db) : ICaseGraphViewRepository {
    private readonly AccountsDbContext db = db;

    public async Task<CaseGraphView?> GetByIdAsync(Guid id, CancellationToken ct) {
        return await db.GraphViews
            .Include(entity => entity.Positions)
            .FirstOrDefaultAsync(entity => entity.Id == id, ct);
    }

    public async Task AddAsync(CaseGraphView entity, CancellationToken ct) {
        await db.GraphViews.AddAsync(entity, ct);
    }

    public async Task<IReadOnlyList<CaseGraphView>> ListByCaseAsync(Guid caseId, CancellationToken ct) {
        return await db.GraphViews
            .Include(entity => entity.Positions)
            .Where(entity => entity.CaseId == caseId)
            .OrderByDescending(entity => entity.CreatedAt)
            .ToListAsync(ct);
    }

    public void Remove(CaseGraphView entity) {
        db.GraphViews.Remove(entity);
    }

    public Task SaveChangesAsync(CancellationToken ct) {
        return db.SaveChangesAsync(ct);
    }
}
