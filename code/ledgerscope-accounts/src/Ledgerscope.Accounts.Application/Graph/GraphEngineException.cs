namespace Ledgerscope.Accounts.Application.Graph;

/// <summary>
/// Thrown when the Rust engine is unreachable or returns an error. The API
/// layer maps this to 502 (bad gateway) — the failure is downstream, not the
/// caller's fault.
/// </summary>
public sealed class GraphEngineException(String message, Exception? inner = null) : Exception(message, inner) {
}
