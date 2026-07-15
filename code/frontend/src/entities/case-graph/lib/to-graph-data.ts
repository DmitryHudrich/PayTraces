import type { GraphData, GraphNode } from '@/shared/graph'
import { shortAddress } from '@/shared/lib/format'
import { edgeKey, type CaseGraphEdge, type CaseGraphNode } from '@/entities/case-graph/model/graph'
import { nodeGroup } from '@/entities/case-graph/lib/risk'
import { COMPLETENESS_BORDER, nodeCompleteness } from '@/entities/case-graph/lib/completeness'

function edgeAmount(edge: CaseGraphEdge): number {
  const value = Number(edge.formatted)
  return Number.isFinite(value) ? value : 0
}

function edgeWeight(edge: CaseGraphEdge): number {
  return Math.max(1, Math.min(10, Math.log10(edgeAmount(edge) + 1) * 2.2))
}

/**
 * Projects the accumulated engine nodes/edges onto the shared graph contract.
 * `labelOverrides` (keyed by lowercased address) win over service names.
 */
export function buildGraphData(
  nodes: CaseGraphNode[],
  edges: CaseGraphEdge[],
  labelOverrides?: ReadonlyMap<string, string>,
): GraphData {
  const nodeByAddress = new Map(nodes.map((node) => [node.address.toLowerCase(), node]))
  const degree = new Map<string, number>()
  const addresses = new Set<string>(nodeByAddress.keys())

  for (const edge of edges) {
    const from = edge.from.toLowerCase()
    const to = edge.to.toLowerCase()
    addresses.add(from)
    addresses.add(to)
    degree.set(from, (degree.get(from) ?? 0) + 1)
    degree.set(to, (degree.get(to) ?? 0) + 1)
  }

  const graphNodes: GraphNode[] = Array.from(addresses).map((address) => {
    const node = nodeByAddress.get(address)
    const custom = labelOverrides?.get(address)
    const serviceName = node?.serviceName?.trim()
    const label = custom ?? (serviceName ? serviceName : shortAddress(address))
    const rawWeight = node?.txCount ?? degree.get(address) ?? 1
    return {
      id: address,
      label,
      group: node ? nodeGroup(node) : 'wallet',
      weight: Math.max(1, Math.min(30, rawWeight)),
      borderColor: COMPLETENESS_BORDER[nodeCompleteness(node)],
    }
  })

  const graphEdges = edges.map((edge, index) => ({
    id: `${edgeKey(edge)}:${index}`,
    source: edge.from.toLowerCase(),
    target: edge.to.toLowerCase(),
    label: `${edge.formatted} ${edge.symbol}`,
    weight: edgeWeight(edge),
  }))

  return { nodes: graphNodes, edges: graphEdges }
}
