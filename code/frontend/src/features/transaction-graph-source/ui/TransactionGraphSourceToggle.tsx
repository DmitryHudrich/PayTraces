import { Button } from '@/shared/ui/button'

export type GraphSourceMode = 'mock' | 'backend'

type TransactionGraphSourceToggleProps = {
  value: GraphSourceMode
  onChange: (value: GraphSourceMode) => void
}

export const TransactionGraphSourceToggle = ({ value, onChange }: TransactionGraphSourceToggleProps) => {
  return (
    <div className='flex flex-col gap-2'>
      <span className='text-xs font-medium text-muted-foreground'>Data source</span>
      <div className='grid grid-cols-2 gap-1 rounded-lg border border-border bg-background p-1'>
        <Button
          type='button'
          size='sm'
          variant={value === 'mock' ? 'default' : 'ghost'}
          onClick={() => onChange('mock')}
        >
          Mock
        </Button>
        <Button
          type='button'
          size='sm'
          variant={value === 'backend' ? 'default' : 'ghost'}
          onClick={() => onChange('backend')}
        >
          Backend
        </Button>
      </div>
    </div>
  )
}
