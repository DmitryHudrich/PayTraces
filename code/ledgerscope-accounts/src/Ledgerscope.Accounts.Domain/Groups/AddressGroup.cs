namespace Ledgerscope.Accounts.Domain.Groups;

/// <summary>
/// A stable, user-curated set of addresses inside a case. Ledgerscope needs its
/// own group entity because the Rust engine's <c>/cluster</c> has no stable
/// cluster id (it is recomputed on every call), so a group — not raw cluster
/// output — is what carries labels and survives across sessions. A group may be
/// seeded from a <c>/cluster</c> result but is thereafter independent.
/// </summary>
public sealed class AddressGroup {
    private readonly List<AddressGroupMember> members = [];

    private AddressGroup() {
    }

    public AddressGroup(Guid id, Guid caseId, Guid createdBy, String name, DateTimeOffset createdAt) {
        Id = id;
        CaseId = caseId;
        CreatedBy = createdBy;
        Name = name;
        CreatedAt = createdAt;
    }

    public Guid Id { get; private set; }
    public Guid CaseId { get; private set; }
    public Guid CreatedBy { get; private set; }
    public String Name { get; private set; } = String.Empty;
    public DateTimeOffset CreatedAt { get; private set; }

    public IReadOnlyCollection<AddressGroupMember> Members => members;

    public void Rename(String name) {
        Name = name;
    }

    public AddressGroupMember AddMember(String address, Int32 chainId, DateTimeOffset when) {
        var existing = members.FirstOrDefault(
            member => member.Address == address && member.ChainId == chainId);
        if (existing is not null) {
            return existing;
        }

        var created = new AddressGroupMember(Guid.NewGuid(), Id, address, chainId, when);
        members.Add(created);
        return created;
    }

    public void RemoveMember(String address, Int32 chainId) {
        _ = members.RemoveAll(member => member.Address == address && member.ChainId == chainId);
    }
}
