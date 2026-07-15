import { Loader2, Network } from 'lucide-react'

import type { GraphData, GraphLayoutMode, XY } from '@/shared/graph'
import { SigmaGraphAdapter } from '@/shared/graph'
import { GraphLegend } from '@/widgets/case-graph/ui/GraphLegend'

type CaseGraphCanvasProps = {
  graph: GraphData
  layout: GraphLayoutMode
  rootNodeIds: ReadonlySet<string> | null
  selectedNodeId: string
  visibleNodeIds: ReadonlySet<string> | null
  visibleEdgeIds: ReadonlySet<string> | null
  pinnedPositions: ReadonlyMap<string, XY> | null
  isStreaming: boolean
  isEmpty: boolean
  onSelectNode: (nodeId: string) => void
  onPositionsChange?: (positions: Map<string, XY>) => void
  onExportReady?: (getPositions: () => Map<string, XY>) => void
}

export function CaseGraphCanvas({
  graph,
  layout,
  rootNodeIds,
  selectedNodeId,
  visibleNodeIds,
  visibleEdgeIds,
  pinnedPositions,
  isStreaming,
  isEmpty,
  onSelectNode,
  onPositionsChange,
  onExportReady,
}: CaseGraphCanvasProps) {
  return (
    <div className='relative flex h-full min-h-0 flex-col gap-2'>
      <div className='flex items-center justify-between px-1'>
        <GraphLegend />
        {isStreaming ? (
          <span className='flex items-center gap-1.5 text-xs text-accent'>
            <Loader2 className='size-3 animate-spin' /> Streaming
          </span>
        ) : null}
      </div>

      <div className='relative min-h-0 flex-1'>
        <SigmaGraphAdapter
          graph={graph}
          layout={layout}
          rootNodeIds={rootNodeIds}
          selectedNodeId={selectedNodeId}
          visibleNodeIds={visibleNodeIds}
          visibleEdgeIds={visibleEdgeIds}
          pinnedPositions={pinnedPositions}
          onNodeSelect={onSelectNode}
          onPositionsChange={onPositionsChange}
          onExportReady={onExportReady}
        />

        {isEmpty && !isStreaming ? (
          <div className='pointer-events-none absolute inset-0 flex items-center justify-center rounded-xl border border-dashed border-border/70 bg-background/60 backdrop-blur-sm'>
            <div className='flex max-w-xs flex-col items-center gap-3 px-6 text-center'>
              <span className='flex size-11 items-center justify-center rounded-full bg-primary/10 text-primary'>
                <Network className='size-5' />
              </span>
              <p className='text-sm font-medium'>No graph loaded</p>
              <p className='text-xs text-muted-foreground'>
                Pick a case address and press Trace to stream its transaction graph from the engine.
              </p>
            </div>
          </div>
        ) : null}
      </div>
    </div>
  )
}
