import type { TransactionEdge, TransactionNodeDetails } from '@/entities/transaction'
import { Badge } from '@/shared/ui/badge'
import {
  Drawer,
  DrawerContent,
  DrawerDescription,
  DrawerHeader,
  DrawerTitle,
} from '@/shared/ui/drawer'
import { Separator } from '@/shared/ui/separator'

type TransactionNodeDetailsDrawerProps = {
  open: boolean
  onOpenChange: (open: boolean) => void
  details: TransactionNodeDetails | null
}

const groupLabels: Record<string, string> = {
  wallet: 'Wallet',
  exchange: 'Exchange',
  risk: 'Risk',
}

export const TransactionNodeDetailsDrawer = ({
  open,
  onOpenChange,
  details,
}: TransactionNodeDetailsDrawerProps) => {
  return (
    <Drawer open={open} onOpenChange={onOpenChange} direction='right'>
      <DrawerContent className='max-h-screen'>
        {details ? (
          <>
            <DrawerHeader className='border-b border-border pb-4'>
              <div className='flex items-center gap-2'>
                <DrawerTitle className='font-mono text-base'>{details.node.label}</DrawerTitle>
                {details.node.group ? (
                  <Badge variant='outline'>{groupLabels[details.node.group] ?? details.node.group}</Badge>
                ) : null}
              </div>
              <DrawerDescription className='break-all font-mono text-xs'>{details.address}</DrawerDescription>
            </DrawerHeader>

            <div className='flex-1 space-y-4 overflow-y-auto p-4'>
              <section className='grid grid-cols-2 gap-3 text-sm'>
                <Stat label='Incoming' value={details.incoming.length} />
                <Stat label='Outgoing' value={details.outgoing.length} />
                <Stat label='Weight' value={details.node.weight?.toFixed(1) ?? '—'} />
                <Stat label='Connections' value={details.incoming.length + details.outgoing.length} />
              </section>

              <Separator />

              <TransactionList title='Incoming' edges={details.incoming} direction='in' />
              <Separator />
              <TransactionList title='Outgoing' edges={details.outgoing} direction='out' />
            </div>
          </>
        ) : (
          <DrawerHeader>
            <DrawerTitle>Node details</DrawerTitle>
            <DrawerDescription>Select a node on the graph to inspect its transactions.</DrawerDescription>
          </DrawerHeader>
        )}
      </DrawerContent>
    </Drawer>
  )
}

function Stat({ label, value }: { label: string; value: string | number }) {
  return (
    <div className='rounded-lg border border-border bg-card/50 px-3 py-2'>
      <p className='text-xs text-muted-foreground'>{label}</p>
      <p className='mt-0.5 font-medium tabular-nums'>{value}</p>
    </div>
  )
}

function TransactionList({
  title,
  edges,
  direction,
}: {
  title: string
  edges: TransactionEdge[]
  direction: 'in' | 'out'
}) {
  if (edges.length === 0) {
    return (
      <section className='space-y-2'>
        <h3 className='text-sm font-medium'>{title}</h3>
        <p className='text-xs text-muted-foreground'>No transactions</p>
      </section>
    )
  }

  return (
    <section className='space-y-2'>
      <h3 className='text-sm font-medium'>
        {title} <span className='text-muted-foreground'>({edges.length})</span>
      </h3>
      <ul className='space-y-2'>
        {edges.map((edge) => (
          <li key={`${edge.tx_hash}-${edge.index}-${direction}`} className='rounded-lg border border-border bg-card/40 p-3'>
            <div className='flex items-center justify-between gap-2'>
              <span className='text-sm font-medium tabular-nums'>
                {edge.formatted} {edge.symbol}
              </span>
              <Badge variant='secondary' className='font-mono text-[10px] uppercase'>
                {edge.kind}
              </Badge>
            </div>
            <p className='mt-1 font-mono text-[11px] text-muted-foreground'>{shortHash(edge.tx_hash)}</p>
            <p className='mt-1 text-[11px] text-muted-foreground'>
              {direction === 'in' ? 'From' : 'To'}: {shortAddress(direction === 'in' ? edge.from : edge.to)}
            </p>
            <p className='text-[11px] text-muted-foreground'>Block {edge.block.toLocaleString()}</p>
          </li>
        ))}
      </ul>
    </section>
  )
}

function shortHash(hash: string) {
  if (hash.length <= 16) {
    return hash
  }
  return `${hash.slice(0, 10)}…${hash.slice(-8)}`
}

function shortAddress(address: string) {
  if (address.length <= 12) {
    return address
  }
  return `${address.slice(0, 6)}…${address.slice(-4)}`
}
