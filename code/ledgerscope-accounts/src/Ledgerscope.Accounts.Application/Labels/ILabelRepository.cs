using Ledgerscope.Accounts.Domain.Labels;

namespace Ledgerscope.Accounts.Application.Labels;

/// <summary>
/// Read model for a label as applied to an address (the label joined with its
/// application link), so the API can answer "what labels are on this address".
/// </summary>
public sealed record AppliedLabel(Guid LabelId, String Text, String? Color, DateTimeOffset AppliedAt);

/// <summary>
/// Persistence boundary for custom labels and their address applications. EF
/// Core is the unit of work behind <see cref="SaveChangesAsync"/>.
/// </summary>
public interface ILabelRepository {
    Task<CustomLabel?> GetLabelAsync(Guid id, CancellationToken ct);

    Task AddLabelAsync(CustomLabel entity, CancellationToken ct);

    void RemoveLabel(CustomLabel entity);

    Task<IReadOnlyList<CustomLabel>> ListLabelsByCaseAsync(Guid caseId, CancellationToken ct);

    Task<AddressLabelLink?> GetLinkAsync(Guid labelId, String address, Int32 chainId, CancellationToken ct);

    Task AddLinkAsync(AddressLabelLink entity, CancellationToken ct);

    void RemoveLink(AddressLabelLink entity);

    Task<IReadOnlyList<AppliedLabel>> ListAppliedForAddressAsync(
        Guid caseId, String address, Int32 chainId, CancellationToken ct);

    Task SaveChangesAsync(CancellationToken ct);
}
