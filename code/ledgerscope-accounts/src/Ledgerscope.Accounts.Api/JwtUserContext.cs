using Ledgerscope.Accounts.Application.Authorization;

namespace Ledgerscope.Accounts.Api;

/// <summary>
/// <see cref="IUserContext"/> backed by the authenticated JWT principal. The
/// user id comes from the token's <c>sub</c> claim (inbound claim mapping is
/// disabled, so it stays "sub").
/// </summary>
public sealed class JwtUserContext(IHttpContextAccessor accessor) : IUserContext {
    private readonly IHttpContextAccessor accessor = accessor;

    public Boolean IsAuthenticated => accessor.HttpContext?.User.Identity?.IsAuthenticated ?? false;

    public Guid UserId {
        get {
            var subject = accessor.HttpContext?.User.FindFirst("sub")?.Value;
            return Guid.TryParse(subject, out var id)
                ? id
                : throw new InvalidOperationException("No authenticated user on the current request.");
        }
    }
}
