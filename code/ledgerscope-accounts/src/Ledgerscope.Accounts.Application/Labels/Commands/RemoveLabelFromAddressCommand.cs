using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using MediatR;

namespace Ledgerscope.Accounts.Application.Labels.Commands;

public sealed record RemoveLabelFromAddressCommand(Guid CaseId, Guid LabelId, String Address, Int32 ChainId)
    : IRequest, IRequirePermission {
    public Permission Required => Permission.LabelApply;

    Guid? IRequirePermission.CaseId => CaseId;
}

public sealed class RemoveLabelFromAddressCommandHandler(ILabelRepository labels)
    : IRequestHandler<RemoveLabelFromAddressCommand> {
    private readonly ILabelRepository labels = labels;

    public async Task Handle(RemoveLabelFromAddressCommand request, CancellationToken cancellationToken) {
        var label = await labels.GetLabelAsync(request.LabelId, cancellationToken);
        if (label is null || label.CaseId != request.CaseId) {
            throw new LabelNotFoundException(request.LabelId);
        }

        var link = await labels.GetLinkAsync(
            request.LabelId, request.Address, request.ChainId, cancellationToken);
        if (link is null) {
            return;
        }

        labels.RemoveLink(link);
        await labels.SaveChangesAsync(cancellationToken);
    }
}
