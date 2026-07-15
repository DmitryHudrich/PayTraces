using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Design;

namespace Ledgerscope.Accounts.Infrastructure.Persistence;

/// <summary>
/// Design-time factory so <c>dotnet ef</c> can build the context without
/// spinning up the API host. Uses LEDGERSCOPE_ACCOUNTS_DB when set, otherwise
/// the local dev default. Runtime resolves the DI-configured context instead.
/// </summary>
public sealed class AccountsDbContextFactory : IDesignTimeDbContextFactory<AccountsDbContext> {
    public AccountsDbContext CreateDbContext(String[] args) {
        var connectionString = Environment.GetEnvironmentVariable("LEDGERSCOPE_ACCOUNTS_DB")
            ?? "Host=localhost;Port=5432;Database=wallet_db;Username=wallet;Password=wallet;Search Path=accounts";

        var options = new DbContextOptionsBuilder<AccountsDbContext>()
            .UseNpgsql(connectionString, npgsql =>
                npgsql.MigrationsHistoryTable("__ef_migrations_history", AccountsDbContext.Schema))
            .Options;

        return new AccountsDbContext(options);
    }
}
