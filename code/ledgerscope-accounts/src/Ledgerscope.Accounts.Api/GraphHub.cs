using System.Runtime.CompilerServices;

using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Application.Graph;
using Ledgerscope.Accounts.Application.Views;
using Ledgerscope.Accounts.Domain.Authorization;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.SignalR;

namespace Ledgerscope.Accounts.Api;

/// <summary>
/// One streamed item of a case's graph: a page from the engine plus, on the
/// first item only, the pinned node positions from the requested canvas view so
/// the client can lay the graph out as the investigator left it.
/// </summary>
public sealed record GraphStreamItem(GraphPageDto Page, IReadOnlyList<NodePositionDto>? Positions);

/// <summary>
/// Progressive/live graph delivery for the frontend. The frontend connects with
/// its JWT, then either streams a case's graph (paged BFS from the engine, with
/// pinned positions merged in) or expands a single node on click. Every method
/// re-checks case access — a live connection is not a standing authorization.
/// </summary>
[Authorize]
public sealed class GraphHub(
    IPermissionResolver permissions, IGraphEngineClient engine, ICaseGraphViewRepository views) : Hub {
    private const Int32 PageSize = 200;

    private readonly IPermissionResolver permissions = permissions;
    private readonly IGraphEngineClient engine = engine;
    private readonly ICaseGraphViewRepository views = views;

    /// <summary>
    /// Streams the engine's paged BFS walk around <paramref name="address"/>.
    /// The first item also carries the pinned positions of <paramref name="viewId"/>
    /// (when it belongs to the case and is the caller's own or shared).
    /// </summary>
    public async IAsyncEnumerable<GraphStreamItem> StreamCaseGraph(
        Guid caseId, String address, Int32 chainId, Int32 maxDepth, Guid? viewId,
        [EnumeratorCancellation] CancellationToken ct) {
        var caller = CallerId();
        await EnsureCaseReadAsync(caller, caseId, ct);

        var positions = await LoadPositionsAsync(caller, caseId, viewId, ct);

        var page = 0;
        while (true) {
            ct.ThrowIfCancellationRequested();
            var result = await engine.GetGraphPageAsync(address, chainId, maxDepth, page, PageSize, ct);
            yield return new GraphStreamItem(result, page == 0 ? positions : null);
            if (!result.HasNext) {
                break;
            }

            page++;
        }
    }

    /// <summary>
    /// Click-to-load: one page of neighbours around a node.
    /// </summary>
    public async Task<GraphPageDto> ExpandNode(Guid caseId, String address, Int32 chainId, Int32 maxDepth) {
        var caller = CallerId();
        await EnsureCaseReadAsync(caller, caseId, Context.ConnectionAborted);
        return await engine.GetGraphPageAsync(
            address, chainId, maxDepth <= 0 ? 1 : maxDepth, 0, PageSize, Context.ConnectionAborted);
    }

    private Guid CallerId() {
        var sub = Context.User?.FindFirst("sub")?.Value;
        return Guid.TryParse(sub, out var id)
            ? id
            : throw new HubException("The connection is not associated with a valid user.");
    }

    private async Task EnsureCaseReadAsync(Guid caller, Guid caseId, CancellationToken ct) {
        var allowed = await permissions.HasAsync(caller, Permission.CaseRead, caseId, ct);
        if (!allowed) {
            throw new HubException($"Caller lacks permission {Permission.CaseRead} on case {caseId}.");
        }
    }

    private async Task<IReadOnlyList<NodePositionDto>?> LoadPositionsAsync(
        Guid caller, Guid caseId, Guid? viewId, CancellationToken ct) {
        if (viewId is not Guid id) {
            return null;
        }

        var view = await views.GetByIdAsync(id, ct);
        if (view is null || view.CaseId != caseId || (!view.IsShared && view.CreatedBy != caller)) {
            return null;
        }

        return [.. view.Positions.Select(p => new NodePositionDto(p.Address, p.X, p.Y))];
    }
}
