using Ledgerscope.Accounts.Domain.Cases;
using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Metadata.Builders;

namespace Ledgerscope.Accounts.Infrastructure.Persistence.Configurations;

public sealed class CaseConfiguration : IEntityTypeConfiguration<Case> {
    public void Configure(EntityTypeBuilder<Case> builder) {
        _ = builder.ToTable("cases");
        _ = builder.HasKey(entity => entity.Id);
        _ = builder.Property(entity => entity.Id).ValueGeneratedNever();

        _ = builder.Property(entity => entity.Title).HasMaxLength(200).IsRequired();
        _ = builder.Property(entity => entity.Description).HasMaxLength(4000);
        _ = builder.Property(entity => entity.Status).HasConversion<String>().HasMaxLength(20);
        _ = builder.Property(entity => entity.Priority).HasConversion<String>().HasMaxLength(20);

        _ = builder.HasIndex(entity => entity.OrganizationId);

        _ = builder.HasMany(entity => entity.Addresses)
            .WithOne()
            .HasForeignKey(address => address.CaseId)
            .OnDelete(DeleteBehavior.Cascade);
        _ = builder.HasMany(entity => entity.Assignments)
            .WithOne()
            .HasForeignKey(assignment => assignment.CaseId)
            .OnDelete(DeleteBehavior.Cascade);
        _ = builder.HasMany(entity => entity.Notes)
            .WithOne()
            .HasForeignKey(note => note.CaseId)
            .OnDelete(DeleteBehavior.Cascade);

        _ = builder.Navigation(entity => entity.Addresses).UsePropertyAccessMode(PropertyAccessMode.Field);
        _ = builder.Navigation(entity => entity.Assignments).UsePropertyAccessMode(PropertyAccessMode.Field);
        _ = builder.Navigation(entity => entity.Notes).UsePropertyAccessMode(PropertyAccessMode.Field);
    }
}
