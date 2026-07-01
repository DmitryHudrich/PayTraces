import { Button } from '@/shared/ui/button'

type GraphFlowState = {
  address: string
  fromBlock: string
  maxDepth: string
  maxNodes: string
}

type TransactionGraphFlowFormProps = {
  value: GraphFlowState
  onChange: (next: GraphFlowState) => void
  onIngest: () => void
  onDrawGraph: () => void
  isIngesting: boolean
  isDrawing: boolean
  ingestJobId?: string | null
  statusMessage?: string | null
  errorMessage?: string | null
}

export const TransactionGraphFlowForm = ({
  value,
  onChange,
  onIngest,
  onDrawGraph,
  isIngesting,
  isDrawing,
  ingestJobId,
  statusMessage,
  errorMessage,
}: TransactionGraphFlowFormProps) => {
  const setField = (field: keyof GraphFlowState, next: string) => {
    onChange({ ...value, [field]: next })
  }

  return (
    <div className='rounded-xl border border-border bg-card p-4'>
      <div className='grid gap-3 md:grid-cols-2'>
        <label className='flex flex-col gap-1 text-sm'>
          Wallet address (required)
          <input
            value={value.address}
            onChange={(event) => setField('address', event.target.value)}
            placeholder='0x...'
            className='h-10 rounded-md border border-input bg-background px-3 outline-none ring-offset-background placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring'
          />
        </label>

        <label className='flex flex-col gap-1 text-sm'>
          from_block (required)
          <input
            value={value.fromBlock}
            onChange={(event) => setField('fromBlock', event.target.value)}
            placeholder='19000000'
            inputMode='numeric'
            className='h-10 rounded-md border border-input bg-background px-3 outline-none ring-offset-background placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring'
          />
        </label>

        <label className='flex flex-col gap-1 text-sm'>
          max_depth (optional, default 3)
          <input
            value={value.maxDepth}
            onChange={(event) => setField('maxDepth', event.target.value)}
            placeholder='3'
            inputMode='numeric'
            className='h-10 rounded-md border border-input bg-background px-3 outline-none ring-offset-background placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring'
          />
        </label>

        <label className='flex flex-col gap-1 text-sm'>
          max_nodes (optional, default 500)
          <input
            value={value.maxNodes}
            onChange={(event) => setField('maxNodes', event.target.value)}
            placeholder='500'
            inputMode='numeric'
            className='h-10 rounded-md border border-input bg-background px-3 outline-none ring-offset-background placeholder:text-muted-foreground focus-visible:ring-2 focus-visible:ring-ring'
          />
        </label>
      </div>

      <div className='mt-4 flex flex-wrap items-center gap-3'>
        <Button type='button' onClick={onIngest} disabled={isIngesting || isDrawing}>
          {isIngesting ? 'Стягиваем...' : 'Стянуть данные'}
        </Button>

        <Button type='button' variant='secondary' onClick={onDrawGraph} disabled={isIngesting || isDrawing}>
          {isDrawing ? 'Рисуем...' : 'Отрисовать граф'}
        </Button>

        {ingestJobId ? <span className='text-xs text-muted-foreground'>job_id: {ingestJobId}</span> : null}
      </div>

      {statusMessage ? <p className='mt-3 text-xs text-muted-foreground'>{statusMessage}</p> : null}
      {errorMessage ? <p className='mt-2 text-xs text-destructive'>{errorMessage}</p> : null}
    </div>
  )
}
