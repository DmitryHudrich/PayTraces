using System.Text;
using System.Text.Json.Serialization;
using Ledgerscope.Accounts.Api;
using Ledgerscope.Accounts.Api.Dev;
using Ledgerscope.Accounts.Application;
using Ledgerscope.Accounts.Application.Auth;
using Ledgerscope.Accounts.Application.Authorization;
using Ledgerscope.Accounts.Application.Cases;
using Ledgerscope.Accounts.Application.Cases.Commands;
using Ledgerscope.Accounts.Application.Cases.Queries;
using Ledgerscope.Accounts.Application.Graph;
using Ledgerscope.Accounts.Application.Groups;
using Ledgerscope.Accounts.Application.Groups.Commands;
using Ledgerscope.Accounts.Application.Groups.Queries;
using Ledgerscope.Accounts.Application.Labels;
using Ledgerscope.Accounts.Application.Labels.Commands;
using Ledgerscope.Accounts.Application.Labels.Queries;
using Ledgerscope.Accounts.Application.Users;
using Ledgerscope.Accounts.Application.Users.Commands;
using Ledgerscope.Accounts.Application.Users.Queries;
using Ledgerscope.Accounts.Application.Views;
using Ledgerscope.Accounts.Application.Views.Commands;
using Ledgerscope.Accounts.Application.Views.Queries;
using Ledgerscope.Accounts.Infrastructure;
using Ledgerscope.Accounts.Infrastructure.Auth;
using Ledgerscope.Accounts.Infrastructure.Persistence;
using MediatR;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.EntityFrameworkCore;
using Microsoft.IdentityModel.Tokens;
using Scalar.AspNetCore;

var builder = WebApplication.CreateBuilder(args);

// File-driven authorization policy (role -> permissions). Kept in its own
// file so the "which role can do what" mapping can change without touching
// appsettings or redeploying code.
builder.Configuration.AddJsonFile("authorization.json", optional: false, reloadOnChange: true);

builder.Services.AddLedgerscopeApplication(builder.Configuration);
builder.Services.AddLedgerscopeInfrastructure(builder.Configuration);

// Enum values as strings in request/response JSON (e.g. priority "High").
builder.Services.ConfigureHttpJsonOptions(options =>
    options.SerializerOptions.Converters.Add(new JsonStringEnumConverter()));

// The authenticated caller (from the JWT) drives authorization.
builder.Services.AddHttpContextAccessor();
builder.Services.AddScoped<IUserContext, JwtUserContext>();

var jwt = builder.Configuration.GetSection(JwtOptions.SectionName).Get<JwtOptions>()
    ?? throw new InvalidOperationException("Missing 'Jwt' configuration section.");

builder.Services.AddAuthentication(JwtBearerDefaults.AuthenticationScheme)
    .AddJwtBearer(options => {
        options.MapInboundClaims = false;
        options.TokenValidationParameters = new TokenValidationParameters {
            ValidateIssuer = true,
            ValidIssuer = jwt.Issuer,
            ValidateAudience = true,
            ValidAudience = jwt.Audience,
            ValidateIssuerSigningKey = true,
            IssuerSigningKey = new SymmetricSecurityKey(Encoding.UTF8.GetBytes(jwt.SigningKey)),
            ValidateLifetime = true,
            NameClaimType = "sub",
        };
        // WebSocket clients can't set an Authorization header, so SignalR passes
        // the JWT in the query string for hub connections.
        options.Events = new JwtBearerEvents {
            OnMessageReceived = context => {
                var accessToken = context.Request.Query["access_token"];
                if (!String.IsNullOrEmpty(accessToken)
                    && context.HttpContext.Request.Path.StartsWithSegments("/hubs")) {
                    context.Token = accessToken;
                }

                return Task.CompletedTask;
            },
        };
    });
builder.Services.AddAuthorization();

// OpenAPI document for the frontend contract, surfaced via Scalar below.
builder.Services.AddOpenApi();

// Progressive/live graph delivery to the frontend.
builder.Services.AddSignalR();

var app = builder.Build();

// Frontend contract carries X-API-Version; resolve/validate it before anything
// else and echo it back.
app.UseMiddleware<ApiVersionMiddleware>();

