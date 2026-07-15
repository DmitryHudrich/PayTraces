using System;
using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace Ledgerscope.Accounts.Infrastructure.Migrations
{
    /// <inheritdoc />
    public partial class AddLabelsAndGroups : Migration
    {
        /// <inheritdoc />
        protected override void Up(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.CreateTable(
                name: "address_groups",
                schema: "accounts",
                columns: table => new
                {
                    Id = table.Column<Guid>(type: "uuid", nullable: false),
                    CaseId = table.Column<Guid>(type: "uuid", nullable: false),
                    CreatedBy = table.Column<Guid>(type: "uuid", nullable: false),
                    Name = table.Column<string>(type: "character varying(200)", maxLength: 200, nullable: false),
                    CreatedAt = table.Column<DateTimeOffset>(type: "timestamp with time zone", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_address_groups", x => x.Id);
                });

            migrationBuilder.CreateTable(
                name: "custom_labels",
                schema: "accounts",
                columns: table => new
                {
                    Id = table.Column<Guid>(type: "uuid", nullable: false),
                    CaseId = table.Column<Guid>(type: "uuid", nullable: true),
                    CreatedBy = table.Column<Guid>(type: "uuid", nullable: false),
                    Text = table.Column<string>(type: "character varying(200)", maxLength: 200, nullable: false),
                    Color = table.Column<string>(type: "character varying(32)", maxLength: 32, nullable: true),
                    CreatedAt = table.Column<DateTimeOffset>(type: "timestamp with time zone", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_custom_labels", x => x.Id);
                });

            migrationBuilder.CreateTable(
                name: "address_group_members",
                schema: "accounts",
                columns: table => new
                {
                    Id = table.Column<Guid>(type: "uuid", nullable: false),
                    GroupId = table.Column<Guid>(type: "uuid", nullable: false),
                    Address = table.Column<string>(type: "character varying(128)", maxLength: 128, nullable: false),
                    ChainId = table.Column<int>(type: "integer", nullable: false),
                    AddedAt = table.Column<DateTimeOffset>(type: "timestamp with time zone", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_address_group_members", x => x.Id);
                    table.ForeignKey(
                        name: "FK_address_group_members_address_groups_GroupId",
                        column: x => x.GroupId,
                        principalSchema: "accounts",
                        principalTable: "address_groups",
                        principalColumn: "Id",
                        onDelete: ReferentialAction.Cascade);
                });

            migrationBuilder.CreateTable(
                name: "address_label_links",
                schema: "accounts",
                columns: table => new
                {
                    Id = table.Column<Guid>(type: "uuid", nullable: false),
                    LabelId = table.Column<Guid>(type: "uuid", nullable: false),
                    Address = table.Column<string>(type: "character varying(128)", maxLength: 128, nullable: false),
                    ChainId = table.Column<int>(type: "integer", nullable: false),
                    AppliedBy = table.Column<Guid>(type: "uuid", nullable: false),
                    AppliedAt = table.Column<DateTimeOffset>(type: "timestamp with time zone", nullable: false)
                },
                constraints: table =>
                {
                    table.PrimaryKey("PK_address_label_links", x => x.Id);
                    table.ForeignKey(
                        name: "FK_address_label_links_custom_labels_LabelId",
                        column: x => x.LabelId,
                        principalSchema: "accounts",
                        principalTable: "custom_labels",
                        principalColumn: "Id",
                        onDelete: ReferentialAction.Cascade);
                });

            migrationBuilder.CreateIndex(
                name: "IX_address_group_members_GroupId_Address_ChainId",
                schema: "accounts",
                table: "address_group_members",
                columns: new[] { "GroupId", "Address", "ChainId" },
                unique: true);

            migrationBuilder.CreateIndex(
                name: "IX_address_groups_CaseId",
                schema: "accounts",
                table: "address_groups",
                column: "CaseId");

            migrationBuilder.CreateIndex(
                name: "IX_address_label_links_Address_ChainId",
                schema: "accounts",
                table: "address_label_links",
                columns: new[] { "Address", "ChainId" });

            migrationBuilder.CreateIndex(
                name: "IX_address_label_links_LabelId_Address_ChainId",
                schema: "accounts",
                table: "address_label_links",
                columns: new[] { "LabelId", "Address", "ChainId" },
                unique: true);

            migrationBuilder.CreateIndex(
                name: "IX_custom_labels_CaseId",
                schema: "accounts",
                table: "custom_labels",
                column: "CaseId");
        }

        /// <inheritdoc />
        protected override void Down(MigrationBuilder migrationBuilder)
        {
            migrationBuilder.DropTable(
                name: "address_group_members",
                schema: "accounts");

            migrationBuilder.DropTable(
                name: "address_label_links",
                schema: "accounts");

            migrationBuilder.DropTable(
                name: "address_groups",
                schema: "accounts");

            migrationBuilder.DropTable(
                name: "custom_labels",
                schema: "accounts");
        }
    }
}
