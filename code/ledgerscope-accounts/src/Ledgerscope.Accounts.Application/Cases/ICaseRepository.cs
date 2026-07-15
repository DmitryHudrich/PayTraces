using Ledgerscope.Accounts.Domain.Cases;

namespace Ledgerscope.Accounts.Application.Cases;

/// <summary>
/// Persistence boundary for the Case aggregate. Implemented in Infrastructure
/// over EF Core; the DbContext is the unit of work behind
/// <see cref="SaveChangesAsync"/>.
/// </summary>
public interface ICaseRepository {
    Task<Case?> GetByIdAsync(Guid id, CancellationToken ct);

    Task AddAsync(Case entity, CancellationToken ct);

    Task<IReadOnlyList<Case>> ListByOrganizationAsync(Guid organizationId, CancellationToken ct);

    Task SaveChangesAsync(CancellationToken ct);
}
