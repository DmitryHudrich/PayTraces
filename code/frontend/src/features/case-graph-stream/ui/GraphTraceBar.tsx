import { Loader2, Play, Waypoints } from 'lucide-react'
import { useEffect, useState } from 'react'

import type { CaseAddress } from '@/entities/case'
import type { CaseGraphViewSummary } from '@/entities/view'
import type { StreamParams } from '@/features/case-graph-stream/model/use-case-graph'
import { shortAddress } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/shared/ui/select'

const DEPTHS = [1, 2, 3, 4]
const NO_VIEW = '__none__'

type GraphTraceBarProps = {
  addresses: CaseAddress[]
  views: CaseGraphViewSummary[]
  isStreaming: boolean
  onTrace: (params: StreamParams) => void
}

export function GraphTraceBar({ addresses, views, isStreaming, onTrace }: GraphTraceBarProps) {
  const [address, setAddress] = useState('')
  const [depth, setDepth] = useState('2')
  const [viewId, setViewId] = useState<string>(NO_VIEW)

  useEffect(() => {
    if (!address && addresses.length > 0) {
      setAddress(addresses[0].address)
    }
  }, [address, addresses])

  const selected = addresses.find((item) => item.address === address)
  const chainId = selected?.chainId ?? 1

  const trace = () => {
    if (!address) {
      return
    }
    onTrace({
      address,
      chainId,
      maxDepth: Number(depth),
      viewId: viewId === NO_VIEW ? null : viewId,
    })
  }

  return (
    <div className='flex flex-wrap items-center gap-2'>
      <div className='min-w-52 flex-1'>
        <Select value={address} onValueChange={setAddress} disabled={addresses.length === 0}>
          <SelectTrigger size='sm' className='font-mono text-xs'>
            <SelectValue placeholder='Select a case address…' />
          </SelectTrigger>
          <SelectContent>
            {addresses.map((item) => (
              <SelectItem key={`${item.chainId}:${item.address}`} value={item.address} className='font-mono text-xs'>
                {shortAddress(item.address, 10, 6)}
                <span className='ml-1 text-muted-foreground'>· chain {item.chainId}</span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <Select value={depth} onValueChange={setDepth}>
        <SelectTrigger size='sm' className='w-28'>
          <Waypoints className='size-3.5 text-muted-foreground' />
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {DEPTHS.map((value) => (
            <SelectItem key={value} value={String(value)}>
              Depth {value}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      {views.length > 0 ? (
        <Select value={viewId} onValueChange={setViewId}>
          <SelectTrigger size='sm' className='w-40'>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={NO_VIEW}>Auto layout</SelectItem>
            {views.map((view) => (
              <SelectItem key={view.id} value={view.id}>
                {view.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      ) : null}

      <Button size='sm' onClick={trace} disabled={!address || isStreaming}>
        {isStreaming ? <Loader2 className='animate-spin' /> : <Play />}
        Trace
      </Button>
    </div>
  )
}
