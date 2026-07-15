using Ledgerscope.Accounts.Application.Auth;
using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Application.Cases;
using Ledgerscope.Accounts.Application.Graph;
using Ledgerscope.Accounts.Application.Groups;
using Ledgerscope.Accounts.Application.Labels;
using Ledgerscope.Accounts.Application.Users;
using Ledgerscope.Accounts.Application.Views;
using Ledgerscope.Accounts.Infrastructure.Auth;
using Ledgerscope.Accounts.Infrastructure.Authorization;
using Ledgerscope.Accounts.Infrastructure.Graph;
using Ledgerscope.Accounts.Infrastructure.Persistence;
using MediatR;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Caching.Distributed;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Options;

namespace Ledgerscope.Accounts.Infrastructure;

public static class DependencyInjection {
    public static IServiceCollection AddLedgerscopeInfrastructure(
        this IServiceCollection services, IConfiguration configuration) {
        var connectionString = configuration.GetConnectionString("AccountsDb")
            ?? throw new InvalidOperationException("Missing connection string 'AccountsDb'.");

        _ = services.AddDbContext<AccountsDbContext>(options =>
            options.UseNpgsql(connectionString, npgsql =>
                npgsql.MigrationsHistoryTable("__ef_migrations_history", AccountsDbContext.Schema)));

        _ = services.AddScoped<ICaseRepository, CaseRepository>();
        _ = services.AddScoped<ICaseGraphViewRepository, CaseGraphViewRepository>();
        _ = services.AddScoped<ILabelRepository, LabelRepository>();
        _ = services.AddScoped<IAddressGroupRepository, AddressGroupRepository>();
        _ = services.AddScoped<IUserRepository, UserRepository>();
        _ = services.AddScoped<IRoleAssignmentStore, EfRoleAssignmentStore>();

        _ = services.AddOptions<JwtOptions>()
            .Bind(configuration.GetSection(JwtOptions.SectionName));

        _ = services.AddSingleton<IPasswordHasher, IdentityPasswordHasher>();
        _ = services.AddSingleton<IJwtIssuer, JwtIssuer>();

        // Typed HttpClient to the Rust engine's internal API (shared secret).
        _ = services.AddOptions<GraphEngineOptions>()
            .Bind(configuration.GetSection(GraphEngineOptions.SectionName));
        _ = services.AddHttpClient<IGraphEngineClient, GraphEngineClient>((sp, client) => {
            var graph = sp.GetRequiredService<IOptions<GraphEngineOptions>>().Value;
            client.BaseAddress = new Uri(graph.BaseUrl);
            client.Timeout = TimeSpan.FromSeconds(graph.TimeoutSeconds);
            if (!String.IsNullOrWhiteSpace(graph.ApiKey)) {
                client.DefaultRequestHeaders.Add("X-Api-Key", graph.ApiKey);
            }
        });

        // When Redis is configured, wrap the DB-backed permission resolver with
        // a cache-aside decorator and invalidate it on role-assignment changes.
        // Without it the app still works, just uncached.
        var redisConnection = configuration.GetConnectionString("Redis");
        if (!String.IsNullOrWhiteSpace(redisConnection)) {
            _ = services.AddStackExchangeRedisCache(options => options.Configuration = redisConnection);
            _ = services.AddScoped<IPermissionResolver>(sp => new CachedPermissionResolver(
                sp.GetRequiredService<PermissionResolver>(),
                sp.GetRequiredService<IDistributedCache>()));
            _ = services.AddScoped<INotificationHandler<CasePermissionsChangedEvent>, InvalidatePermissionCacheHandler>();
        }

        return services;
    }
}
