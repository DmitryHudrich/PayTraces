namespace Ledgerscope.Accounts.Application.Authorization;

/// <summary>
/// Marks a command that mutates a per-user-owned resource inside a case
/// (e.g. a private canvas view). Role permissions alone can't express "only
/// the owner — unless you hold the case-wide override", so the
/// <see cref="Behaviors.OwnershipBehavior{TRequest,TResponse}"/> adds that
/// check on top of the ordinary permission check.
/// </summary>
public interface IOwnedCaseResource {
    Guid ResourceOwnerId { get; }

    Boolean IsShared { get; }

    Guid CaseId { get; }
}
