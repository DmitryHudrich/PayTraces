import { AlertTriangle, Boxes, Loader2, ShieldAlert, Sparkles, Target } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'

import {
  firedHeuristics,
  useClusterQuery,
  useEntityQuery,
  useHeuristicsQuery,
  useScoreQuery,
} from '@/entities/graph-insight'
import { useCreateGroupMutation } from '@/entities/group'
import { getErrorMessage } from '@/shared/api'
import { cn } from '@/shared/lib/cn'
import { shortAddress } from '@/shared/lib/format'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'

type NodeInsightsProps = {
  caseId: string
  address: string
  chainId: number
  clusterActive: boolean
  onHighlightCluster: (addresses: string[]) => void
  onClearHighlight: () => void
}

export function NodeInsights({
  caseId,
  address,
  chainId,
  clusterActive,
  onHighlightCluster,
  onClearHighlight,
}: NodeInsightsProps) {
  return (
    <div className='space-y-4'>
      <ScoreCard caseId={caseId} address={address} chainId={chainId} />
      <AutoLabels caseId={caseId} address={address} chainId={chainId} />
      <HeuristicsCard caseId={caseId} address={address} chainId={chainId} />
      <ClusterCard
        caseId={caseId}
        address={address}
        chainId={chainId}
        clusterActive={clusterActive}
        onHighlightCluster={onHighlightCluster}
        onClearHighlight={onClearHighlight}
      />
    </div>
  )
}

function InsightHeader({ icon: Icon, title }: { icon: typeof ShieldAlert; title: string }) {
  return (
    <p className='flex items-center gap-1.5 text-xs font-medium text-muted-foreground'>
      <Icon className='size-3.5' />
      {title}
    </p>
  )
}

function scoreTone(score: number, isHighRisk: boolean) {
  if (isHighRisk || score >= 75) {
    return 'text-destructive'
  }
  if (score >= 50) {
    return 'text-warning'
  }
  if (score >= 25) {
    return 'text-accent'
  }
  return 'text-success'
}

function ScoreCard({ caseId, address, chainId }: { caseId: string; address: string; chainId: number }) {
  const query = useScoreQuery(caseId, chainId, address)

  return (
    <section className='space-y-2 rounded-lg border border-border/70 bg-card/40 p-3'>
      <InsightHeader icon={ShieldAlert} title='Risk score' />
      {query.isPending ? (
        <Spinner />
      ) : query.isError || !query.data ? (
        <p className='text-xs text-muted-foreground'>Unavailable — ingest the address first.</p>
      ) : (
        <>
          <div className='flex items-baseline gap-2'>
            <span className={cn('text-2xl font-semibold tabular-nums', scoreTone(query.data.score, query.data.isHighRisk))}>
              {query.data.score}
            </span>
            <span className='text-xs text-muted-foreground'>/ 100</span>
            {query.data.isHighRisk ? (
              <Badge variant='outline' className='ml-auto border-destructive/30 bg-destructive/15 text-destructive'>
                High risk
              </Badge>
            ) : null}
          </div>
          {query.data.signals.length > 0 ? (
            <ul className='space-y-1'>
              {query.data.signals.map((signal, index) => (
                <li key={`${signal.kind}-${index}`} className='flex items-start gap-1.5 text-xs'>
                  <span className='mt-1 size-1.5 shrink-0 rounded-full bg-muted-foreground' />
                  <span>
                    <span className='font-medium'>{signal.kind}</span>
                    <span className='text-muted-foreground'> · sev {signal.severity}</span>
                    <span className='block text-muted-foreground'>{signal.description}</span>
                  </span>
                </li>
              ))}
            </ul>
          ) : (
            <p className='text-xs text-muted-foreground'>No contributing signals.</p>
          )}
        </>
      )}
    </section>
  )
}

function AutoLabels({ caseId, address, chainId }: { caseId: string; address: string; chainId: number }) {
  const query = useEntityQuery(caseId, chainId, address)
  const tags = (query.data?.tags ?? []).filter((tag) => tag.active)

  return (
    <section className='space-y-2 rounded-lg border border-border/70 bg-card/40 p-3'>
      <InsightHeader icon={Sparkles} title='Auto-labels' />
      {query.isPending ? (
        <Spinner />
      ) : !query.data || tags.length === 0 ? (
        <p className='text-xs text-muted-foreground'>No automatic labels for this address.</p>
      ) : (
        <div className='flex flex-wrap gap-1.5'>
          {tags.map((tag) => (
            <Badge key={tag.tagId} variant='outline' className='gap-1'>
              <span className='font-medium capitalize'>{tag.category}</span>
              {tag.labelName ? <span className='text-muted-foreground'>· {tag.labelName}</span> : null}
              {tag.sanctionList ? (
                <span className='rounded bg-destructive/15 px-1 text-destructive uppercase'>{tag.sanctionList}</span>
              ) : null}
            </Badge>
          ))}
        </div>
      )}
    </section>
  )
}

