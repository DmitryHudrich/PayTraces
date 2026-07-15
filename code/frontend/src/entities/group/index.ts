export type { AddressGroup, AddressGroupSummary, GroupMember } from '@/entities/group/model/group'
export {
  addGroupMember,
  createGroup,
  deleteGroup,
  getGroup,
  listGroups,
  removeGroupMember,
  renameGroup,
} from '@/entities/group/api/groups'
export {
  groupKeys,
  useAddGroupMemberMutation,
  useCreateGroupMutation,
  useDeleteGroupMutation,
  useGroupQuery,
  useGroupsQuery,
  useRemoveGroupMemberMutation,
  useRenameGroupMutation,
} from '@/entities/group/api/queries'
