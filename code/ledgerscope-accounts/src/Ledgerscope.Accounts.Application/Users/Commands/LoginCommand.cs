using Ledgerscope.Accounts.Application.Auth;
using MediatR;

namespace Ledgerscope.Accounts.Application.Users.Commands;

public sealed record LoginCommand(String Email, String Password) : IRequest<AccessToken>;

public sealed class LoginCommandHandler(IUserRepository users, IPasswordHasher passwordHasher, IJwtIssuer jwtIssuer) : IRequestHandler<LoginCommand, AccessToken> {
    private readonly IUserRepository users = users;
    private readonly IPasswordHasher passwordHasher = passwordHasher;
    private readonly IJwtIssuer jwtIssuer = jwtIssuer;

    public async Task<AccessToken> Handle(LoginCommand request, CancellationToken cancellationToken) {
        var email = request.Email.Trim().ToLowerInvariant();
        var user = await users.GetByEmailAsync(email, cancellationToken);

        return user is null || !user.IsActive || user.PasswordHash is null
            || !passwordHasher.Verify(user.PasswordHash, request.Password)
            ? throw new InvalidCredentialsException()
            : jwtIssuer.Issue(user);
    }
}
