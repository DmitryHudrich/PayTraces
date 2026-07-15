import type { CaseGraphEdge } from '@/entities/case-graph/model/graph'

export type BlockBounds = { min: number; max: number }

/** Min/max block height across the given transfers, or null when empty. */
export function blockBounds(edges: readonly CaseGraphEdge[]): BlockBounds | null {
  if (edges.length === 0) {
    return null
  }
  let min = Number.POSITIVE_INFINITY
  let max = Number.NEGATIVE_INFINITY
  for (const edge of edges) {
    if (edge.block < min) {
      min = edge.block
    }
    if (edge.block > max) {
      max = edge.block
    }
  }
  return Number.isFinite(min) && Number.isFinite(max) ? { min, max } : null
}
