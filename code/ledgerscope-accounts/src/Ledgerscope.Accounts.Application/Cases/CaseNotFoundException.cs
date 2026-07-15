namespace Ledgerscope.Accounts.Application.Cases;

/// <summary>
/// Thrown when a command/query targets a case id that does not exist. The API
/// layer maps this to 404.
/// </summary>
public sealed class CaseNotFoundException(Guid caseId) : Exception($"Case {caseId} was not found.") {
    public Guid CaseId { get; } = caseId;
}
