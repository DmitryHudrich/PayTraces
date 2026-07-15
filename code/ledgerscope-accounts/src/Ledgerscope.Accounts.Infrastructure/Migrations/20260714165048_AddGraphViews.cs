using System;
using Microsoft.EntityFrameworkCore.Migrations;

#nullable disable

namespace Ledgerscope.Accounts.Infrastructure.Migrations; 
/// <inheritdoc />
public partial class AddGraphViews : Migration {
    /// <inheritdoc />
    protected override void Up(MigrationBuilder migrationBuilder) {
        _ = migrationBuilder.CreateTable(
            name: "graph_views",
            schema: "accounts",
            columns: table => new {
                Id = table.Column<Guid>(type: "uuid", nullable: false),
                CaseId = table.Column<Guid>(type: "uuid", nullable: false),
                Name = table.Column<String>(type: "character varying(200)", maxLength: 200, nullable: false),
                CreatedBy = table.Column<Guid>(type: "uuid", nullable: false),
                CreatedAt = table.Column<DateTimeOffset>(type: "timestamp with time zone", nullable: false),
                IsShared = table.Column<Boolean>(type: "boolean", nullable: false)
            },
            constraints: table => _ = table.PrimaryKey("PK_graph_views", x => x.Id));

        _ = migrationBuilder.CreateTable(
            name: "graph_node_positions",
            schema: "accounts",
            columns: table => new {
                Id = table.Column<Guid>(type: "uuid", nullable: false),
                ViewId = table.Column<Guid>(type: "uuid", nullable: false),
                Address = table.Column<String>(type: "character varying(128)", maxLength: 128, nullable: false),
                X = table.Column<Double>(type: "double precision", nullable: false),
                Y = table.Column<Double>(type: "double precision", nullable: false),
                PinnedBy = table.Column<Guid>(type: "uuid", nullable: false),
                PinnedAt = table.Column<DateTimeOffset>(type: "timestamp with time zone", nullable: false)
            },
            constraints: table => {
                _ = table.PrimaryKey("PK_graph_node_positions", x => x.Id);
                _ = table.ForeignKey(
                    name: "FK_graph_node_positions_graph_views_ViewId",
                    column: x => x.ViewId,
                    principalSchema: "accounts",
                    principalTable: "graph_views",
                    principalColumn: "Id",
                    onDelete: ReferentialAction.Cascade);
            });

        _ = migrationBuilder.CreateIndex(
            name: "IX_graph_node_positions_ViewId_Address",
            schema: "accounts",
            table: "graph_node_positions",
            columns: ["ViewId", "Address"],
            unique: true);

        _ = migrationBuilder.CreateIndex(
            name: "IX_graph_views_CaseId",
            schema: "accounts",
            table: "graph_views",
            column: "CaseId");
    }

    /// <inheritdoc />
    protected override void Down(MigrationBuilder migrationBuilder) {
        _ = migrationBuilder.DropTable(
            name: "graph_node_positions",
            schema: "accounts");

        _ = migrationBuilder.DropTable(
            name: "graph_views",
            schema: "accounts");
    }
}
