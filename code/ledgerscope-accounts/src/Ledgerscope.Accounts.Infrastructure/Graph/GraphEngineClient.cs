using System.Net.Http.Json;
using System.Text.Json;

using Ledgerscope.Accounts.Application.Graph;

namespace Ledgerscope.Accounts.Infrastructure.Graph;

/// <summary>
/// Typed HttpClient over the Rust engine's internal REST API. The base address,
/// shared secret, and timeout are supplied by DI from
/// <see cref="GraphEngineOptions"/>. The engine speaks snake_case JSON.
/// </summary>
public sealed class GraphEngineClient(HttpClient http) : IGraphEngineClient {
    private static readonly JsonSerializerOptions Json = new() {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        PropertyNameCaseInsensitive = true,
        // Omit null optionals (from_block/to_block/…) so the engine falls back
        // to its own defaults instead of receiving explicit nulls.
        DefaultIgnoreCondition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingNull,
    };

    private readonly HttpClient http = http;

    public async Task<IReadOnlyList<GraphNode>> GetNodesBatchAsync(
        IReadOnlyCollection<String> addresses, Int32 chainId, CancellationToken ct) {
        if (addresses.Count == 0) {
            return [];
        }

        var joined = Uri.EscapeDataString(String.Join(',', addresses));
        var path = $"/nodes/batch?addresses={joined}&chain_id={chainId}";

        try {
            using var response = await http.GetAsync(path, ct);
            _ = response.EnsureSuccessStatusCode();
            var payload = await response.Content.ReadFromJsonAsync<NodesBatchResponse>(Json, ct);
            return payload?.Nodes ?? [];
        } catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException) {
            throw new GraphEngineException($"Graph engine request failed ({path}).", ex);
        }
    }

    public async Task<GraphPageDto> GetGraphPageAsync(
        String address, Int32 chainId, Int32 maxDepth, Int32 page, Int32 pageSize, CancellationToken ct) {
        var path = $"/graph?address={Uri.EscapeDataString(address)}&chain_id={chainId}"
            + $"&max_depth={maxDepth}&page={page}&page_size={pageSize}";

        try {
            using var response = await http.GetAsync(path, ct);
            _ = response.EnsureSuccessStatusCode();
            var payload = await response.Content.ReadFromJsonAsync<GraphPageDto>(Json, ct);
            return payload ?? throw new GraphEngineException($"Graph engine returned empty body ({path}).");
        } catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException) {
            throw new GraphEngineException($"Graph engine request failed ({path}).", ex);
        }
    }

    public async Task<IngestAcceptedDto> CreateIngestJobAsync(
        String address, Int32 chainId, Int64? fromBlock, Int64? toBlock, Int32? maxDepth, Int32? maxNodes,
        CancellationToken ct) {
        var body = new IngestBody(address, chainId, maxDepth, maxNodes, fromBlock, toBlock);
        try {
            using var response = await http.PostAsJsonAsync("/jobs/ingest", body, Json, ct);
            _ = response.EnsureSuccessStatusCode();
            var payload = await response.Content.ReadFromJsonAsync<IngestAcceptedDto>(Json, ct);
            return payload ?? throw new GraphEngineException("Graph engine returned empty ingest response.");
        } catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException) {
            throw new GraphEngineException("Graph engine ingest request failed.", ex);
        }
    }

    public Task<JobStatusDto> GetJobStatusAsync(String jobId, CancellationToken ct) =>
        GetAsync<JobStatusDto>($"/jobs/{Uri.EscapeDataString(jobId)}", ct);

    public Task<ScoreDto> GetScoreAsync(String address, Int32 chainId, CancellationToken ct) =>
        GetAsync<ScoreDto>($"/score?address={Uri.EscapeDataString(address)}&chain_id={chainId}", ct);

    public Task<HeuristicsDto> GetHeuristicsAsync(String address, Int32 chainId, CancellationToken ct) =>
        GetAsync<HeuristicsDto>($"/heuristics?address={Uri.EscapeDataString(address)}&chain_id={chainId}", ct);

    public Task<ClusterDto> GetClusterAsync(String address, Int32 chainId, CancellationToken ct) =>
        GetAsync<ClusterDto>($"/cluster?address={Uri.EscapeDataString(address)}&chain_id={chainId}", ct);

    public async Task<AddressEntityDto?> GetAddressEntityAsync(String address, Int32 chainId, CancellationToken ct) {
        var path = $"/labels/{Uri.EscapeDataString(address)}?chain_id={chainId}";
        try {
            using var response = await http.GetAsync(path, ct);
            // The engine answers 400/404 when no entity exists for the address —
            // treat "no automatic labels" as a normal, empty result.
            if (response.StatusCode is System.Net.HttpStatusCode.NotFound
                or System.Net.HttpStatusCode.BadRequest) {
                return null;
            }

            _ = response.EnsureSuccessStatusCode();
            return await response.Content.ReadFromJsonAsync<AddressEntityDto>(Json, ct);
        } catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException) {
            throw new GraphEngineException($"Graph engine request failed ({path}).", ex);
        }
    }

    private async Task<T> GetAsync<T>(String path, CancellationToken ct) {
        try {
            using var response = await http.GetAsync(path, ct);
            _ = response.EnsureSuccessStatusCode();
            var payload = await response.Content.ReadFromJsonAsync<T>(Json, ct);
            return payload ?? throw new GraphEngineException($"Graph engine returned empty body ({path}).");
        } catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException) {
            throw new GraphEngineException($"Graph engine request failed ({path}).", ex);
        }
    }

    private sealed record NodesBatchResponse(GraphNode[] Nodes);

    private sealed record IngestBody(
        String Address, Int32 ChainId, Int32? MaxDepth, Int32? MaxNodes, Int64? FromBlock, Int64? ToBlock);
}