// Translate authorization/domain exceptions to HTTP status codes.
app.Use(async (context, next) => {
    try {
        await next();
    } catch (InvalidCredentialsException ex) {
        context.Response.StatusCode = StatusCodes.Status401Unauthorized;
        await context.Response.WriteAsJsonAsync(new { error = ex.Message });
    } catch (ForbiddenException ex) {
        context.Response.StatusCode = StatusCodes.Status403Forbidden;
        await context.Response.WriteAsJsonAsync(new { error = ex.Message });
    } catch (EmailAlreadyInUseException ex) {
        context.Response.StatusCode = StatusCodes.Status409Conflict;
        await context.Response.WriteAsJsonAsync(new { error = ex.Message });
    } catch (CaseNotFoundException ex) {
        context.Response.StatusCode = StatusCodes.Status404NotFound;
        await context.Response.WriteAsJsonAsync(new { error = ex.Message });
    } catch (UserNotFoundException ex) {
        context.Response.StatusCode = StatusCodes.Status404NotFound;
        await context.Response.WriteAsJsonAsync(new { error = ex.Message });
    } catch (ViewNotFoundException ex) {
        context.Response.StatusCode = StatusCodes.Status404NotFound;
        await context.Response.WriteAsJsonAsync(new { error = ex.Message });
    } catch (LabelNotFoundException ex) {
        context.Response.StatusCode = StatusCodes.Status404NotFound;
        await context.Response.WriteAsJsonAsync(new { error = ex.Message });
    } catch (GroupNotFoundException ex) {
        context.Response.StatusCode = StatusCodes.Status404NotFound;
        await context.Response.WriteAsJsonAsync(new { error = ex.Message });
    } catch (GraphEngineException ex) {
        context.Response.StatusCode = StatusCodes.Status502BadGateway;
        await context.Response.WriteAsJsonAsync(new { error = ex.Message });
    }
});

app.UseAuthentication();
app.UseAuthorization();

// Dev only: apply migrations and seed a demo org/user so the stack is
// exercisable.
using (var scope = app.Services.CreateScope()) {
    var db = scope.ServiceProvider.GetRequiredService<AccountsDbContext>();
    await db.Database.MigrateAsync();
    await DevSeed.EnsureAsync(db);
}

app.MapGet("/", () => "Ledgerscope.Accounts");

// OpenAPI JSON at /openapi/v1.json + Scalar API reference UI at /scalar.
app.MapOpenApi();
app.MapScalarApiReference();

// Progressive/live graph hub (frontend connects with its JWT).
app.MapHub<GraphHub>("/hubs/graph");

app.MapPost("/auth/register", async (ISender sender, RegisterRequest body) => {
    var id = await sender.Send(new RegisterUserCommand(
        body.Email, body.Password, body.DisplayName ?? body.Email, DevSeed.DevOrgId));
    return Results.Created($"/users/{id}", new { id });
});

app.MapPost("/auth/login", async (ISender sender, LoginRequest body) => {
    var token = await sender.Send(new LoginCommand(body.Email, body.Password));
    return Results.Ok(new { access_token = token.Token, expires_at = token.ExpiresAt });
});

// Dogfoods the authorization stack: effective permissions for the caller,
// optionally within a case.
app.MapGet("/me/permissions", async (ISender sender, Guid? caseId) =>
    Results.Ok(await sender.Send(new GetMyPermissionsQuery(caseId))))
    .RequireAuthorization();

app.MapPost("/cases", async (ISender sender, CreateCaseRequest body) => {
    var id = await sender.Send(new CreateCaseCommand(
        body.Title, body.Description ?? String.Empty, body.Priority, DevSeed.DevOrgId));
    return Results.Created($"/cases/{id}", new { id });
}).RequireAuthorization();

app.MapGet("/cases", async (ISender sender) =>
    Results.Ok(await sender.Send(new ListCasesQuery(DevSeed.DevOrgId))))
    .RequireAuthorization();

app.MapGet("/cases/{id:guid}", async (ISender sender, Guid id) => {
    var dto = await sender.Send(new GetCaseByIdQuery(id));
    return dto is null ? Results.NotFound() : Results.Ok(dto);
}).RequireAuthorization();

app.MapPost("/cases/{id:guid}/close", async (ISender sender, Guid id) => {
    await sender.Send(new CloseCaseCommand(id));
    return Results.NoContent();
}).RequireAuthorization();

app.MapPost("/cases/{id:guid}/assign", async (ISender sender, Guid id, AssignCaseRequest body) => {
    await sender.Send(new AssignCaseToUserCommand(id, body.UserId, body.RoleName));
    return Results.NoContent();
}).RequireAuthorization();

app.MapPost("/cases/{id:guid}/addresses", async (ISender sender, Guid id, AddAddressRequest body) => {
    await sender.Send(new AddAddressToCaseCommand(id, body.Address, body.ChainId, body.Note));
    return Results.NoContent();
}).RequireAuthorization();

// BFF: the case's addresses enriched with live node data from the Rust engine.
app.MapGet("/cases/{id:guid}/graph", async (ISender sender, Guid id) =>
    Results.Ok(await sender.Send(new GetCaseGraphQuery(id))))
    .RequireAuthorization();

