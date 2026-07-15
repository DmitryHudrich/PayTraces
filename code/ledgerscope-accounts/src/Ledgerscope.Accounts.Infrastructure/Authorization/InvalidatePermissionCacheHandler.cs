using Ledgerscope.Accounts.Application.Authorization;
using MediatR;
using Microsoft.Extensions.Caching.Distributed;

namespace Ledgerscope.Accounts.Infrastructure.Authorization;

/// <summary>
/// Reacts to <see cref="CasePermissionsChangedEvent"/> by evicting the
/// affected user's cached permission set. Command handlers publish the event
/// after changing role assignments and stay unaware of the cache.
/// </summary>
public sealed class InvalidatePermissionCacheHandler(IDistributedCache cache) : INotificationHandler<CasePermissionsChangedEvent> {
    private readonly IDistributedCache cache = cache;

    public Task Handle(CasePermissionsChangedEvent notification, CancellationToken cancellationToken) {
        return cache.RemoveAsync(
            CachedPermissionResolver.KeyFor(notification.UserId, notification.CaseId), cancellationToken);
    }
}
