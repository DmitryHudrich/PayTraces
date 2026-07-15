using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using Ledgerscope.Accounts.Domain.Views;
using MediatR;

namespace Ledgerscope.Accounts.Application.Views.Commands;

public sealed record CreateGraphViewCommand(Guid CaseId, String Name, Boolean IsShared)
    : IRequest<Guid>, IRequirePermission {
    public Permission Required => Permission.ViewCreate;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class CreateGraphViewCommandHandler(ICaseGraphViewRepository views, IUserContext user) : IRequestHandler<CreateGraphViewCommand, Guid> {
    private readonly ICaseGraphViewRepository views = views;
    private readonly IUserContext user = user;

    public async Task<Guid> Handle(CreateGraphViewCommand request, CancellationToken cancellationToken) {
        var entity = new CaseGraphView(
            Guid.NewGuid(), request.CaseId, request.Name, user.UserId, DateTimeOffset.UtcNow);
        entity.SetShared(request.IsShared);

        await views.AddAsync(entity, cancellationToken);
        await views.SaveChangesAsync(cancellationToken);
        return entity.Id;
    }
}