// Ingest: pull on-chain data (optionally a block range) from the external
// provider into the engine's store, then poll the job to completion.
app.MapPost("/cases/{caseId:guid}/ingest", async (ISender sender, Guid caseId, IngestRequest body) => {
    var result = await sender.Send(new StartIngestCommand(
        caseId, body.Address, body.ChainId, body.FromBlock, body.ToBlock, body.MaxDepth, body.MaxNodes));
    return Results.Accepted($"/cases/{caseId}/jobs/{result.JobId}", result);
}).RequireAuthorization();

app.MapGet("/cases/{caseId:guid}/jobs/{jobId}", async (ISender sender, Guid caseId, String jobId) =>
    Results.Ok(await sender.Send(new GetIngestJobQuery(caseId, jobId))))
    .RequireAuthorization();

// Per-address engine insights (risk score, behavioural heuristics, co-ownership
// cluster, and the engine's automatic entity tags).
app.MapGet("/cases/{caseId:guid}/addresses/{chainId:int}/{address}/score",
    async (ISender sender, Guid caseId, Int32 chainId, String address) =>
        Results.Ok(await sender.Send(new GetAddressScoreQuery(caseId, address, chainId))))
    .RequireAuthorization();

app.MapGet("/cases/{caseId:guid}/addresses/{chainId:int}/{address}/heuristics",
    async (ISender sender, Guid caseId, Int32 chainId, String address) =>
        Results.Ok(await sender.Send(new GetAddressHeuristicsQuery(caseId, address, chainId))))
    .RequireAuthorization();

app.MapGet("/cases/{caseId:guid}/addresses/{chainId:int}/{address}/cluster",
    async (ISender sender, Guid caseId, Int32 chainId, String address) =>
        Results.Ok(await sender.Send(new GetAddressClusterQuery(caseId, address, chainId))))
    .RequireAuthorization();

app.MapGet("/cases/{caseId:guid}/addresses/{chainId:int}/{address}/entity",
    async (ISender sender, Guid caseId, Int32 chainId, String address) =>
        Results.Ok(await sender.Send(new GetAddressEntityQuery(caseId, address, chainId))))
    .RequireAuthorization();

// Canvas views: named arrangements of pinned node positions over a case's graph.
app.MapGet("/cases/{caseId:guid}/views", async (ISender sender, Guid caseId) =>
    Results.Ok(await sender.Send(new ListCaseViewsQuery(caseId))))
    .RequireAuthorization();

app.MapPost("/cases/{caseId:guid}/views", async (ISender sender, Guid caseId, CreateViewRequest body) => {
    var id = await sender.Send(new CreateGraphViewCommand(caseId, body.Name, body.IsShared));
    return Results.Created($"/cases/{caseId}/views/{id}", new { id });
}).RequireAuthorization();

app.MapGet("/cases/{caseId:guid}/views/{viewId:guid}", async (ISender sender, Guid caseId, Guid viewId) =>
    Results.Ok(await sender.Send(new GetGraphViewQuery(caseId, viewId))))
    .RequireAuthorization();

app.MapPut("/cases/{caseId:guid}/views/{viewId:guid}", async (ISender sender, Guid caseId, Guid viewId, RenameViewRequest body) => {
    await sender.Send(new RenameGraphViewCommand(caseId, viewId, body.Name));
    return Results.NoContent();
}).RequireAuthorization();

app.MapPut("/cases/{caseId:guid}/views/{viewId:guid}/sharing", async (ISender sender, Guid caseId, Guid viewId, SetViewSharingRequest body) => {
    await sender.Send(new SetViewSharingCommand(caseId, viewId, body.IsShared));
    return Results.NoContent();
}).RequireAuthorization();

app.MapDelete("/cases/{caseId:guid}/views/{viewId:guid}", async (ISender sender, Guid caseId, Guid viewId) => {
    await sender.Send(new DeleteGraphViewCommand(caseId, viewId));
    return Results.NoContent();
}).RequireAuthorization();

app.MapPut("/cases/{caseId:guid}/views/{viewId:guid}/nodes", async (ISender sender, Guid caseId, Guid viewId, PinNodeRequest body) => {
    await sender.Send(new PinNodeCommand(caseId, viewId, body.Address, body.X, body.Y));
    return Results.NoContent();
}).RequireAuthorization();

app.MapDelete("/cases/{caseId:guid}/views/{viewId:guid}/nodes/{address}", async (ISender sender, Guid caseId, Guid viewId, String address) => {
    await sender.Send(new UnpinNodeCommand(caseId, viewId, address));
    return Results.NoContent();
}).RequireAuthorization();

