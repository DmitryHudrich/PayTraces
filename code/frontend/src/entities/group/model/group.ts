export type GroupMember = {
  address: string
  chainId: number
  addedAt: string
}

export type AddressGroupSummary = {
  id: string
  caseId: string
  name: string
  createdBy: string
  createdAt: string
  memberCount: number
}

export type AddressGroup = {
  id: string
  caseId: string
  name: string
  createdBy: string
  createdAt: string
  members: GroupMember[]
}
