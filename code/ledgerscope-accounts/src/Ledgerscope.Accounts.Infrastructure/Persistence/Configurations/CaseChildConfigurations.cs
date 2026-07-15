using Ledgerscope.Accounts.Domain.Cases;
using Microsoft.EntityFrameworkCore;
using Microsoft.EntityFrameworkCore.Metadata.Builders;

namespace Ledgerscope.Accounts.Infrastructure.Persistence.Configurations;

public sealed class CaseAddressConfiguration : IEntityTypeConfiguration<CaseAddress> {
    public void Configure(EntityTypeBuilder<CaseAddress> builder) {
        _ = builder.ToTable("case_addresses");
        _ = builder.HasKey(entity => entity.Id);
        _ = builder.Property(entity => entity.Id).ValueGeneratedNever();

        _ = builder.Property(entity => entity.Address).HasMaxLength(128).IsRequired();
        _ = builder.Property(entity => entity.Note).HasMaxLength(2000);

        _ = builder.HasIndex(entity => new { entity.CaseId, entity.Address, entity.ChainId }).IsUnique();
    }
}

public sealed class CaseAssignmentConfiguration : IEntityTypeConfiguration<CaseAssignment> {
    public void Configure(EntityTypeBuilder<CaseAssignment> builder) {
        _ = builder.ToTable("case_assignments");
        _ = builder.HasKey(entity => entity.Id);
        _ = builder.Property(entity => entity.Id).ValueGeneratedNever();

        _ = builder.Property(entity => entity.RoleName).HasMaxLength(64).IsRequired();

        _ = builder.HasIndex(entity => new { entity.CaseId, entity.UserId }).IsUnique();
        _ = builder.HasIndex(entity => entity.UserId);
    }
}

public sealed class CaseNoteConfiguration : IEntityTypeConfiguration<CaseNote> {
    public void Configure(EntityTypeBuilder<CaseNote> builder) {
        _ = builder.ToTable("case_notes");
        _ = builder.HasKey(entity => entity.Id);
        _ = builder.Property(entity => entity.Id).ValueGeneratedNever();

        _ = builder.Property(entity => entity.Text).HasMaxLength(8000).IsRequired();

        _ = builder.HasIndex(entity => entity.CaseId);
    }
}
