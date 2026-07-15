using Ledgerscope.Accounts.Domain.Cases;
using Ledgerscope.Accounts.Domain.Groups;
using Ledgerscope.Accounts.Domain.Identity;
using Ledgerscope.Accounts.Domain.Labels;
using Ledgerscope.Accounts.Domain.Users;
using Ledgerscope.Accounts.Domain.Views;
using Microsoft.EntityFrameworkCore;

namespace Ledgerscope.Accounts.Infrastructure.Persistence;

/// <summary>
/// EF Core context for the Ledgerscope Accounts schema. Everything lives in
/// the dedicated <c>accounts</c> schema — no cross-schema FK to the Rust
/// engine's chain-data tables; the link to the graph is by value
/// (address/chain_id), never by reference.
/// </summary>
public sealed class AccountsDbContext(DbContextOptions<AccountsDbContext> options) : DbContext(options) {
    public const String Schema = "accounts";

    public DbSet<Case> Cases => Set<Case>();
    public DbSet<User> Users => Set<User>();
    public DbSet<UserRoleAssignment> UserRoleAssignments => Set<UserRoleAssignment>();
    public DbSet<CaseGraphView> GraphViews => Set<CaseGraphView>();
    public DbSet<CustomLabel> CustomLabels => Set<CustomLabel>();
    public DbSet<AddressLabelLink> AddressLabelLinks => Set<AddressLabelLink>();
    public DbSet<AddressGroup> AddressGroups => Set<AddressGroup>();

    protected override void OnModelCreating(ModelBuilder modelBuilder) {
        _ = modelBuilder.HasDefaultSchema(Schema);
        _ = modelBuilder.ApplyConfigurationsFromAssembly(typeof(AccountsDbContext).Assembly);
    }
}
