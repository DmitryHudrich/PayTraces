using Ledgerscope.Accounts.Domain.Views;

namespace Ledgerscope.Accounts.Application.Views;

/// <summary>
/// Persistence boundary for the <see cref="CaseGraphView"/> aggregate (a view
/// plus its pinned node positions). EF Core is the unit of work behind
/// <see cref="SaveChangesAsync"/>.
/// </summary>
public interface ICaseGraphViewRepository {
    Task<CaseGraphView?> GetByIdAsync(Guid id, CancellationToken ct);

    Task AddAsync(CaseGraphView entity, CancellationToken ct);

    Task<IReadOnlyList<CaseGraphView>> ListByCaseAsync(Guid caseId, CancellationToken ct);

    void Remove(CaseGraphView entity);

    Task SaveChangesAsync(CancellationToken ct);
}
