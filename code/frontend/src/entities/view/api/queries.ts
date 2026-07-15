import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import {
  createView,
  deleteView,
  getView,
  listViews,
  renameView,
  setViewSharing,
} from '@/entities/view/api/views'

export const viewKeys = {
  all: (caseId: string) => ['views', caseId] as const,
  list: (caseId: string) => ['views', caseId, 'list'] as const,
  detail: (caseId: string, viewId: string) => ['views', caseId, 'detail', viewId] as const,
}

export function useViewsQuery(caseId: string | undefined) {
  return useQuery({
    queryKey: viewKeys.list(caseId ?? ''),
    queryFn: () => listViews(caseId as string),
    enabled: Boolean(caseId),
  })
}

export function useViewQuery(caseId: string | undefined, viewId: string | undefined) {
  return useQuery({
    queryKey: viewKeys.detail(caseId ?? '', viewId ?? ''),
    queryFn: () => getView(caseId as string, viewId as string),
    enabled: Boolean(caseId && viewId),
  })
}

export function useCreateViewMutation(caseId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: { name: string; isShared: boolean }) => createView(caseId, input),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: viewKeys.list(caseId) }),
  })
}

export function useRenameViewMutation(caseId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: { viewId: string; name: string }) => renameView(caseId, input.viewId, input.name),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: viewKeys.all(caseId) }),
  })
}

export function useSetViewSharingMutation(caseId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: { viewId: string; isShared: boolean }) =>
      setViewSharing(caseId, input.viewId, input.isShared),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: viewKeys.all(caseId) }),
  })
}

export function useDeleteViewMutation(caseId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (viewId: string) => deleteView(caseId, viewId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: viewKeys.list(caseId) }),
  })
}
