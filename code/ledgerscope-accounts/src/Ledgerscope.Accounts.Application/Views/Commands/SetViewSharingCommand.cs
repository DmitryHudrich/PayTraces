using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Views.Commands;

public sealed record SetViewSharingCommand(Guid CaseId, Guid ViewId, Boolean IsShared)
    : IRequest, IRequirePermission {
    public Permission Required => Permission.ViewManageSharing;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class SetViewSharingCommandHandler(ICaseGraphViewRepository views, ICaseResourceOwnership ownership) : IRequestHandler<SetViewSharingCommand> {
    private readonly ICaseGraphViewRepository views = views;
    private readonly ICaseResourceOwnership ownership = ownership;

    public async Task Handle(SetViewSharingCommand request, CancellationToken cancellationToken) {
        var view = await views.GetByIdAsync(request.ViewId, cancellationToken);
        if (view is null || view.CaseId != request.CaseId) {
            throw new ViewNotFoundException(request.ViewId);
        }

        await ownership.EnsureCanMutateAsync(view.CreatedBy, view.IsShared, request.CaseId, cancellationToken);

        view.SetShared(request.IsShared);
        await views.SaveChangesAsync(cancellationToken);
    }
}
