import type { CaseGraphEdge, CaseGraphNode } from '@/entities/case-graph/model/graph'

export type EdgeFlow = {
  edge: CaseGraphEdge
  direction: 'in' | 'out'
  counterparty: string
}

export type NodeDetails = {
  address: string
  node: CaseGraphNode | null
  inbound: EdgeFlow[]
  outbound: EdgeFlow[]
  inboundCount: number
  outboundCount: number
}

/** Builds a focused summary for the selected address from the raw engine data. */
export function buildNodeDetails(
  address: string,
  nodes: ReadonlyMap<string, CaseGraphNode>,
  edges: readonly CaseGraphEdge[],
): NodeDetails {
  const key = address.toLowerCase()
  const inbound: EdgeFlow[] = []
  const outbound: EdgeFlow[] = []

  for (const edge of edges) {
    const from = edge.from.toLowerCase()
    const to = edge.to.toLowerCase()
    if (to === key) {
      inbound.push({ edge, direction: 'in', counterparty: edge.from })
    }
    if (from === key) {
      outbound.push({ edge, direction: 'out', counterparty: edge.to })
    }
  }

  return {
    address,
    node: nodes.get(key) ?? null,
    inbound,
    outbound,
    inboundCount: inbound.length,
    outboundCount: outbound.length,
  }
}
