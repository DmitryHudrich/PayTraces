using System;
using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace Ledgerscope.Accounts.Infrastructure.Migrations; 
/// <inheritdoc />
public partial class InitialAccounts : Migration {
    /// <inheritdoc />
    protected override void Up(MigrationBuilder migrationBuilder) {
        _ = migrationBuilder.EnsureSchema(
            name: "accounts");

        _ = migrationBuilder.CreateTable(
            name: "cases",
            schema: "accounts",
            columns: table => new {
                Id = table.Column<Guid>(type: "uuid", nullable: false),
                Title = table.Column<String>(type: "character varying(200)", maxLength: 200, nullable: false),
                Description = table.Column<String>(type: "character varying(4000)", maxLength: 4000, nullable: false),
                Status = table.Column<String>(type: "character varying(20)", maxLength: 20, nullable: false),
                Priority = table.Column<String>(type: "character varying(20)", maxLength: 20, nullable: false),
                OrganizationId = table.Column<Guid>(type: "uuid", nullable: false),
                CreatedBy = table.Column<Guid>(type: "uuid", nullable: false),
                CreatedAt = table.Column<DateTimeOffset>(type: "timestamp with time zone", nullable: false),
                ClosedAt = table.Column<DateTimeOffset>(type: "timestamp with time zone", nullable: true)
            },
            constraints: table => _ = table.PrimaryKey("PK_cases", x => x.Id));

        _ = migrationBuilder.CreateTable(
            name: "user_role_assignments",
            schema: "accounts",
            columns: table => new {
                Id = table.Column<Guid>(type: "uuid", nullable: false),
                UserId = table.Column<Guid>(type: "uuid", nullable: false),
                RoleName = table.Column<String>(type: "character varying(64)", maxLength: 64, nullable: false),
                OrganizationId = table.Column<Guid>(type: "uuid", nullable: false),
                AssignedBy = table.Column<Guid>(type: "uuid", nullable: false),
                AssignedAt = table.Column<DateTimeOffset>(type: "timestamp with time zone", nullable: false)
            },
            constraints: table => _ = table.PrimaryKey("PK_user_role_assignments", x => x.Id));

        _ = migrationBuilder.CreateTable(
            name: "users",
            schema: "accounts",
            columns: table => new {
                Id = table.Column<Guid>(type: "uuid", nullable: false),
                Email = table.Column<String>(type: "character varying(256)", maxLength: 256, nullable: false),
                DisplayName = table.Column<String>(type: "character varying(200)", maxLength: 200, nullable: false),
                OrganizationId = table.Column<Guid>(type: "uuid", nullable: false),
                CreatedAt = table.Column<DateTimeOffset>(type: "timestamp with time zone", nullable: false),
                IsActive = table.Column<Boolean>(type: "boolean", nullable: false)
            },
            constraints: table => _ = table.PrimaryKey("PK_users", x => x.Id));

        _ = migrationBuilder.CreateTable(
            name: "case_addresses",
            schema: "accounts",
            columns: table => new {
                Id = table.Column<Guid>(type: "uuid", nullable: false),
                CaseId = table.Column<Guid>(type: "uuid", nullable: false),
                Address = table.Column<String>(type: "character varying(128)", maxLength: 128, nullable: false),
                ChainId = table.Column<Int32>(type: "integer", nullable: false),
                AddedBy = table.Column<Guid>(type: "uuid", nullable: false),
                AddedAt = table.Column<DateTimeOffset>(type: "timestamp with time zone", nullable: false),
                Note = table.Column<String>(type: "character varying(2000)", maxLength: 2000, nullable: true)
            },
            constraints: table => {
                _ = table.PrimaryKey("PK_case_addresses", x => x.Id);
                _ = table.ForeignKey(
                    name: "FK_case_addresses_cases_CaseId",
                    column: x => x.CaseId,
                    principalSchema: "accounts",
                    principalTable: "cases",
                    principalColumn: "Id",
                    onDelete: ReferentialAction.Cascade);
            });

        _ = migrationBuilder.CreateTable(
            name: "case_assignments",
            schema: "accounts",
            columns: table => new {
                Id = table.Column<Guid>(type: "uuid", nullable: false),
                CaseId = table.Column<Guid>(type: "uuid", nullable: false),
                UserId = table.Column<Guid>(type: "uuid", nullable: false),
                RoleName = table.Column<String>(type: "character varying(64)", maxLength: 64, nullable: false),
                AssignedBy = table.Column<Guid>(type: "uuid", nullable: false),
                AssignedAt = table.Column<DateTimeOffset>(type: "timestamp with time zone", nullable: false)
            },
            constraints: table => {
                _ = table.PrimaryKey("PK_case_assignments", x => x.Id);
                _ = table.ForeignKey(
                    name: "FK_case_assignments_cases_CaseId",
                    column: x => x.CaseId,
                    principalSchema: "accounts",
                    principalTable: "cases",
                    principalColumn: "Id",
                    onDelete: ReferentialAction.Cascade);
            });

        _ = migrationBuilder.CreateTable(
            name: "case_notes",
            schema: "accounts",
            columns: table => new {
                Id = table.Column<Guid>(type: "uuid", nullable: false),
                CaseId = table.Column<Guid>(type: "uuid", nullable: false),
                AuthorId = table.Column<Guid>(type: "uuid", nullable: false),
                Text = table.Column<String>(type: "character varying(8000)", maxLength: 8000, nullable: false),
                CreatedAt = table.Column<DateTimeOffset>(type: "timestamp with time zone", nullable: false)
            },
            constraints: table => {
                _ = table.PrimaryKey("PK_case_notes", x => x.Id);
                _ = table.ForeignKey(
                    name: "FK_case_notes_cases_CaseId",
                    column: x => x.CaseId,
                    principalSchema: "accounts",
                    principalTable: "cases",
                    principalColumn: "Id",
                    onDelete: ReferentialAction.Cascade);
            });

        _ = migrationBuilder.CreateIndex(
            name: "IX_case_addresses_CaseId_Address_ChainId",
            schema: "accounts",
            table: "case_addresses",
            columns: ["CaseId", "Address", "ChainId"],
            unique: true);

        _ = migrationBuilder.CreateIndex(
            name: "IX_case_assignments_CaseId_UserId",
            schema: "accounts",
            table: "case_assignments",
            columns: ["CaseId", "UserId"],
            unique: true);

        _ = migrationBuilder.CreateIndex(
            name: "IX_case_assignments_UserId",
            schema: "accounts",
            table: "case_assignments",
            column: "UserId");

        _ = migrationBuilder.CreateIndex(
            name: "IX_case_notes_CaseId",
            schema: "accounts",
            table: "case_notes",
            column: "CaseId");

        _ = migrationBuilder.CreateIndex(
            name: "IX_cases_OrganizationId",
            schema: "accounts",
            table: "cases",
            column: "OrganizationId");

        _ = migrationBuilder.CreateIndex(
            name: "IX_user_role_assignments_UserId",
            schema: "accounts",
            table: "user_role_assignments",
            column: "UserId");

        _ = migrationBuilder.CreateIndex(
            name: "IX_user_role_assignments_UserId_RoleName_OrganizationId",
            schema: "accounts",
            table: "user_role_assignments",
            columns: ["UserId", "RoleName", "OrganizationId"],
            unique: true);

        _ = migrationBuilder.CreateIndex(
            name: "IX_users_Email",
            schema: "accounts",
            table: "users",
            column: "Email",
            unique: true);

        _ = migrationBuilder.CreateIndex(
            name: "IX_users_OrganizationId",
            schema: "accounts",
            table: "users",
            column: "OrganizationId");
    }

    /// <inheritdoc />
    protected override void Down(MigrationBuilder migrationBuilder) {
        _ = migrationBuilder.DropTable(
            name: "case_addresses",
            schema: "accounts");

        _ = migrationBuilder.DropTable(
            name: "case_assignments",
            schema: "accounts");

        _ = migrationBuilder.DropTable(
            name: "case_notes",
            schema: "accounts");

        _ = migrationBuilder.DropTable(
            name: "user_role_assignments",
            schema: "accounts");

        _ = migrationBuilder.DropTable(
            name: "users",
            schema: "accounts");

        _ = migrationBuilder.DropTable(
            name: "cases",
            schema: "accounts");
    }
}
