using System.IdentityModel.Tokens.Jwt;
using System.Security.Claims;
using System.Text;
using Ledgerscope.Accounts.Application.Auth;
using Ledgerscope.Accounts.Domain.Users;
using Microsoft.Extensions.Options;
using Microsoft.IdentityModel.Tokens;

namespace Ledgerscope.Accounts.Infrastructure.Auth;

public sealed class JwtIssuer(IOptions<JwtOptions> options) : IJwtIssuer {
    private readonly JwtOptions options = options.Value;

    public AccessToken Issue(User user) {
        var expiresAt = DateTimeOffset.UtcNow.AddMinutes(options.ExpiryMinutes);

        var claims = new[]
        {
            new Claim(JwtRegisteredClaimNames.Sub, user.Id.ToString()),
            new Claim(JwtRegisteredClaimNames.Email, user.Email),
            new Claim(JwtRegisteredClaimNames.Jti, Guid.NewGuid().ToString()),
            new Claim("org", user.OrganizationId.ToString()),
            new Claim("name", user.DisplayName),
        };

        var key = new SymmetricSecurityKey(Encoding.UTF8.GetBytes(options.SigningKey));
        var credentials = new SigningCredentials(key, SecurityAlgorithms.HmacSha256);

        var token = new JwtSecurityToken(
            issuer: options.Issuer,
            audience: options.Audience,
            claims: claims,
            expires: expiresAt.UtcDateTime,
            signingCredentials: credentials);

        var encoded = new JwtSecurityTokenHandler().WriteToken(token);
        return new AccessToken(encoded, expiresAt);
    }
}
