namespace Ledgerscope.Accounts.Domain.Labels;

/// <summary>
/// Applies a <see cref="CustomLabel"/> to a specific address on a chain. The
/// address is a value reference into the Rust engine's graph — no FK, just the
/// (address, chain) pair — so links survive independently of whether the
/// engine has ingested that address yet.
/// </summary>
public sealed class AddressLabelLink {
    private AddressLabelLink() {
    }

    public AddressLabelLink(
        Guid id, Guid labelId, String address, Int32 chainId, Guid appliedBy, DateTimeOffset appliedAt) {
        Id = id;
        LabelId = labelId;
        Address = address;
        ChainId = chainId;
        AppliedBy = appliedBy;
        AppliedAt = appliedAt;
    }

    public Guid Id { get; private set; }
    public Guid LabelId { get; private set; }
    public String Address { get; private set; } = String.Empty;
    public Int32 ChainId { get; private set; }
    public Guid AppliedBy { get; private set; }
    public DateTimeOffset AppliedAt { get; private set; }
}
