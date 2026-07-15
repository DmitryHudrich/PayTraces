import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import {
  addCaseAddress,
  assignCase,
  closeCase,
  createCase,
  getCase,
  listCases,
} from '@/entities/case/api/cases'

export const caseKeys = {
  all: ['cases'] as const,
  list: () => ['cases', 'list'] as const,
  detail: (id: string) => ['cases', 'detail', id] as const,
}

export function useCasesQuery() {
  return useQuery({ queryKey: caseKeys.list(), queryFn: listCases })
}

export function useCaseQuery(id: string | undefined) {
  return useQuery({
    queryKey: caseKeys.detail(id ?? ''),
    queryFn: () => getCase(id as string),
    enabled: Boolean(id),
  })
}

export function useCreateCaseMutation() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: createCase,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: caseKeys.list() }),
  })
}

export function useCloseCaseMutation(id: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () => closeCase(id),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: caseKeys.detail(id) })
      void queryClient.invalidateQueries({ queryKey: caseKeys.list() })
    },
  })
}

export function useAssignCaseMutation(id: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: { userId: string; roleName: string }) => assignCase(id, input),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: caseKeys.detail(id) }),
  })
}

export function useAddCaseAddressMutation(id: string) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (input: { address: string; chainId: number; note?: string }) => addCaseAddress(id, input),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: caseKeys.detail(id) }),
  })
}
