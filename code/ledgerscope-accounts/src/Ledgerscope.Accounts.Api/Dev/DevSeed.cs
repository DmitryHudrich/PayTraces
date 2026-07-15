using Ledgerscope.Accounts.Domain.Identity;
using Ledgerscope.Accounts.Domain.Users;
using Ledgerscope.Accounts.Infrastructure.Persistence;
using Microsoft.EntityFrameworkCore;

namespace Ledgerscope.Accounts.Api.Dev;

/// <summary>
/// TEMPORARY: seeds a dev user + a global Investigator assignment so the
/// stack is exercisable before real user registration exists. Idempotent.
/// </summary>
public static class DevSeed {
    public static readonly Guid DevUserId = Guid.Parse("00000000-0000-0000-0000-000000000001");
    public static readonly Guid DevOrgId = Guid.Parse("00000000-0000-0000-0000-00000000000a");

    public static async Task EnsureAsync(AccountsDbContext db) {
        if (!await db.Users.AnyAsync(user => user.Id == DevUserId)) {
            _ = db.Users.Add(new User(DevUserId, "dev@ledgerscope.local", "Dev User", DevOrgId, DateTimeOffset.UtcNow));
        }

        var hasInvestigator = await db.UserRoleAssignments
            .AnyAsync(assignment => assignment.UserId == DevUserId && assignment.RoleName == "Investigator");
        if (!hasInvestigator) {
            _ = db.UserRoleAssignments.Add(new UserRoleAssignment(
                Guid.NewGuid(), DevUserId, "Investigator", DevOrgId, DevUserId, DateTimeOffset.UtcNow));
        }

        _ = await db.SaveChangesAsync();
    }
}
