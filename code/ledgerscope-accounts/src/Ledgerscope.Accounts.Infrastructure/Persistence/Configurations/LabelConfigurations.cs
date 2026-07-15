using Ledgerscope.Accounts.Domain.Labels;
using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Metadata.Builders;

namespace Ledgerscope.Accounts.Infrastructure.Persistence.Configurations;

public sealed class CustomLabelConfiguration : IEntityTypeConfiguration<CustomLabel> {
    public void Configure(EntityTypeBuilder<CustomLabel> builder) {
        _ = builder.ToTable("custom_labels");
        _ = builder.HasKey(entity => entity.Id);
        _ = builder.Property(entity => entity.Id).ValueGeneratedNever();

        _ = builder.Property(entity => entity.Text).HasMaxLength(200).IsRequired();
        _ = builder.Property(entity => entity.Color).HasMaxLength(32);

        _ = builder.HasIndex(entity => entity.CaseId);
    }
}

public sealed class AddressLabelLinkConfiguration : IEntityTypeConfiguration<AddressLabelLink> {
    public void Configure(EntityTypeBuilder<AddressLabelLink> builder) {
        _ = builder.ToTable("address_label_links");
        _ = builder.HasKey(entity => entity.Id);
        _ = builder.Property(entity => entity.Id).ValueGeneratedNever();

        _ = builder.Property(entity => entity.Address).HasMaxLength(128).IsRequired();

        _ = builder.HasIndex(entity => new { entity.LabelId, entity.Address, entity.ChainId }).IsUnique();
        _ = builder.HasIndex(entity => new { entity.Address, entity.ChainId });

        _ = builder.HasOne<CustomLabel>()
            .WithMany()
            .HasForeignKey(link => link.LabelId)
            .OnDelete(DeleteBehavior.Cascade);
    }
}
