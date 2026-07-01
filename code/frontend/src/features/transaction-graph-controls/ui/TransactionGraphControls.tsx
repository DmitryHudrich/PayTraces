import { Button } from '@/shared/ui/button'
import type { GraphLayoutMode } from '@/shared/graph'

type TransactionGraphControlsProps = {
  query: string
  onQueryChange: (value: string) => void
  layout: GraphLayoutMode
  onLayoutChange: (value: GraphLayoutMode) => void
  nodeCount: number
  edgeCount: number
  selectedNodeLabel?: string | null
}

export const TransactionGraphControls = ({
  query,
  onQueryChange,
  layout,
  onLayoutChange,
  nodeCount,
  edgeCount,
  selectedNodeLabel,
}: TransactionGraphControlsProps) => {
  return (
    <div className='grid gap-3 rounded-xl border border-border bg-card p-4 md:grid-cols-[1fr_auto_auto] md:items-center'>
      <input
        value={query}
        onChange={(event) => onQueryChange(event.target.value)}
        placeholder='Filter by address, amount or symbol (USDT / ETH)'
        className='h-10 rounded-md border border-input bg-background px-3 text-sm outline-none ring-offset-background placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring'
      />

      <div className='flex items-center gap-2'>
        <Button variant={layout === 'force' ? 'default' : 'outline'} size='sm' onClick={() => onLayoutChange('force')}>
          Force
        </Button>
        <Button
          variant={layout === 'concentric' ? 'default' : 'outline'}
          size='sm'
          onClick={() => onLayoutChange('concentric')}
        >
          Concentric
        </Button>
        <Button
          variant={layout === 'breadthfirst' ? 'default' : 'outline'}
          size='sm'
          onClick={() => onLayoutChange('breadthfirst')}
        >
          Flow
        </Button>
      </div>

      <div className='text-xs text-muted-foreground'>
        nodes: {nodeCount} | edges: {edgeCount}
        {selectedNodeLabel ? ` | selected: ${selectedNodeLabel}` : ''}
      </div>
    </div>
  )
}
