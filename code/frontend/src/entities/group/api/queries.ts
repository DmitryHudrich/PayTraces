import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import {
  addGroupMember,
  createGroup,
  deleteGroup,
  getGroup,
  listGroups,
  removeGroupMember,
  renameGroup,
} from '@/entities/group/api/groups'

export const groupKeys = {
  all: (caseId: string) => ['groups', caseId] as const,
  list: (caseId: string) => ['groups', caseId, 'list'] as const,
  detail: (caseId: string, groupId: string) => ['groups', caseId, 'detail', groupId] as const,
}

export function useGroupsQuery(caseId: string | undefined) {
  return useQuery({
    queryKey: groupKeys.list(caseId ?? ''),
    queryFn: () => listGroups(caseId as string),
    enabled: Boolean(caseId),
  })
}

export function useGroupQuery(caseId: string | undefined, groupId: string | undefined) {
  return useQuery({
    queryKey: groupKeys.detail(caseId ?? '', groupId ?? ''),
    queryFn: () => getGroup(caseId as string, groupId as string),
    enabled: Boolean(caseId && groupId),
  })
}

export function useCreateGroupMutation(caseId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: { name: string; members?: { address: string; chainId: number }[] }) =>
      createGroup(caseId, input),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: groupKeys.list(caseId) }),
  })
}

export function useRenameGroupMutation(caseId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: { groupId: string; name: string }) => renameGroup(caseId, input.groupId, input.name),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: groupKeys.all(caseId) }),
  })
}

export function useDeleteGroupMutation(caseId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (groupId: string) => deleteGroup(caseId, groupId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: groupKeys.list(caseId) }),
  })
}

export function useAddGroupMemberMutation(caseId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: { groupId: string; address: string; chainId: number }) =>
      addGroupMember(caseId, input.groupId, { address: input.address, chainId: input.chainId }),
    onSuccess: (_data, variables) => {
      void queryClient.invalidateQueries({ queryKey: groupKeys.detail(caseId, variables.groupId) })
      void queryClient.invalidateQueries({ queryKey: groupKeys.list(caseId) })
    },
  })
}

export function useRemoveGroupMemberMutation(caseId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: { groupId: string; chainId: number; address: string }) =>
      removeGroupMember(caseId, input.groupId, { chainId: input.chainId, address: input.address }),
    onSuccess: (_data, variables) => {
      void queryClient.invalidateQueries({ queryKey: groupKeys.detail(caseId, variables.groupId) })
      void queryClient.invalidateQueries({ queryKey: groupKeys.list(caseId) })
    },
  })
}