// Custom labels: the investigation's own annotations on addresses (distinct
// from the engine's authoritative labelling).
app.MapGet("/cases/{caseId:guid}/labels", async (ISender sender, Guid caseId) =>
    Results.Ok(await sender.Send(new ListCaseLabelsQuery(caseId))))
    .RequireAuthorization();

app.MapPost("/cases/{caseId:guid}/labels", async (ISender sender, Guid caseId, CreateLabelRequest body) => {
    var id = await sender.Send(new CreateLabelCommand(caseId, body.Text, body.Color));
    return Results.Created($"/cases/{caseId}/labels/{id}", new { id });
}).RequireAuthorization();

app.MapDelete("/cases/{caseId:guid}/labels/{labelId:guid}", async (ISender sender, Guid caseId, Guid labelId) => {
    await sender.Send(new DeleteLabelCommand(caseId, labelId));
    return Results.NoContent();
}).RequireAuthorization();

app.MapPost("/cases/{caseId:guid}/labels/apply", async (ISender sender, Guid caseId, ApplyLabelRequest body) => {
    await sender.Send(new ApplyLabelCommand(caseId, body.LabelId, body.Address, body.ChainId));
    return Results.NoContent();
}).RequireAuthorization();

app.MapDelete("/cases/{caseId:guid}/labels/{labelId:guid}/addresses/{chainId:int}/{address}",
    async (ISender sender, Guid caseId, Guid labelId, Int32 chainId, String address) => {
        await sender.Send(new RemoveLabelFromAddressCommand(caseId, labelId, address, chainId));
        return Results.NoContent();
    }).RequireAuthorization();

app.MapGet("/cases/{caseId:guid}/addresses/{chainId:int}/{address}/labels",
    async (ISender sender, Guid caseId, Int32 chainId, String address) =>
        Results.Ok(await sender.Send(new ListAddressLabelsQuery(caseId, address, chainId))))
    .RequireAuthorization();

// Address groups: stable, user-curated sets that carry labels (the engine's
// /cluster has no stable id).
app.MapGet("/cases/{caseId:guid}/groups", async (ISender sender, Guid caseId) =>
    Results.Ok(await sender.Send(new ListCaseGroupsQuery(caseId))))
    .RequireAuthorization();

app.MapPost("/cases/{caseId:guid}/groups", async (ISender sender, Guid caseId, CreateGroupRequest body) => {
    var members = body.Members is null
        ? []
        : body.Members.Select(m => new GroupMemberInput(m.Address, m.ChainId)).ToArray();
    var id = await sender.Send(new CreateGroupCommand(caseId, body.Name, members));
    return Results.Created($"/cases/{caseId}/groups/{id}", new { id });
}).RequireAuthorization();

app.MapGet("/cases/{caseId:guid}/groups/{groupId:guid}", async (ISender sender, Guid caseId, Guid groupId) =>
    Results.Ok(await sender.Send(new GetGroupQuery(caseId, groupId))))
    .RequireAuthorization();

app.MapPut("/cases/{caseId:guid}/groups/{groupId:guid}", async (ISender sender, Guid caseId, Guid groupId, RenameGroupRequest body) => {
    await sender.Send(new RenameGroupCommand(caseId, groupId, body.Name));
    return Results.NoContent();
}).RequireAuthorization();

app.MapDelete("/cases/{caseId:guid}/groups/{groupId:guid}", async (ISender sender, Guid caseId, Guid groupId) => {
    await sender.Send(new DeleteGroupCommand(caseId, groupId));
    return Results.NoContent();
}).RequireAuthorization();

app.MapPost("/cases/{caseId:guid}/groups/{groupId:guid}/members", async (ISender sender, Guid caseId, Guid groupId, GroupMemberBody body) => {
    await sender.Send(new AddGroupMemberCommand(caseId, groupId, body.Address, body.ChainId));
    return Results.NoContent();
}).RequireAuthorization();

app.MapDelete("/cases/{caseId:guid}/groups/{groupId:guid}/members/{chainId:int}/{address}",
    async (ISender sender, Guid caseId, Guid groupId, Int32 chainId, String address) => {
        await sender.Send(new RemoveGroupMemberCommand(caseId, groupId, address, chainId));
        return Results.NoContent();
    }).RequireAuthorization();

app.MapPost("/users/{id:guid}/global-roles", async (ISender sender, Guid id, AssignGlobalRoleRequest body) => {
    await sender.Send(new AssignGlobalRoleCommand(id, body.RoleName));
    return Results.NoContent();
}).RequireAuthorization();

app.Run();
