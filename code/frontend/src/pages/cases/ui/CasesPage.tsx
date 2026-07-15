import { AlertCircle, FolderOpen, Loader2, Search } from 'lucide-react'
import { useMemo, useState } from 'react'
import { Link } from 'react-router-dom'

import {
  priorityClasses,
  statusClasses,
  STATUS_LABEL,
  useCasesQuery,
  type CaseSummary,
} from '@/entities/case'
import { CreateCaseDialog } from '@/features/case-create'
import { getErrorMessage } from '@/shared/api'
import { cn } from '@/shared/lib/cn'
import { formatRelative } from '@/shared/lib/format'
import { useDebouncedValue } from '@/shared/lib/use-debounced-value'
import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { Badge } from '@/shared/ui/badge'
import { EmptyState } from '@/shared/ui/empty-state'
import { Input } from '@/shared/ui/input'
import { ScrollArea } from '@/shared/ui/scroll-area'
import { Skeleton } from '@/shared/ui/skeleton'

export function CasesPage() {
  const casesQuery = useCasesQuery()
  const [query, setQuery] = useState('')
  const debounced = useDebouncedValue(query, 200)

  const cases = useMemo(() => {
    const list = casesQuery.data ?? []
    const term = debounced.trim().toLowerCase()
    const filtered = term ? list.filter((item) => item.title.toLowerCase().includes(term)) : list
    return [...filtered].sort((a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime())
  }, [casesQuery.data, debounced])

  return (
    <ScrollArea className='h-full'>
      <div className='mx-auto w-full max-w-6xl px-4 py-8'>
        <div className='flex flex-wrap items-end justify-between gap-4'>
          <div className='space-y-1'>
            <h1 className='text-2xl font-semibold tracking-tight'>Cases</h1>
            <p className='text-sm text-muted-foreground'>Your organization's active and archived investigations.</p>
          </div>
          <CreateCaseDialog />
        </div>

        <div className='relative mt-6 max-w-sm'>
          <Search className='pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground' />
          <Input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder='Search cases…'
            className='pl-9'
          />
        </div>

        <div className='mt-6'>
          {casesQuery.isPending ? (
            <div className='grid gap-3 sm:grid-cols-2 lg:grid-cols-3'>
              {Array.from({ length: 6 }).map((_, index) => (
                <Skeleton key={index} className='h-36 rounded-xl' />
              ))}
            </div>
          ) : casesQuery.isError ? (
            <Alert variant='destructive'>
              <AlertCircle />
              <AlertTitle>Could not load cases</AlertTitle>
              <AlertDescription>{getErrorMessage(casesQuery.error, 'Request failed.')}</AlertDescription>
            </Alert>
          ) : cases.length === 0 ? (
            <EmptyState
              icon={FolderOpen}
              title={debounced ? 'No matching cases' : 'No cases yet'}
              description={
                debounced
                  ? 'Try a different search term.'
                  : 'Create your first case to start tracing on-chain activity.'
              }
              action={debounced ? undefined : <CreateCaseDialog />}
            />
          ) : (
            <div className='grid gap-3 sm:grid-cols-2 lg:grid-cols-3'>
              {cases.map((item) => (
                <CaseCard key={item.id} item={item} />
              ))}
            </div>
          )}
        </div>

        {casesQuery.isFetching && !casesQuery.isPending ? (
          <div className='mt-4 flex items-center gap-2 text-xs text-muted-foreground'>
            <Loader2 className='size-3 animate-spin' /> Refreshing…
          </div>
        ) : null}
      </div>
    </ScrollArea>
  )
}

function CaseCard({ item }: { item: CaseSummary }) {
  return (
    <Link
      to={`/cases/${item.id}`}
      className='group flex flex-col gap-3 rounded-xl border border-border/70 bg-card/60 p-4 transition-colors hover:border-primary/50 hover:bg-card'
    >
      <div className='flex items-start justify-between gap-2'>
        <h3 className='line-clamp-2 font-medium leading-snug group-hover:text-foreground'>{item.title}</h3>
        <Badge variant='outline' className={cn('shrink-0', priorityClasses(item.priority))}>
          {item.priority}
        </Badge>
      </div>
      <div className='mt-auto flex items-center justify-between'>
        <Badge variant='outline' className={statusClasses(item.status)}>
          {STATUS_LABEL[item.status]}
        </Badge>
        <span className='text-xs text-muted-foreground'>{formatRelative(item.createdAt)}</span>
      </div>
    </Link>
  )
}
