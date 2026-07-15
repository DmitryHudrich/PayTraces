namespace Ledgerscope.Accounts.Application.Cases;

/// <summary>
/// Names of case-scoped roles referenced from code (e.g. the creator becomes
/// the Lead). These must exist as scope=Case roles in the authorization
/// config file.
/// </summary>
public static class CaseRoles {
    public const String Lead = "Lead";
    public const String Collaborator = "Collaborator";
}
