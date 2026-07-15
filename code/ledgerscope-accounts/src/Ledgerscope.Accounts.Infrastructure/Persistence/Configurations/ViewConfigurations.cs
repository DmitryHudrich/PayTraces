using Ledgerscope.Accounts.Domain.Views;
using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Metadata.Builders;

namespace Ledgerscope.Accounts.Infrastructure.Persistence.Configurations;

public sealed class CaseGraphViewConfiguration : IEntityTypeConfiguration<CaseGraphView> {
    public void Configure(EntityTypeBuilder<CaseGraphView> builder) {
        _ = builder.ToTable("graph_views");
        _ = builder.HasKey(entity => entity.Id);
        _ = builder.Property(entity => entity.Id).ValueGeneratedNever();

        _ = builder.Property(entity => entity.Name).HasMaxLength(200).IsRequired();

        _ = builder.HasIndex(entity => entity.CaseId);

        _ = builder.HasMany(entity => entity.Positions)
            .WithOne()
            .HasForeignKey(position => position.ViewId)
            .OnDelete(DeleteBehavior.Cascade);

        _ = builder.Navigation(entity => entity.Positions).UsePropertyAccessMode(PropertyAccessMode.Field);
    }
}

public sealed class CaseGraphNodePositionConfiguration : IEntityTypeConfiguration<CaseGraphNodePosition> {
    public void Configure(EntityTypeBuilder<CaseGraphNodePosition> builder) {
        _ = builder.ToTable("graph_node_positions");
        _ = builder.HasKey(entity => entity.Id);
        _ = builder.Property(entity => entity.Id).ValueGeneratedNever();

        _ = builder.Property(entity => entity.Address).HasMaxLength(128).IsRequired();

        _ = builder.HasIndex(entity => new { entity.ViewId, entity.Address }).IsUnique();
    }
}
