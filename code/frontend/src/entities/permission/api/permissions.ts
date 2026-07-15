import { apiRequest } from '@/shared/api'

export function getMyPermissions(caseId?: string): Promise<string[]> {
  const query = caseId ? `?caseId=${encodeURIComponent(caseId)}` : ''
  return apiRequest<string[]>(`/me/permissions${query}`)
}
