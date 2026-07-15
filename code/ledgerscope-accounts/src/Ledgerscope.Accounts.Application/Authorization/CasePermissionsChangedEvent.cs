using MediatR;

namespace Ledgerscope.Accounts.Application.Authorization;

/// <summary>
/// Published by command handlers after they change a user's role assignments
/// — globally when <see cref="CaseId"/> is null, or on a specific case. An
/// Infrastructure notification handler invalidates that user's cached
/// permission set in response, so command handlers never touch the cache.
/// </summary>
public sealed record CasePermissionsChangedEvent(Guid UserId, Guid? CaseId) : INotification;
