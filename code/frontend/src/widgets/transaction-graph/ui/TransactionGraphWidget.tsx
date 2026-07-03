import { SigmaGraphAdapter, type GraphData, type GraphLayoutMode } from '@/shared/graph'

type TransactionGraphWidgetProps = {
  graph: GraphData
  layout: GraphLayoutMode
  selectedNodeId: string
  visibleNodeIds?: ReadonlySet<string> | null
  visibleEdgeIds?: ReadonlySet<string> | null
  onSelectNode: (nodeId: string) => void
}

export const TransactionGraphWidget = ({
  graph,
  layout,
  selectedNodeId,
  visibleNodeIds,
  visibleEdgeIds,
  onSelectNode,
}: TransactionGraphWidgetProps) => {
  return (
    <SigmaGraphAdapter
      graph={graph}
      layout={layout}
      selectedNodeId={selectedNodeId}
      visibleNodeIds={visibleNodeIds}
      visibleEdgeIds={visibleEdgeIds}
      onNodeSelect={onSelectNode}
    />
  )
}