function HeuristicsCard({ caseId, address, chainId }: { caseId: string; address: string; chainId: number }) {
  const query = useHeuristicsQuery(caseId, chainId, address)
  const fired = query.data ? firedHeuristics(query.data) : []

  return (
    <section className='space-y-2 rounded-lg border border-border/70 bg-card/40 p-3'>
      <InsightHeader icon={AlertTriangle} title='Heuristics' />
      {query.isPending ? (
        <Spinner />
      ) : !query.data || fired.length === 0 ? (
        <p className='text-xs text-muted-foreground'>No behavioural patterns fired.</p>
      ) : (
        <ul className='space-y-1.5'>
          {fired.map((item) => (
            <li key={item.label} className='rounded-md border border-warning/20 bg-warning/5 px-2 py-1.5'>
              <div className='flex items-center justify-between gap-2'>
                <span className='text-xs font-medium'>{item.label}</span>
                <Badge variant='outline' className='border-warning/30 text-warning'>
                  {item.evidence.confidence}
                </Badge>
              </div>
              <p className='mt-0.5 text-[11px] text-muted-foreground'>
                {item.evidence.notes ?? `${item.evidence.addresses.length} counterparties`}
              </p>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}

function ClusterCard({
  caseId,
  address,
  chainId,
  clusterActive,
  onHighlightCluster,
  onClearHighlight,
}: NodeInsightsProps) {
  const [enabled, setEnabled] = useState(false)
  const query = useClusterQuery(caseId, chainId, address, enabled)
  const createGroup = useCreateGroupMutation(caseId)

  const components = query.data?.components ?? []

  const saveAsGroup = (members: string[], index: number) => {
    createGroup.mutate(
      {
        name: `Cluster ${shortAddress(address, 6, 4)} #${index + 1}`,
        members: members.map((member) => ({ address: member, chainId })),
      },
      {
        onSuccess: () => toast.success('Cluster saved as group'),
        onError: (error) => toast.error(getErrorMessage(error, 'Failed to save cluster.')),
      },
    )
  }

  return (
    <section className='space-y-2 rounded-lg border border-border/70 bg-card/40 p-3'>
      <div className='flex items-center justify-between'>
        <InsightHeader icon={Boxes} title='Co-ownership cluster' />
        {clusterActive ? (
          <button type='button' className='text-xs text-accent hover:underline' onClick={onClearHighlight}>
            Clear
          </button>
        ) : null}
      </div>

      {!enabled ? (
        <Button size='sm' variant='secondary' className='w-full' onClick={() => setEnabled(true)}>
          <Target />
          Find cluster
        </Button>
      ) : query.isPending ? (
        <Spinner />
      ) : query.isError || components.length === 0 ? (
        <p className='text-xs text-muted-foreground'>No cluster found — ingest the address first.</p>
      ) : (
        <ul className='space-y-1.5'>
          {components.slice(0, 6).map((members, index) => (
            <li key={index} className='rounded-md border border-border/60 bg-background/40 px-2 py-1.5'>
              <div className='flex items-center justify-between gap-2'>
                <span className='text-xs font-medium'>
                  {index === 0 ? 'Primary' : `Satellite ${index}`} · {members.length}
                </span>
                <div className='flex items-center gap-1'>
                  <Button
                    size='sm'
                    variant='ghost'
                    className='h-6 px-2 text-xs'
                    onClick={() => onHighlightCluster(members)}
                  >
                    Highlight
                  </Button>
                  <Button
                    size='sm'
                    variant='ghost'
                    className='h-6 px-2 text-xs'
                    onClick={() => saveAsGroup(members, index)}
                    disabled={createGroup.isPending}
                  >
                    Save
                  </Button>
                </div>
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  )
}

function Spinner() {
  return (
    <div className='flex items-center gap-2 text-xs text-muted-foreground'>
      <Loader2 className='size-3 animate-spin' /> Loading…
    </div>
  )
}
