using Ledgerscope.Accounts.Domain.Authorization;

namespace Ledgerscope.Accounts.Application.Authorization;

/// <summary>
/// Thrown by the authorization pipeline when the caller lacks the required
/// permission or fails an ownership check. The API layer maps this to 403.
/// </summary>
public sealed class ForbiddenException(String message) : Exception(message) {
    public static ForbiddenException MissingPermission(Permission permission, Guid? caseId) {
        return new(caseId is null
            ? $"Caller lacks permission {permission}."
            : $"Caller lacks permission {permission} on case {caseId}.");
    }
}
