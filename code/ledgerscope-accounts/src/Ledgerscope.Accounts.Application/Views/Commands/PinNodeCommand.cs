using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Views.Commands;

public sealed record PinNodeCommand(Guid CaseId, Guid ViewId, String Address, Double X, Double Y)
    : IRequest, IRequirePermission {
    public Permission Required => Permission.ViewUpdate;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class PinNodeCommandHandler(
    ICaseGraphViewRepository views, ICaseResourceOwnership ownership, IUserContext user) : IRequestHandler<PinNodeCommand> {
    private readonly ICaseGraphViewRepository views = views;
    private readonly ICaseResourceOwnership ownership = ownership;
    private readonly IUserContext user = user;

    public async Task Handle(PinNodeCommand request, CancellationToken cancellationToken) {
        var view = await views.GetByIdAsync(request.ViewId, cancellationToken);
        if (view is null || view.CaseId != request.CaseId) {
            throw new ViewNotFoundException(request.ViewId);
        }

        await ownership.EnsureCanMutateAsync(view.CreatedBy, view.IsShared, request.CaseId, cancellationToken);

        _ = view.PinNode(request.Address, request.X, request.Y, user.UserId, DateTimeOffset.UtcNow);
        await views.SaveChangesAsync(cancellationToken);
    }
}
