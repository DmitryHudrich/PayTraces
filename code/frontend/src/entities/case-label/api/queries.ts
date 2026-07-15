import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import {
  applyLabel,
  createLabel,
  deleteLabel,
  listAddressLabels,
  listCaseLabels,
  removeLabelFromAddress,
} from '@/entities/case-label/api/labels'

export const labelKeys = {
  all: (caseId: string) => ['labels', caseId] as const,
  list: (caseId: string) => ['labels', caseId, 'list'] as const,
  address: (caseId: string, chainId: number, address: string) =>
    ['labels', caseId, 'address', chainId, address.toLowerCase()] as const,
}

export function useCaseLabelsQuery(caseId: string | undefined) {
  return useQuery({
    queryKey: labelKeys.list(caseId ?? ''),
    queryFn: () => listCaseLabels(caseId as string),
    enabled: Boolean(caseId),
  })
}

export function useAddressLabelsQuery(caseId: string | undefined, chainId: number, address: string | null) {
  return useQuery({
    queryKey: labelKeys.address(caseId ?? '', chainId, address ?? ''),
    queryFn: () => listAddressLabels(caseId as string, chainId, address as string),
    enabled: Boolean(caseId && address),
  })
}

export function useCreateLabelMutation(caseId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: { text: string; color?: string | null }) => createLabel(caseId, input),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: labelKeys.list(caseId) }),
  })
}

export function useDeleteLabelMutation(caseId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (labelId: string) => deleteLabel(caseId, labelId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: labelKeys.all(caseId) }),
  })
}

export function useApplyLabelMutation(caseId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: { labelId: string; address: string; chainId: number }) => applyLabel(caseId, input),
    onSuccess: (_data, variables) =>
      queryClient.invalidateQueries({
        queryKey: labelKeys.address(caseId, variables.chainId, variables.address),
      }),
  })
}

export function useRemoveAddressLabelMutation(caseId: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: { labelId: string; chainId: number; address: string }) =>
      removeLabelFromAddress(caseId, input),
    onSuccess: (_data, variables) =>
      queryClient.invalidateQueries({
        queryKey: labelKeys.address(caseId, variables.chainId, variables.address),
      }),
  })
}
