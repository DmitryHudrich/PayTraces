using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Labels.Queries;

public sealed record ListCaseLabelsQuery(Guid CaseId)
    : IRequest<IReadOnlyList<CustomLabelDto>>, IRequirePermission {
    public Permission Required => Permission.CaseRead;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class ListCaseLabelsQueryHandler(ILabelRepository labels)
    : IRequestHandler<ListCaseLabelsQuery, IReadOnlyList<CustomLabelDto>> {
    private readonly ILabelRepository labels = labels;

    public async Task<IReadOnlyList<CustomLabelDto>> Handle(
        ListCaseLabelsQuery request, CancellationToken cancellationToken) {
        var found = await labels.ListLabelsByCaseAsync(request.CaseId, cancellationToken);
        return [.. found.Select(label => label.ToDto())];
    }
}
