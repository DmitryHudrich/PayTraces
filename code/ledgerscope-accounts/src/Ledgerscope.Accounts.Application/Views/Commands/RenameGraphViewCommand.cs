using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Views.Commands;

public sealed record RenameGraphViewCommand(Guid CaseId, Guid ViewId, String Name)
    : IRequest, IRequirePermission {
    public Permission Required => Permission.ViewUpdate;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class RenameGraphViewCommandHandler(ICaseGraphViewRepository views, ICaseResourceOwnership ownership) : IRequestHandler<RenameGraphViewCommand> {
    private readonly ICaseGraphViewRepository views = views;
    private readonly ICaseResourceOwnership ownership = ownership;

    public async Task Handle(RenameGraphViewCommand request, CancellationToken cancellationToken) {
        var view = await views.GetByIdAsync(request.ViewId, cancellationToken);
        if (view is null || view.CaseId != request.CaseId) {
            throw new ViewNotFoundException(request.ViewId);
        }

        await ownership.EnsureCanMutateAsync(view.CreatedBy, view.IsShared, request.CaseId, cancellationToken);

        view.Rename(request.Name);
        await views.SaveChangesAsync(cancellationToken);
    }
}
