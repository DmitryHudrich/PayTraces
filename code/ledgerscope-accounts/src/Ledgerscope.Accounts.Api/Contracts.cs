using Ledgerscope.Accounts.Domain.Cases;

namespace Ledgerscope.Accounts.Api;

public sealed record CreateCaseRequest(String Title, String? Description, CasePriority Priority);

public sealed record AssignCaseRequest(Guid UserId, String RoleName);

public sealed record AddAddressRequest(String Address, Int32 ChainId, String? Note);

public sealed record RegisterRequest(String Email, String Password, String? DisplayName);

public sealed record LoginRequest(String Email, String Password);

public sealed record AssignGlobalRoleRequest(String RoleName);

public sealed record CreateViewRequest(String Name, Boolean IsShared);

public sealed record RenameViewRequest(String Name);

public sealed record SetViewSharingRequest(Boolean IsShared);

public sealed record PinNodeRequest(String Address, Double X, Double Y);

public sealed record CreateLabelRequest(String Text, String? Color);

public sealed record ApplyLabelRequest(Guid LabelId, String Address, Int32 ChainId);

public sealed record CreateGroupRequest(String Name, IReadOnlyList<GroupMemberRequest>? Members);

public sealed record GroupMemberRequest(String Address, Int32 ChainId);

public sealed record RenameGroupRequest(String Name);

public sealed record GroupMemberBody(String Address, Int32 ChainId);

public sealed record IngestRequest(
    String Address, Int32 ChainId, Int64? FromBlock, Int64? ToBlock, Int32? MaxDepth, Int32? MaxNodes);
