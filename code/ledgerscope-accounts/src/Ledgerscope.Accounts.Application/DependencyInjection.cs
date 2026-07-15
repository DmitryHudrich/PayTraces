using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Application.Behaviors;
using MediatR;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;

namespace Ledgerscope.Accounts.Application;

public static class DependencyInjection {
    public static IServiceCollection AddLedgerscopeApplication(
        this IServiceCollection services, IConfiguration configuration) {
        _ = services.AddMediatR(cfg =>
            cfg.RegisterServicesFromAssembly(typeof(DependencyInjection).Assembly));

        _ = services.AddOptions<AuthorizationOptions>()
            .Bind(configuration.GetSection(AuthorizationOptions.SectionName));

        _ = services.AddSingleton<IRolePermissionMap, RolePermissionMap>();

        // Registered as the concrete type too so Infrastructure can wrap it
        // with a caching decorator while this stays the underlying resolver.
        _ = services.AddScoped<PermissionResolver>();
        _ = services.AddScoped<IPermissionResolver>(sp => sp.GetRequiredService<PermissionResolver>());

        _ = services.AddScoped<ICaseResourceOwnership, CaseResourceOwnership>();

        // Order matters: the role/permission check runs first, then the
        // ownership check for private per-user resources.
        _ = services.AddScoped(typeof(IPipelineBehavior<,>), typeof(AuthorizationBehavior<,>));
        _ = services.AddScoped(typeof(IPipelineBehavior<,>), typeof(OwnershipBehavior<,>));

        return services;
    }
}
