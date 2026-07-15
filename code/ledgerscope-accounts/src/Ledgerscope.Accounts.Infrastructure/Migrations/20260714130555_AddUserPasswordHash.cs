using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace Ledgerscope.Accounts.Infrastructure.Migrations; 
/// <inheritdoc />
public partial class AddUserPasswordHash : Migration {
    /// <inheritdoc />
    protected override void Up(MigrationBuilder migrationBuilder) {
        _ = migrationBuilder.AddColumn<String>(
            name: "PasswordHash",
            schema: "accounts",
            table: "users",
            type: "character varying(512)",
            maxLength: 512,
            nullable: true);
    }

    /// <inheritdoc />
    protected override void Down(MigrationBuilder migrationBuilder) {
        _ = migrationBuilder.DropColumn(
            name: "PasswordHash",
            schema: "accounts",
            table: "users");
    }
}
