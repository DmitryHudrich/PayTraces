import { apiRequest } from '@/shared/api'
import type { AddressGroup, AddressGroupSummary } from '@/entities/group/model/group'

export function listGroups(caseId: string): Promise<AddressGroupSummary[]> {
  return apiRequest<AddressGroupSummary[]>(`/cases/${caseId}/groups`)
}

export function getGroup(caseId: string, groupId: string): Promise<AddressGroup> {
  return apiRequest<AddressGroup>(`/cases/${caseId}/groups/${groupId}`)
}

export function createGroup(
  caseId: string,
  input: { name: string; members?: { address: string; chainId: number }[] },
): Promise<{ id: string }> {
  return apiRequest<{ id: string }>(`/cases/${caseId}/groups`, {
    method: 'POST',
    body: JSON.stringify({ name: input.name, members: input.members ?? [] }),
  })
}

export function renameGroup(caseId: string, groupId: string, name: string): Promise<void> {
  return apiRequest<void>(`/cases/${caseId}/groups/${groupId}`, {
    method: 'PUT',
    body: JSON.stringify({ name }),
  })
}

export function deleteGroup(caseId: string, groupId: string): Promise<void> {
  return apiRequest<void>(`/cases/${caseId}/groups/${groupId}`, { method: 'DELETE' })
}

export function addGroupMember(
  caseId: string,
  groupId: string,
  input: { address: string; chainId: number },
): Promise<void> {
  return apiRequest<void>(`/cases/${caseId}/groups/${groupId}/members`, {
    method: 'POST',
    body: JSON.stringify(input),
  })
}

export function removeGroupMember(
  caseId: string,
  groupId: string,
  input: { chainId: number; address: string },
): Promise<void> {
  return apiRequest<void>(
    `/cases/${caseId}/groups/${groupId}/members/${input.chainId}/${encodeURIComponent(input.address)}`,
    { method: 'DELETE' },
  )
}
