import { ArrowDownLeft, ArrowUpRight, Check, Copy, Loader2, Maximize2, MousePointerClick } from 'lucide-react'
import { useState, type ReactNode } from 'react'

import { RISK_BAND_LABEL, riskBand, riskBandClasses, type NodeDetails } from '@/entities/case-graph'
import { AddressLabelsControl } from '@/features/label-manager'
import { AddToGroupControl } from '@/features/group-manager'
import { AddAddressDialog } from '@/features/case-address-add'
import { NodeInsights } from '@/widgets/case-graph/ui/NodeInsights'
import { cn } from '@/shared/lib/cn'
import { copyToClipboard } from '@/shared/lib/copy-to-clipboard'
import { shortAddress } from '@/shared/lib/format'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { ScrollArea } from '@/shared/ui/scroll-area'

type NodeInspectorProps = {
  caseId: string
  details: NodeDetails | null
  chainId: number
  canApplyLabel: boolean
  canAddAddress: boolean
  isExpanding: boolean
  clusterActive: boolean
  onExpand: (address: string, chainId: number) => void
  onHighlightCluster: (addresses: string[]) => void
  onClearHighlight: () => void
}

export function NodeInspector({
  caseId,
  details,
  chainId,
  canApplyLabel,
  canAddAddress,
  isExpanding,
  clusterActive,
  onExpand,
  onHighlightCluster,
  onClearHighlight,
}: NodeInspectorProps) {
  const [copied, setCopied] = useState(false)

  if (!details) {
    return (
      <EmptyState
        icon={MousePointerClick}
        title='No node selected'
        description='Click a node in the graph to inspect its risk, flows and labels.'
        className='m-4'
      />
    )
  }

  const { node, address } = details
  const band = node ? riskBand(node) : 'unknown'

  const copy = async () => {
    await copyToClipboard(address)
    setCopied(true)
    window.setTimeout(() => setCopied(false), 1200)
  }

  return (
    <ScrollArea className='h-full'>
      <div className='space-y-4 p-4'>
        <div>
          <div className='flex items-center gap-2'>
            <Badge variant='outline' className={riskBandClasses(band)}>
              {RISK_BAND_LABEL[band]}
              {node?.riskScore != null ? ` · ${node.riskScore}` : ''}
            </Badge>
            {node?.serviceName ? <Badge variant='secondary'>{node.serviceName}</Badge> : null}
            {node?.kind ? <Badge variant='outline'>{node.kind}</Badge> : null}
          </div>
          <div className='mt-2 flex items-start gap-2'>
            <p className='min-w-0 flex-1 font-mono text-sm break-all'>{address}</p>
            <Button size='icon' variant='ghost' className='size-7 shrink-0' onClick={copy} title='Copy address'>
              {copied ? <Check className='text-success' /> : <Copy />}
            </Button>
          </div>
        </div>

        <div className='grid grid-cols-2 gap-2'>
          <Stat label='In degree' value={node?.inDegree ?? details.inboundCount} />
          <Stat label='Out degree' value={node?.outDegree ?? details.outboundCount} />
          <Stat label='Tx count' value={node?.txCount ?? '—'} />
          <Stat label='Edges here' value={details.inboundCount + details.outboundCount} />
        </div>

        <div className='flex flex-wrap gap-2'>
          <Button size='sm' variant='secondary' onClick={() => onExpand(address, chainId)} disabled={isExpanding}>
            {isExpanding ? <Loader2 className='animate-spin' /> : <Maximize2 />}
            Expand
          </Button>
          {canAddAddress ? (
            <AddAddressDialog
              caseId={caseId}
              defaultAddress={address}
              defaultChainId={chainId}
              trigger={
                <Button size='sm' variant='outline'>
                  Add to case
                </Button>
              }
            />
          ) : null}
        </div>

        <NodeInsights
          caseId={caseId}
          address={address}
          chainId={chainId}
          clusterActive={clusterActive}
          onHighlightCluster={onHighlightCluster}
          onClearHighlight={onClearHighlight}
        />

        <Section title='Labels'>
          <AddressLabelsControl caseId={caseId} address={address} chainId={chainId} canApply={canApplyLabel} />
        </Section>

        <Section title='Groups'>
          <AddToGroupControl caseId={caseId} address={address} chainId={chainId} />
        </Section>

        <Section title={`Flows (${details.inboundCount + details.outboundCount})`}>
          <div className='space-y-1'>
            {details.inbound.length === 0 && details.outbound.length === 0 ? (
              <p className='text-xs text-muted-foreground'>No transfers in the current graph.</p>
            ) : (
              <>
                {details.inbound.slice(0, 12).map((flow, index) => (
                  <FlowRow key={`in-${index}`} direction='in' counterparty={flow.counterparty} amount={`${flow.edge.formatted} ${flow.edge.symbol}`} />
                ))}
                {details.outbound.slice(0, 12).map((flow, index) => (
                  <FlowRow key={`out-${index}`} direction='out' counterparty={flow.counterparty} amount={`${flow.edge.formatted} ${flow.edge.symbol}`} />
                ))}
              </>
            )}
          </div>
        </Section>
      </div>
    </ScrollArea>
  )
}

function Stat({ label, value }: { label: string; value: string | number }) {
  return (
    <div className='rounded-md border border-border/70 bg-card/40 px-3 py-2'>
      <p className='text-[11px] text-muted-foreground'>{label}</p>
      <p className='text-sm font-semibold tabular-nums'>{value}</p>
    </div>
  )
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className='space-y-2'>
      <p className='text-xs font-medium text-muted-foreground'>{title}</p>
      {children}
    </div>
  )
}

function FlowRow({
  direction,
  counterparty,
  amount,
}: {
  direction: 'in' | 'out'
  counterparty: string
  amount: string
}) {
  const isIn = direction === 'in'
  return (
    <div className='flex items-center justify-between gap-2 rounded-md border border-border/50 bg-card/30 px-2 py-1.5 text-xs'>
      <span className='flex min-w-0 items-center gap-1.5'>
        <span
          className={cn(
            'flex size-5 shrink-0 items-center justify-center rounded',
            isIn ? 'bg-success/15 text-success' : 'bg-warning/15 text-warning',
          )}
        >
          {isIn ? <ArrowDownLeft className='size-3' /> : <ArrowUpRight className='size-3' />}
        </span>
        <span className='truncate font-mono text-muted-foreground'>{shortAddress(counterparty, 8, 6)}</span>
      </span>
      <span className='shrink-0 tabular-nums'>{amount}</span>
    </div>
  )
}
