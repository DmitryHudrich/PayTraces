using Ledgerscope.Accounts.Application.Labels;
using Ledgerscope.Accounts.Domain.Labels;
using Microsoft.EntityFrameworkCore;

namespace Ledgerscope.Accounts.Infrastructure.Persistence;

public sealed class LabelRepository(AccountsDbContext db) : ILabelRepository {
    private readonly AccountsDbContext db = db;

    public async Task<CustomLabel?> GetLabelAsync(Guid id, CancellationToken ct) {
        return await db.CustomLabels.FirstOrDefaultAsync(entity => entity.Id == id, ct);
    }

    public async Task AddLabelAsync(CustomLabel entity, CancellationToken ct) {
        _ = await db.CustomLabels.AddAsync(entity, ct);
    }

    public void RemoveLabel(CustomLabel entity) {
        _ = db.CustomLabels.Remove(entity);
    }

    public async Task<IReadOnlyList<CustomLabel>> ListLabelsByCaseAsync(Guid caseId, CancellationToken ct) {
        return await db.CustomLabels
            .Where(entity => entity.CaseId == caseId)
            .OrderByDescending(entity => entity.CreatedAt)
            .ToListAsync(ct);
    }

    public async Task<AddressLabelLink?> GetLinkAsync(
        Guid labelId, String address, Int32 chainId, CancellationToken ct) {
        return await db.AddressLabelLinks.FirstOrDefaultAsync(
            link => link.LabelId == labelId && link.Address == address && link.ChainId == chainId, ct);
    }

    public async Task AddLinkAsync(AddressLabelLink entity, CancellationToken ct) {
        _ = await db.AddressLabelLinks.AddAsync(entity, ct);
    }

    public void RemoveLink(AddressLabelLink entity) {
        _ = db.AddressLabelLinks.Remove(entity);
    }

    public async Task<IReadOnlyList<AppliedLabel>> ListAppliedForAddressAsync(
        Guid caseId, String address, Int32 chainId, CancellationToken ct) {
        return await (
            from link in db.AddressLabelLinks
            join label in db.CustomLabels on link.LabelId equals label.Id
            where label.CaseId == caseId && link.Address == address && link.ChainId == chainId
            orderby link.AppliedAt descending
            select new AppliedLabel(label.Id, label.Text, label.Color, link.AppliedAt))
            .ToListAsync(ct);
    }

    public Task SaveChangesAsync(CancellationToken ct) {
        return db.SaveChangesAsync(ct);
    }
}
