import { COMPLETENESS_BORDER } from '@/entities/case-graph'

const FILL_ITEMS: { label: string; color: string }[] = [
  { label: 'Root', color: '#f59e0b' },
  { label: 'Critical', color: '#f43f5e' },
  { label: 'High', color: '#fb923c' },
  { label: 'Medium', color: '#38bdf8' },
  { label: 'Service', color: '#c4b0f5' },
  { label: 'Wallet', color: '#7eb6ff' },
]

const BORDER_ITEMS: { label: string; color: string }[] = [
  { label: 'Complete', color: COMPLETENESS_BORDER.complete },
  { label: 'Expandable', color: COMPLETENESS_BORDER['view-boundary'] },
  { label: 'Ingest boundary', color: COMPLETENESS_BORDER['ingest-boundary'] },
]

export function GraphLegend() {
  return (
    <div className='flex flex-wrap items-center gap-x-4 gap-y-1.5 text-xs text-muted-foreground'>
      {FILL_ITEMS.map((item) => (
        <span key={item.label} className='flex items-center gap-1.5'>
          <span className='size-2.5 rounded-full' style={{ backgroundColor: item.color }} />
          {item.label}
        </span>
      ))}
      <span className='mx-1 h-3 w-px bg-border' />
      {BORDER_ITEMS.map((item) => (
        <span key={item.label} className='flex items-center gap-1.5'>
          <span
            className='size-2.5 rounded-full border-2 bg-transparent'
            style={{ borderColor: item.color }}
          />
          {item.label}
        </span>
      ))}
    </div>
  )
}
