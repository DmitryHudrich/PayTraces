using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Cases.Commands;

public sealed record CloseCaseCommand(Guid CaseId) : IRequest, IRequirePermission {
    public Permission Required => Permission.CaseClose;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class CloseCaseCommandHandler(ICaseRepository cases) : IRequestHandler<CloseCaseCommand> {
    private readonly ICaseRepository cases = cases;

    public async Task Handle(CloseCaseCommand request, CancellationToken cancellationToken) {
        var entity = await cases.GetByIdAsync(request.CaseId, cancellationToken)
            ?? throw new CaseNotFoundException(request.CaseId);

        entity.Close(DateTimeOffset.UtcNow);
        await cases.SaveChangesAsync(cancellationToken);
    }
}
