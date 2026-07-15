export type GraphNode = {
  id: string
  label: string
  group?: string
  weight?: number
  /** Outline colour drawn as a ring around the node (never a fill). */
  borderColor?: string | null
}

export type GraphEdge = {
  id: string
  source: string
  target: string
  label?: string
  weight?: number
}

export type GraphData = {
  nodes: GraphNode[]
  edges: GraphEdge[]
}

export type XY = { x: number; y: number }

export const GRAPH_LAYOUT_OPTIONS = [
  { value: 'force', label: 'Force', description: 'Organic force-directed layout' },
  { value: 'breadthfirst', label: 'Flow', description: 'Layers by graph distance' },
  { value: 'spiral', label: 'Spiral', description: 'Readable spacing for dense graphs' },
  { value: 'grid', label: 'Grid', description: 'Even grid for dense graphs' },
] as const

export type GraphLayoutMode = (typeof GRAPH_LAYOUT_OPTIONS)[number]['value']

export function isGraphLayoutMode(value: string): value is GraphLayoutMode {
  return GRAPH_LAYOUT_OPTIONS.some((option) => option.value === value)
}

export type GraphAdapterProps = {
  graph: GraphData
  layout: GraphLayoutMode
  rootNodeIds?: ReadonlySet<string> | null
  selectedNodeId?: string
  visibleNodeIds?: ReadonlySet<string> | null
  visibleEdgeIds?: ReadonlySet<string> | null
  /** Seed coordinates for named nodes (e.g. a saved canvas view). */
  pinnedPositions?: ReadonlyMap<string, XY> | null
  onNodeSelect?: (nodeId: string) => void
  onNodeHover?: (nodeId: string | null) => void
  /** Fires after a drag with the full current position map. */
  onPositionsChange?: (positions: Map<string, XY>) => void
  /** Provides a stable getter for the current node positions (for saving views). */
  onExportReady?: (getPositions: () => Map<string, XY>) => void
}
