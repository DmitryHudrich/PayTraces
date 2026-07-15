namespace Ledgerscope.Accounts.Infrastructure.Graph;

/// <summary>
/// Bound from the "GraphEngine" config section — how to reach the Rust engine's
/// internal API. <see cref="ApiKey"/> is the shared secret presented on every
/// call; production may additionally sit behind mTLS at the network layer.
/// </summary>
public sealed class GraphEngineOptions {
    public const String SectionName = "GraphEngine";

    public String BaseUrl { get; set; } = "http://localhost:8080";
    public String? ApiKey { get; set; }
    public Int32 TimeoutSeconds { get; set; } = 15;
}
