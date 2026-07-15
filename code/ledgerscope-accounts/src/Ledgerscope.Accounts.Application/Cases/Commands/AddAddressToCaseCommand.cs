using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Cases.Commands;

public sealed record AddAddressToCaseCommand(Guid CaseId, String Address, Int32 ChainId, String? Note)
    : IRequest, IRequirePermission {
    public Permission Required => Permission.CaseAddressAdd;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class AddAddressToCaseCommandHandler(ICaseRepository cases, IUserContext user) : IRequestHandler<AddAddressToCaseCommand> {
    private readonly ICaseRepository cases = cases;
    private readonly IUserContext user = user;

    public async Task Handle(AddAddressToCaseCommand request, CancellationToken cancellationToken) {
        var entity = await cases.GetByIdAsync(request.CaseId, cancellationToken)
            ?? throw new CaseNotFoundException(request.CaseId);

        _ = entity.AddAddress(request.Address, request.ChainId, user.UserId, DateTimeOffset.UtcNow, request.Note);
        await cases.SaveChangesAsync(cancellationToken);
    }
}
