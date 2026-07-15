using System.Text.Json;
using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Authorization;
using Microsoft.Extensions.Caching.Distributed;

namespace Ledgerscope.Accounts.Infrastructure.Authorization;

/// <summary>
/// Cache-aside decorator over the DB-backed <see cref="PermissionResolver"/>.
/// Effective permission sets are read straight from Redis on a hit; on a miss
/// they're resolved from the database and stored. Invalidation is explicit and
/// event-driven (<see cref="InvalidatePermissionCacheHandler"/> reacts to
/// <see cref="CasePermissionsChangedEvent"/>); the TTL is only a backstop.
/// </summary>
public sealed class CachedPermissionResolver(PermissionResolver inner, IDistributedCache cache) : IPermissionResolver {
    private static readonly TimeSpan Ttl = TimeSpan.FromMinutes(5);

    private readonly PermissionResolver inner = inner;
    private readonly IDistributedCache cache = cache;

    public static String KeyFor(Guid userId, Guid? caseId) {
        return caseId is Guid id ? $"perm:{userId}:case:{id}" : $"perm:{userId}:global";
    }

    public async Task<IReadOnlySet<Permission>> GetEffectivePermissionsAsync(
        Guid userId, Guid? caseId, CancellationToken ct) {
        var key = KeyFor(userId, caseId);

        var cached = await cache.GetStringAsync(key, ct);
        if (cached is not null) {
            return Deserialize(cached);
        }

        var effective = await inner.GetEffectivePermissionsAsync(userId, caseId, ct);

        await cache.SetStringAsync(
            key, Serialize(effective),
            new DistributedCacheEntryOptions { AbsoluteExpirationRelativeToNow = Ttl },
            ct);

        return effective;
    }

    public async Task<Boolean> HasAsync(Guid userId, Permission permission, Guid? caseId, CancellationToken ct) {
        var effective = await GetEffectivePermissionsAsync(userId, caseId, ct);
        return effective.Contains(permission);
    }

    private static String Serialize(IReadOnlySet<Permission> permissions) {
        return JsonSerializer.Serialize(permissions.Select(permission => permission.ToString()));
    }

    private static IReadOnlySet<Permission> Deserialize(String json) {
        var names = JsonSerializer.Deserialize<String[]>(json) ?? [];
        var set = new HashSet<Permission>();
        foreach (var name in names) {
            if (Enum.TryParse<Permission>(name, out var permission)) {
                _ = set.Add(permission);
            }
        }

        return set;
    }
}
