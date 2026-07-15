import { apiRequest } from '@/shared/api'
import type { AppliedLabel, CustomLabel } from '@/entities/case-label/model/label'

export function listCaseLabels(caseId: string): Promise<CustomLabel[]> {
  return apiRequest<CustomLabel[]>(`/cases/${caseId}/labels`)
}

export function createLabel(caseId: string, input: { text: string; color?: string | null }): Promise<{ id: string }> {
  return apiRequest<{ id: string }>(`/cases/${caseId}/labels`, {
    method: 'POST',
    body: JSON.stringify({ text: input.text, color: input.color ?? null }),
  })
}

export function deleteLabel(caseId: string, labelId: string): Promise<void> {
  return apiRequest<void>(`/cases/${caseId}/labels/${labelId}`, { method: 'DELETE' })
}

export function applyLabel(
  caseId: string,
  input: { labelId: string; address: string; chainId: number },
): Promise<void> {
  return apiRequest<void>(`/cases/${caseId}/labels/apply`, {
    method: 'POST',
    body: JSON.stringify(input),
  })
}

export function removeLabelFromAddress(
  caseId: string,
  input: { labelId: string; chainId: number; address: string },
): Promise<void> {
  return apiRequest<void>(
    `/cases/${caseId}/labels/${input.labelId}/addresses/${input.chainId}/${encodeURIComponent(input.address)}`,
    { method: 'DELETE' },
  )
}

export function listAddressLabels(caseId: string, chainId: number, address: string): Promise<AppliedLabel[]> {
  return apiRequest<AppliedLabel[]>(
    `/cases/${caseId}/addresses/${chainId}/${encodeURIComponent(address)}/labels`,
  )
}
