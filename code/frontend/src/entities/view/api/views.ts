import { apiRequest } from '@/shared/api'
import type { CaseGraphView, CaseGraphViewSummary } from '@/entities/view/model/view'

export function listViews(caseId: string): Promise<CaseGraphViewSummary[]> {
  return apiRequest<CaseGraphViewSummary[]>(`/cases/${caseId}/views`)
}

export function getView(caseId: string, viewId: string): Promise<CaseGraphView> {
  return apiRequest<CaseGraphView>(`/cases/${caseId}/views/${viewId}`)
}

export function createView(caseId: string, input: { name: string; isShared: boolean }): Promise<{ id: string }> {
  return apiRequest<{ id: string }>(`/cases/${caseId}/views`, {
    method: 'POST',
    body: JSON.stringify(input),
  })
}

export function renameView(caseId: string, viewId: string, name: string): Promise<void> {
  return apiRequest<void>(`/cases/${caseId}/views/${viewId}`, {
    method: 'PUT',
    body: JSON.stringify({ name }),
  })
}

export function setViewSharing(caseId: string, viewId: string, isShared: boolean): Promise<void> {
  return apiRequest<void>(`/cases/${caseId}/views/${viewId}/sharing`, {
    method: 'PUT',
    body: JSON.stringify({ isShared }),
  })
}

export function deleteView(caseId: string, viewId: string): Promise<void> {
  return apiRequest<void>(`/cases/${caseId}/views/${viewId}`, { method: 'DELETE' })
}

export function pinNode(
  caseId: string,
  viewId: string,
  input: { address: string; x: number; y: number },
): Promise<void> {
  return apiRequest<void>(`/cases/${caseId}/views/${viewId}/nodes`, {
    method: 'PUT',
    body: JSON.stringify(input),
  })
}

export function unpinNode(caseId: string, viewId: string, address: string): Promise<void> {
  return apiRequest<void>(`/cases/${caseId}/views/${viewId}/nodes/${encodeURIComponent(address)}`, {
    method: 'DELETE',
  })
}
