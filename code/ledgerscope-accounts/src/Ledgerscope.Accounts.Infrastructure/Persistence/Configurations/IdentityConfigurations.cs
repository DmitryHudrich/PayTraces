using Ledgerscope.Accounts.Domain.Identity;
using Ledgerscope.Accounts.Domain.Users;
using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Metadata.Builders;

namespace Ledgerscope.Accounts.Infrastructure.Persistence.Configurations;

public sealed class UserConfiguration : IEntityTypeConfiguration<User> {
    public void Configure(EntityTypeBuilder<User> builder) {
        _ = builder.ToTable("users");
        _ = builder.HasKey(entity => entity.Id);
        _ = builder.Property(entity => entity.Id).ValueGeneratedNever();

        _ = builder.Property(entity => entity.Email).HasMaxLength(256).IsRequired();
        _ = builder.Property(entity => entity.DisplayName).HasMaxLength(200);
        _ = builder.Property(entity => entity.PasswordHash).HasMaxLength(512);

        _ = builder.HasIndex(entity => entity.Email).IsUnique();
        _ = builder.HasIndex(entity => entity.OrganizationId);
    }
}

public sealed class UserRoleAssignmentConfiguration : IEntityTypeConfiguration<UserRoleAssignment> {
    public void Configure(EntityTypeBuilder<UserRoleAssignment> builder) {
        _ = builder.ToTable("user_role_assignments");
        _ = builder.HasKey(entity => entity.Id);
        _ = builder.Property(entity => entity.Id).ValueGeneratedNever();

        _ = builder.Property(entity => entity.RoleName).HasMaxLength(64).IsRequired();

        _ = builder.HasIndex(entity => new { entity.UserId, entity.RoleName, entity.OrganizationId }).IsUnique();
        _ = builder.HasIndex(entity => entity.UserId);
    }
}
