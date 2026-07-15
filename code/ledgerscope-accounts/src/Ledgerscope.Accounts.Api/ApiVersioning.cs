namespace Ledgerscope.Accounts.Api;

/// <summary>
/// The frontend ↔ Accounts contract carries an explicit API version in the
/// <c>X-API-Version</c> header (this lives on the C# side only — the internal
/// Rust boundary dropped versioning). Requests without the header default to
/// the current version; an unsupported value is rejected with 400. The
/// resolved version is echoed back on every response.
/// </summary>
public sealed class ApiVersionMiddleware(RequestDelegate next) {
    public const String HeaderName = "X-API-Version";
    public const String CurrentVersion = "1";

    private static readonly HashSet<String> Supported = new(StringComparer.Ordinal) { "1" };

    private readonly RequestDelegate next = next;

    public async Task InvokeAsync(HttpContext context) {
        var requested = context.Request.Headers[HeaderName].ToString();
        var version = String.IsNullOrWhiteSpace(requested) ? CurrentVersion : requested;

        if (!Supported.Contains(version)) {
            context.Response.StatusCode = StatusCodes.Status400BadRequest;
            await context.Response.WriteAsJsonAsync(new {
                error = $"Unsupported {HeaderName} '{requested}'. Supported: {String.Join(", ", Supported)}.",
            });
            return;
        }

        context.Response.Headers[HeaderName] = version;
        await next(context);
    }
}
