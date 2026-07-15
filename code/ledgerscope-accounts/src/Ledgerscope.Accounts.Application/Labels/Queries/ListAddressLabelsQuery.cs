using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Labels.Queries;

public sealed record ListAddressLabelsQuery(Guid CaseId, String Address, Int32 ChainId)
    : IRequest<IReadOnlyList<AppliedLabelDto>>, IRequirePermission {
    public Permission Required => Permission.CaseRead;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class ListAddressLabelsQueryHandler(ILabelRepository labels)
    : IRequestHandler<ListAddressLabelsQuery, IReadOnlyList<AppliedLabelDto>> {
    private readonly ILabelRepository labels = labels;

    public async Task<IReadOnlyList<AppliedLabelDto>> Handle(
        ListAddressLabelsQuery request, CancellationToken cancellationToken) {
        var applied = await labels.ListAppliedForAddressAsync(
            request.CaseId, request.Address, request.ChainId, cancellationToken);
        return [.. applied.Select(item => item.ToDto())];
    }
}
