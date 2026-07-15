namespace Ledgerscope.Accounts.Application.Authorization;

/// <summary>
/// The authenticated caller for the current request. Implemented in the API
/// layer from the JWT the frontend presents (the frontend ↔ Accounts
/// boundary).
/// </summary>
public interface IUserContext {
    Guid UserId { get; }

    Boolean IsAuthenticated { get; }
}
