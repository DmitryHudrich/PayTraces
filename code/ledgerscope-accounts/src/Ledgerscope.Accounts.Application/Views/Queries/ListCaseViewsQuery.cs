using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Views.Queries;

/// <summary>
/// Lists the graph views of a case visible to the caller: their own views, any
/// shared views, and — if they hold the case-wide ViewManageSharing override
/// (e.g. the case Lead) — everyone's.
/// </summary>
public sealed record ListCaseViewsQuery(Guid CaseId)
    : IRequest<IReadOnlyList<CaseGraphViewSummaryDto>>, IRequirePermission {
    public Permission Required => Permission.ViewRead;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class ListCaseViewsQueryHandler(
    ICaseGraphViewRepository views, IUserContext user, IPermissionResolver permissions)
        : IRequestHandler<ListCaseViewsQuery, IReadOnlyList<CaseGraphViewSummaryDto>> {
    private readonly ICaseGraphViewRepository views = views;
    private readonly IUserContext user = user;
    private readonly IPermissionResolver permissions = permissions;

    public async Task<IReadOnlyList<CaseGraphViewSummaryDto>> Handle(
        ListCaseViewsQuery request, CancellationToken cancellationToken) {
        var all = await views.ListByCaseAsync(request.CaseId, cancellationToken);
        var canSeeAll = await permissions.HasAsync(
            user.UserId, Permission.ViewManageSharing, request.CaseId, cancellationToken);

        return [.. all
            .Where(view => canSeeAll || view.IsShared || view.CreatedBy == user.UserId)
            .Select(view => view.ToSummary())];
    }
}
