/** Enriched node as returned by the C# graph BFF (camelCase JSON). */
export type CaseGraphNode = {
  address: string
  kind: string | null
  serviceName: string | null
  riskScore: number | null
  isHighRisk: boolean | null
  inDegree: number | null
  outDegree: number | null
  txCount: number | null
  isViewBoundary: boolean | null
  isIngestBoundary: boolean | null
}

/** One transfer edge from the engine. */
export type CaseGraphEdge = {
  txHash: string
  index: number
  from: string
  to: string
  raw: string
  formatted: string
  symbol: string
  decimals: number
  block: number
  ts: number
  kind: string
  contract: string | null
  chainId: number
}

/** One page of the engine's paginated BFS walk. */
export type CaseGraphPage = {
  totalNodes: number
  totalEdges: number
  page: number
  pageSize: number
  totalPages: number
  hasNext: boolean
  nodes: CaseGraphNode[]
  edges: CaseGraphEdge[]
}

export type NodePosition = { address: string; x: number; y: number }

export function edgeKey(edge: CaseGraphEdge): string {
  return `${edge.txHash}:${edge.index}:${edge.from.toLowerCase()}:${edge.to.toLowerCase()}`
}
