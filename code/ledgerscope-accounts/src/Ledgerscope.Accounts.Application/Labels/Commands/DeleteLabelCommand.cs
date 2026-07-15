using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Labels.Commands;

public sealed record DeleteLabelCommand(Guid CaseId, Guid LabelId)
    : IRequest, IRequirePermission {
    public Permission Required => Permission.LabelCreate;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class DeleteLabelCommandHandler(ILabelRepository labels)
    : IRequestHandler<DeleteLabelCommand> {
    private readonly ILabelRepository labels = labels;

    public async Task Handle(DeleteLabelCommand request, CancellationToken cancellationToken) {
        var label = await labels.GetLabelAsync(request.LabelId, cancellationToken);
        if (label is null || label.CaseId != request.CaseId) {
            throw new LabelNotFoundException(request.LabelId);
        }

        labels.RemoveLabel(label);
        await labels.SaveChangesAsync(cancellationToken);
    }
}
