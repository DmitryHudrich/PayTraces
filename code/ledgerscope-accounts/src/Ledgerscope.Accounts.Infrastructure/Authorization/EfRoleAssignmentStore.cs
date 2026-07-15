using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Domain.Cases;
using Ledgerscope.Accounts.Infrastructure.Persistence;
using Microsoft.EntityFrameworkCore;

namespace Ledgerscope.Accounts.Infrastructure.Authorization;

/// <summary>
/// EF-backed source of a user's role <em>assignments</em>. Role definitions
/// (what each role name grants) come from config; this only reads which role
/// names a user holds globally and per case.
/// </summary>
public sealed class EfRoleAssignmentStore(AccountsDbContext db) : IRoleAssignmentStore {
    private readonly AccountsDbContext db = db;

    public async Task<IReadOnlyCollection<String>> GetGlobalRoleNamesAsync(Guid userId, CancellationToken ct) {
        return await db.UserRoleAssignments
            .Where(assignment => assignment.UserId == userId)
            .Select(assignment => assignment.RoleName)
            .Distinct()
            .ToListAsync(ct);
    }

    public async Task<IReadOnlyCollection<String>> GetCaseRoleNamesAsync(
        Guid userId, Guid caseId, CancellationToken ct) {
        return await db.Set<CaseAssignment>()
            .Where(assignment => assignment.UserId == userId && assignment.CaseId == caseId)
            .Select(assignment => assignment.RoleName)
            .Distinct()
            .ToListAsync(ct);
    }
}
