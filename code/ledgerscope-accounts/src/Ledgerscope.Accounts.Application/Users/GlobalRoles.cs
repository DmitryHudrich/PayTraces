namespace Ledgerscope.Accounts.Application.Users;

/// <summary>
/// Names of global (organization-wide) roles referenced from code. These must
/// exist as scope=Global roles in the authorization config file.
/// </summary>
public static class GlobalRoles {
    public const String OrgAdmin = "OrgAdmin";
    public const String Investigator = "Investigator";
    public const String Viewer = "Viewer";
}
