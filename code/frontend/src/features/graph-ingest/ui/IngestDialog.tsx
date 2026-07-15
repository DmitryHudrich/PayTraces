import { DownloadCloud, Loader2 } from 'lucide-react'
import { useState, type FormEvent } from 'react'
import { toast } from 'sonner'

import type { CaseAddress } from '@/entities/case'
import { startIngest } from '@/entities/graph-insight'
import { waitForIngestJob } from '@/features/graph-ingest/lib/wait-for-job'
import { getErrorMessage } from '@/shared/api'
import { Button } from '@/shared/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'

type IngestDialogProps = {
  caseId: string
  addresses: CaseAddress[]
  onIngested: (address: string, chainId: number) => void
}

function toNumber(value: string): number | null {
  const trimmed = value.trim()
  if (!trimmed) {
    return null
  }
  const parsed = Number(trimmed)
  return Number.isFinite(parsed) ? parsed : null
}

export function IngestDialog({ caseId, addresses, onIngested }: IngestDialogProps) {
  const [open, setOpen] = useState(false)
  const [address, setAddress] = useState(addresses[0]?.address ?? '')
  const [chainId, setChainId] = useState(String(addresses[0]?.chainId ?? 1))
  const [fromBlock, setFromBlock] = useState('')
  const [toBlock, setToBlock] = useState('')
  const [maxDepth, setMaxDepth] = useState('3')
  const [maxNodes, setMaxNodes] = useState('500')
  const [phase, setPhase] = useState<string | null>(null)

  const busy = phase !== null

  const submit = async (event: FormEvent) => {
    event.preventDefault()
    const trimmed = address.trim()
    if (!trimmed) {
      return
    }
    const chain = toNumber(chainId) ?? 1
    setPhase('Starting ingest…')
    try {
      const { jobId } = await startIngest(caseId, {
        address: trimmed,
        chainId: chain,
        fromBlock: toNumber(fromBlock),
        toBlock: toNumber(toBlock),
        maxDepth: toNumber(maxDepth),
        maxNodes: toNumber(maxNodes),
      })
      setPhase('Pulling on-chain data…')
      await waitForIngestJob(caseId, jobId, {
        onProgress: (status) => setPhase(`Ingest: ${status.status}…`),
      })
      toast.success('Ingest complete')
      setOpen(false)
      setPhase(null)
      onIngested(trimmed, chain)
    } catch (error) {
      setPhase(null)
      toast.error(getErrorMessage(error, 'Ingest failed.'))
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (busy) {
          return
        }
        setOpen(next)
        if (next) {
          setAddress(addresses[0]?.address ?? '')
          setChainId(String(addresses[0]?.chainId ?? 1))
        }
      }}
    >
      <DialogTrigger asChild>
        <Button size='sm' variant='outline'>
          <DownloadCloud />
          Ingest
        </Button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={submit}>
          <DialogHeader>
            <DialogTitle>Ingest on-chain data</DialogTitle>
            <DialogDescription>
              Pull transfers from the external provider into the engine, optionally limited to a block range.
            </DialogDescription>
          </DialogHeader>

          <div className='space-y-4 py-4'>
            <div className='space-y-2'>
              <Label htmlFor='ingest-address'>Address</Label>
              <Input
                id='ingest-address'
                required
                value={address}
                onChange={(event) => setAddress(event.target.value)}
                list='ingest-address-options'
                placeholder='0x…'
                className='font-mono text-sm'
              />
              <datalist id='ingest-address-options'>
                {addresses.map((item) => (
                  <option key={`${item.chainId}:${item.address}`} value={item.address} />
                ))}
              </datalist>
            </div>

            <div className='grid grid-cols-2 gap-3'>
              <div className='space-y-2'>
                <Label htmlFor='ingest-from'>From block</Label>
                <Input
                  id='ingest-from'
                  inputMode='numeric'
                  value={fromBlock}
                  onChange={(event) => setFromBlock(event.target.value)}
                  placeholder='earliest'
                />
              </div>
              <div className='space-y-2'>
                <Label htmlFor='ingest-to'>To block</Label>
                <Input
                  id='ingest-to'
                  inputMode='numeric'
                  value={toBlock}
                  onChange={(event) => setToBlock(event.target.value)}
                  placeholder='latest'
                />
              </div>
            </div>

            <div className='grid grid-cols-3 gap-3'>
              <div className='space-y-2'>
                <Label htmlFor='ingest-chain'>Chain ID</Label>
                <Input
                  id='ingest-chain'
                  inputMode='numeric'
                  value={chainId}
                  onChange={(event) => setChainId(event.target.value)}
                />
              </div>
              <div className='space-y-2'>
                <Label htmlFor='ingest-depth'>Max depth</Label>
                <Input
                  id='ingest-depth'
                  inputMode='numeric'
                  value={maxDepth}
                  onChange={(event) => setMaxDepth(event.target.value)}
                />
              </div>
              <div className='space-y-2'>
                <Label htmlFor='ingest-nodes'>Max nodes</Label>
                <Input
                  id='ingest-nodes'
                  inputMode='numeric'
                  value={maxNodes}
                  onChange={(event) => setMaxNodes(event.target.value)}
                />
              </div>
            </div>
          </div>

          <DialogFooter>
            {phase ? <span className='mr-auto flex items-center gap-2 text-xs text-muted-foreground'><Loader2 className='size-3 animate-spin' />{phase}</span> : null}
            <Button type='submit' disabled={busy || address.trim().length === 0}>
              {busy ? <Loader2 className='animate-spin' /> : <DownloadCloud />}
              Ingest &amp; trace
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
