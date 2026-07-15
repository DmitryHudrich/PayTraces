import { Clock } from 'lucide-react'

import type { BlockBounds } from '@/entities/case-graph'
import { RangeTimelineSlider } from '@/shared/ui/range-timeline-slider'

type GraphTimelineProps = {
  bounds: BlockBounds
  value: { from: number; to: number }
  onChange: (value: { from: number; to: number }) => void
}

export function GraphTimeline({ bounds, value, onChange }: GraphTimelineProps) {
  const isFull = value.from <= bounds.min && value.to >= bounds.max
  return (
    <div className='flex items-center gap-3 rounded-lg border border-border/70 bg-card/50 px-3 py-2 backdrop-blur-sm'>
      <span className='flex shrink-0 items-center gap-1.5 text-xs text-muted-foreground'>
        <Clock className='size-3.5' />
        Blocks
      </span>
      <span className='shrink-0 font-mono text-xs tabular-nums text-muted-foreground'>{value.from.toLocaleString()}</span>
      <RangeTimelineSlider
        min={bounds.min}
        max={bounds.max}
        value={value}
        onChange={onChange}
        className='flex-1'
      />
      <span className='shrink-0 font-mono text-xs tabular-nums text-muted-foreground'>{value.to.toLocaleString()}</span>
      {!isFull ? (
        <button
          type='button'
          className='shrink-0 rounded px-1.5 py-0.5 text-xs text-accent hover:underline'
          onClick={() => onChange({ from: bounds.min, to: bounds.max })}
        >
          Reset
        </button>
      ) : null}
    </div>
  )
}
