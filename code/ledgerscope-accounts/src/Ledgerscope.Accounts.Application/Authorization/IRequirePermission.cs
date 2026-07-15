using Ledgerscope.Accounts.Domain.Authorization;

namespace Ledgerscope.Accounts.Application.Authorization;

/// <summary>
/// A command or query declares the permission it needs by implementing this.
/// The <see cref="Behaviors.AuthorizationBehavior{TRequest,TResponse}"/>
/// reads it off the request and checks it against the caller's effective
/// permissions before the handler runs. <see cref="CaseId"/> null → the check
/// runs against global roles only; non-null → it is scoped to that case.
/// </summary>
public interface IRequirePermission {
    Permission Required { get; }

    Guid? CaseId { get; }
}
