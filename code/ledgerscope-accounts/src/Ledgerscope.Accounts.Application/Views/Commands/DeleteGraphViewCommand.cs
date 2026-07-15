using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Views.Commands;

public sealed record DeleteGraphViewCommand(Guid CaseId, Guid ViewId)
    : IRequest, IRequirePermission {
    public Permission Required => Permission.ViewDelete;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class DeleteGraphViewCommandHandler(ICaseGraphViewRepository views, ICaseResourceOwnership ownership) : IRequestHandler<DeleteGraphViewCommand> {
    private readonly ICaseGraphViewRepository views = views;
    private readonly ICaseResourceOwnership ownership = ownership;

    public async Task Handle(DeleteGraphViewCommand request, CancellationToken cancellationToken) {
        var view = await views.GetByIdAsync(request.ViewId, cancellationToken);
        if (view is null || view.CaseId != request.CaseId) {
            throw new ViewNotFoundException(request.ViewId);
        }

        await ownership.EnsureCanMutateAsync(view.CreatedBy, view.IsShared, request.CaseId, cancellationToken);

        views.Remove(view);
        await views.SaveChangesAsync(cancellationToken);
    }
}
