using Ledgerscope.Accounts.Application.Auth;
using Ledgerscope.Accounts.Domain.Identity;
using Ledgerscope.Accounts.Domain.Users;
using MediatR;

namespace Ledgerscope.Accounts.Application.Users.Commands;

public sealed record RegisterUserCommand(String Email, String Password, String DisplayName, Guid OrganizationId)
    : IRequest<Guid>;

public sealed class RegisterUserCommandHandler(IUserRepository users, IPasswordHasher passwordHasher) : IRequestHandler<RegisterUserCommand, Guid> {
    private readonly IUserRepository users = users;
    private readonly IPasswordHasher passwordHasher = passwordHasher;

    public async Task<Guid> Handle(RegisterUserCommand request, CancellationToken cancellationToken) {
        var email = request.Email.Trim().ToLowerInvariant();
        if (await users.EmailExistsAsync(email, cancellationToken)) {
            throw new EmailAlreadyInUseException(email);
        }

        var now = DateTimeOffset.UtcNow;
        var user = new User(Guid.NewGuid(), email, request.DisplayName, request.OrganizationId, now);
        user.SetPasswordHash(passwordHasher.Hash(request.Password));
        await users.AddAsync(user, cancellationToken);

        // New users start as a global Investigator so they can open cases.
        await users.AddGlobalRoleAsync(
            new UserRoleAssignment(
                Guid.NewGuid(), user.Id, GlobalRoles.Investigator, request.OrganizationId, user.Id, now),
            cancellationToken);

        await users.SaveChangesAsync(cancellationToken);
        return user.Id;
    }
}
