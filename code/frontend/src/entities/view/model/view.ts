import type { NodePosition } from '@/entities/case-graph'

export type CaseGraphViewSummary = {
  id: string
  caseId: string
  name: string
  createdBy: string
  isShared: boolean
  createdAt: string
  pinnedCount: number
}

export type CaseGraphView = {
  id: string
  caseId: string
  name: string
  createdBy: string
  isShared: boolean
  createdAt: string
  positions: NodePosition[]
}
