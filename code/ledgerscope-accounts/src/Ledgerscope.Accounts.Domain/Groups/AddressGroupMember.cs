namespace Ledgerscope.Accounts.Domain.Groups;

/// <summary>
/// One address belonging to an <see cref="AddressGroup"/>. Referenced by value
/// (address, chain) into the Rust engine's graph, never by FK.
/// </summary>
public sealed class AddressGroupMember {
    private AddressGroupMember() {
    }

    public AddressGroupMember(Guid id, Guid groupId, String address, Int32 chainId, DateTimeOffset addedAt) {
        Id = id;
        GroupId = groupId;
        Address = address;
        ChainId = chainId;
        AddedAt = addedAt;
    }

    public Guid Id { get; private set; }
    public Guid GroupId { get; private set; }
    public String Address { get; private set; } = String.Empty;
    public Int32 ChainId { get; private set; }
    public DateTimeOffset AddedAt { get; private set; }
}
