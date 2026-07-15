using Ledgerscope.Accounts.Domain.Groups;

namespace Ledgerscope.Accounts.Application.Groups;

/// <summary>
/// Persistence boundary for the <see cref="AddressGroup"/> aggregate (a group
/// plus its member addresses). EF Core is the unit of work behind
/// <see cref="SaveChangesAsync"/>.
/// </summary>
public interface IAddressGroupRepository {
    Task<AddressGroup?> GetByIdAsync(Guid id, CancellationToken ct);

    Task AddAsync(AddressGroup entity, CancellationToken ct);

    Task<IReadOnlyList<AddressGroup>> ListByCaseAsync(Guid caseId, CancellationToken ct);

    void Remove(AddressGroup entity);

    Task SaveChangesAsync(CancellationToken ct);
}
