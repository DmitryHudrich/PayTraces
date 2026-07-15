using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Cases.Queries;

public sealed record GetCaseByIdQuery(Guid CaseId) : IRequest<CaseDto?>, IRequirePermission {
    public Permission Required => Permission.CaseRead;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class GetCaseByIdQueryHandler(ICaseRepository cases) : IRequestHandler<GetCaseByIdQuery, CaseDto?> {
    private readonly ICaseRepository cases = cases;

    public async Task<CaseDto?> Handle(GetCaseByIdQuery request, CancellationToken cancellationToken) {
        var entity = await cases.GetByIdAsync(request.CaseId, cancellationToken);
        return entity?.ToDto();
    }
}
