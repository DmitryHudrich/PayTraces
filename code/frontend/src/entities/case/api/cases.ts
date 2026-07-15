import { apiRequest } from '@/shared/api'
import type { CaseDetail, CasePriority, CaseSummary } from '@/entities/case/model/case'

export function listCases(): Promise<CaseSummary[]> {
  return apiRequest<CaseSummary[]>('/cases')
}

export function getCase(id: string): Promise<CaseDetail> {
  return apiRequest<CaseDetail>(`/cases/${id}`)
}

export function createCase(input: {
  title: string
  description?: string
  priority: CasePriority
}): Promise<{ id: string }> {
  return apiRequest<{ id: string }>('/cases', {
    method: 'POST',
    body: JSON.stringify({
      title: input.title,
      description: input.description ?? '',
      priority: input.priority,
    }),
  })
}

export function closeCase(id: string): Promise<void> {
  return apiRequest<void>(`/cases/${id}/close`, { method: 'POST' })
}

export function assignCase(id: string, input: { userId: string; roleName: string }): Promise<void> {
  return apiRequest<void>(`/cases/${id}/assign`, {
    method: 'POST',
    body: JSON.stringify(input),
  })
}

export function addCaseAddress(
  id: string,
  input: { address: string; chainId: number; note?: string },
): Promise<void> {
  return apiRequest<void>(`/cases/${id}/addresses`, {
    method: 'POST',
    body: JSON.stringify({
      address: input.address,
      chainId: input.chainId,
      note: input.note?.trim() ? input.note.trim() : null,
    }),
  })
}
