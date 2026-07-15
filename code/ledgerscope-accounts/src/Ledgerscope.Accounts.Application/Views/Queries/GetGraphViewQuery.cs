using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Views.Queries;

/// <summary>
/// Reads a single graph view (with its pinned positions). A private view owned
/// by someone else is only returned to a caller holding the case-wide
/// ViewManageSharing override.
/// </summary>
public sealed record GetGraphViewQuery(Guid CaseId, Guid ViewId)
    : IRequest<CaseGraphViewDto>, IRequirePermission {
    public Permission Required => Permission.ViewRead;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class GetGraphViewQueryHandler(ICaseGraphViewRepository views, ICaseResourceOwnership ownership) : IRequestHandler<GetGraphViewQuery, CaseGraphViewDto> {
    private readonly ICaseGraphViewRepository views = views;
    private readonly ICaseResourceOwnership ownership = ownership;

    public async Task<CaseGraphViewDto> Handle(GetGraphViewQuery request, CancellationToken cancellationToken) {
        var view = await views.GetByIdAsync(request.ViewId, cancellationToken);
        if (view is null || view.CaseId != request.CaseId) {
            throw new ViewNotFoundException(request.ViewId);
        }

        await ownership.EnsureCanMutateAsync(view.CreatedBy, view.IsShared, request.CaseId, cancellationToken);

        return view.ToDto();
    }
}
