using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using Ledgerscope.Accounts.Domain.Groups;
using MediatR;

namespace Ledgerscope.Accounts.Application.Groups.Commands;

public sealed record GroupMemberInput(String Address, Int32 ChainId);

/// <summary>
/// Creates a stable address group. <see cref="Members"/> may seed it (e.g. from
/// a Rust /cluster result the caller chose to persist); it is independent
/// thereafter.
/// </summary>
public sealed record CreateGroupCommand(Guid CaseId, String Name, IReadOnlyList<GroupMemberInput> Members)
    : IRequest<Guid>, IRequirePermission {
    public Permission Required => Permission.GroupCreate;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class CreateGroupCommandHandler(IAddressGroupRepository groups, IUserContext user)
    : IRequestHandler<CreateGroupCommand, Guid> {
    private readonly IAddressGroupRepository groups = groups;
    private readonly IUserContext user = user;

    public async Task<Guid> Handle(CreateGroupCommand request, CancellationToken cancellationToken) {
        var now = DateTimeOffset.UtcNow;
        var entity = new AddressGroup(Guid.NewGuid(), request.CaseId, user.UserId, request.Name, now);
        foreach (var member in request.Members) {
            _ = entity.AddMember(member.Address, member.ChainId, now);
        }

        await groups.AddAsync(entity, cancellationToken);
        await groups.SaveChangesAsync(cancellationToken);
        return entity.Id;
    }
}
