import type { CaseGraphNode } from '@/entities/case-graph/model/graph'

/**
 * How complete the graph is at a node — drives the node's outline colour so an
 * investigator can see at a glance how much of the graph is still hidden.
 */
export type Completeness = 'ingest-boundary' | 'view-boundary' | 'complete' | 'unknown'

export function nodeCompleteness(node: CaseGraphNode | null | undefined): Completeness {
  if (!node) {
    return 'unknown'
  }
  if (node.isIngestBoundary) {
    return 'ingest-boundary'
  }
  if (node.isViewBoundary) {
    return 'view-boundary'
  }
  return 'complete'
}

/** Outline (border) colour per completeness state — outlines, never fills. */
export const COMPLETENESS_BORDER: Record<Completeness, string> = {
  'ingest-boundary': '#f472b6',
  'view-boundary': '#facc15',
  complete: '#34d399',
  unknown: '#64748b',
}

export const COMPLETENESS_LABEL: Record<Completeness, string> = {
  'ingest-boundary': 'Ingest boundary — no external data beyond',
  'view-boundary': 'Has more data — expand to load',
  complete: 'Fully shown — no further transfers',
  unknown: 'Not enriched',
}
