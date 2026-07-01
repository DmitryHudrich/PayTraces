import { SigmaGraphAdapter, type GraphData, type GraphLayoutMode } from '@/shared/graph'

type TransactionGraphWidgetProps = {
  graph: GraphData
  layout: GraphLayoutMode
  selectedNodeId: string
  onSelectNode: (nodeId: string) => void
}

export const TransactionGraphWidget = ({ graph, layout, selectedNodeId, onSelectNode }: TransactionGraphWidgetProps) => {
  return (
    <SigmaGraphAdapter graph={graph} layout={layout} selectedNodeId={selectedNodeId} onNodeSelect={onSelectNode} />
  )
}
