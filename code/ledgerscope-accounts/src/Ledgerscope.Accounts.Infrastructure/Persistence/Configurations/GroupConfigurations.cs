using Ledgerscope.Accounts.Domain.Groups;
using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Metadata.Builders;

namespace Ledgerscope.Accounts.Infrastructure.Persistence.Configurations;

public sealed class AddressGroupConfiguration : IEntityTypeConfiguration<AddressGroup> {
    public void Configure(EntityTypeBuilder<AddressGroup> builder) {
        _ = builder.ToTable("address_groups");
        _ = builder.HasKey(entity => entity.Id);
        _ = builder.Property(entity => entity.Id).ValueGeneratedNever();

        _ = builder.Property(entity => entity.Name).HasMaxLength(200).IsRequired();

        _ = builder.HasIndex(entity => entity.CaseId);

        _ = builder.HasMany(entity => entity.Members)
            .WithOne()
            .HasForeignKey(member => member.GroupId)
            .OnDelete(DeleteBehavior.Cascade);

        _ = builder.Navigation(entity => entity.Members).UsePropertyAccessMode(PropertyAccessMode.Field);
    }
}

public sealed class AddressGroupMemberConfiguration : IEntityTypeConfiguration<AddressGroupMember> {
    public void Configure(EntityTypeBuilder<AddressGroupMember> builder) {
        _ = builder.ToTable("address_group_members");
        _ = builder.HasKey(entity => entity.Id);
        _ = builder.Property(entity => entity.Id).ValueGeneratedNever();

        _ = builder.Property(entity => entity.Address).HasMaxLength(128).IsRequired();

        _ = builder.HasIndex(entity => new { entity.GroupId, entity.Address, entity.ChainId }).IsUnique();
    }
}
