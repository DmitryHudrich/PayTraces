using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using Ledgerscope.Accounts.Domain.Labels;
using MediatR;

namespace Ledgerscope.Accounts.Application.Labels.Commands;

public sealed record ApplyLabelCommand(Guid CaseId, Guid LabelId, String Address, Int32 ChainId)
    : IRequest, IRequirePermission {
    public Permission Required => Permission.LabelApply;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class ApplyLabelCommandHandler(ILabelRepository labels, IUserContext user)
    : IRequestHandler<ApplyLabelCommand> {
    private readonly ILabelRepository labels = labels;
    private readonly IUserContext user = user;

    public async Task Handle(ApplyLabelCommand request, CancellationToken cancellationToken) {
        var label = await labels.GetLabelAsync(request.LabelId, cancellationToken);
        if (label is null || label.CaseId != request.CaseId) {
            throw new LabelNotFoundException(request.LabelId);
        }

        var existing = await labels.GetLinkAsync(
            request.LabelId, request.Address, request.ChainId, cancellationToken);
        if (existing is not null) {
            return;
        }

        var link = new AddressLabelLink(
            Guid.NewGuid(), request.LabelId, request.Address, request.ChainId,
            user.UserId, DateTimeOffset.UtcNow);
        await labels.AddLinkAsync(link, cancellationToken);
        await labels.SaveChangesAsync(cancellationToken);
    }
}
