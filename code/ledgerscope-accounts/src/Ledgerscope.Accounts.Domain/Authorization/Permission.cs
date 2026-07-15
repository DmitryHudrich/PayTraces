namespace Ledgerscope.Accounts.Domain.Authorization;

/// <summary>
/// Atomic, code-defined unit of authority. The set of <em>possible</em>
/// permissions is fixed here; which permissions each role grants is
/// configured in a file (see the Application layer's AuthorizationOptions),
/// so the policy can change without a redeploy. Every command/query's
/// required permission is drawn from this enum.
/// </summary>
public enum Permission {
    CaseCreate,
    CaseRead,
    CaseUpdate,
    CaseClose,
    CaseAssign,
    CaseDelete,
    CaseAddressAdd,
    CaseAddressRemove,
    CaseNoteAdd,
    ViewCreate,
    ViewRead,
    ViewUpdate,
    ViewDelete,
    ViewManageSharing,
    LabelCreate,
    LabelApply,
    GroupCreate,
    GroupUpdate,
    GroupDelete,
    UserManage,
    RoleManage,
}
