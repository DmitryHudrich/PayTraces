using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using Ledgerscope.Accounts.Domain.Labels;
using MediatR;

namespace Ledgerscope.Accounts.Application.Labels.Commands;

public sealed record CreateLabelCommand(Guid CaseId, String Text, String? Color)
    : IRequest<Guid>, IRequirePermission {
    public Permission Required => Permission.LabelCreate;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class CreateLabelCommandHandler(ILabelRepository labels, IUserContext user)
    : IRequestHandler<CreateLabelCommand, Guid> {
    private readonly ILabelRepository labels = labels;
    private readonly IUserContext user = user;

    public async Task<Guid> Handle(CreateLabelCommand request, CancellationToken cancellationToken) {
        var entity = new CustomLabel(
            Guid.NewGuid(), request.CaseId, user.UserId, request.Text, request.Color, DateTimeOffset.UtcNow);

        await labels.AddLabelAsync(entity, cancellationToken);
        await labels.SaveChangesAsync(cancellationToken);
        return entity.Id;
    }
}
