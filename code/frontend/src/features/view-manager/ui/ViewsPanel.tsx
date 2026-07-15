import { Check, Loader2, Pencil, Save, Share2, Trash2, Users, X } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'

import {
  useDeleteViewMutation,
  useRenameViewMutation,
  useSetViewSharingMutation,
  useViewsQuery,
  type CaseGraphViewSummary,
} from '@/entities/view'
import { getErrorMessage } from '@/shared/api'
import { cn } from '@/shared/lib/cn'
import { formatRelative } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { Input } from '@/shared/ui/input'

type ViewsPanelProps = {
  caseId: string
  canCreate: boolean
  canManage: boolean
  hasGraph: boolean
  activeViewId: string | null
  onSaveCurrent: (name: string, isShared: boolean) => Promise<void>
  onApplyView: (view: CaseGraphViewSummary) => void
}

export function ViewsPanel({
  caseId,
  canCreate,
  canManage,
  hasGraph,
  activeViewId,
  onSaveCurrent,
  onApplyView,
}: ViewsPanelProps) {
  const viewsQuery = useViewsQuery(caseId)
  const setSharing = useSetViewSharingMutation(caseId)
  const deleteView = useDeleteViewMutation(caseId)
  const renameView = useRenameViewMutation(caseId)

  const [name, setName] = useState('')
  const [shared, setShared] = useState(false)
  const [saving, setSaving] = useState(false)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [editName, setEditName] = useState('')

  const commitRename = (viewId: string) => {
    const trimmed = editName.trim()
    setEditingId(null)
    if (!trimmed) {
      return
    }
    renameView.mutate(
      { viewId, name: trimmed },
      {
        onSuccess: () => toast.success('View renamed'),
        onError: (error) => toast.error(getErrorMessage(error, 'Failed to rename view.')),
      },
    )
  }

  const save = async () => {
    const trimmed = name.trim()
    if (!trimmed) {
      return
    }
    setSaving(true)
    try {
      await onSaveCurrent(trimmed, shared)
      setName('')
      setShared(false)
      toast.success('View saved')
    } catch (error) {
      toast.error(getErrorMessage(error, 'Could not save the view.'))
    } finally {
      setSaving(false)
    }
  }

  const views = viewsQuery.data ?? []

  return (
    <div className='flex flex-col gap-4'>
      {canCreate ? (
        <div className='space-y-2 rounded-lg border border-border/70 bg-card/40 p-3'>
          <p className='text-xs font-medium text-muted-foreground'>Save current canvas</p>
          <Input
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder='View name'
            className='h-8 text-sm'
            disabled={!hasGraph}
          />
          <div className='flex items-center justify-between'>
            <button
              type='button'
              onClick={() => setShared((value) => !value)}
              className={cn(
                'inline-flex items-center gap-1.5 rounded-md border px-2 py-1 text-xs transition-colors',
                shared ? 'border-accent/40 bg-accent/15 text-accent' : 'border-border text-muted-foreground',
              )}
              disabled={!hasGraph}
            >
              <Users className='size-3.5' />
              {shared ? 'Shared' : 'Private'}
            </button>
            <Button size='sm' className='h-8' onClick={save} disabled={!hasGraph || saving || !name.trim()}>
              {saving ? <Loader2 className='animate-spin' /> : <Save />}
              Save
            </Button>
          </div>
          {!hasGraph ? <p className='text-xs text-muted-foreground'>Trace a graph first to save positions.</p> : null}
        </div>
      ) : null}

      {viewsQuery.isPending ? (
        <div className='flex items-center gap-2 text-xs text-muted-foreground'>
          <Loader2 className='size-3 animate-spin' /> Loading views…
        </div>
      ) : views.length === 0 ? (
        <EmptyState title='No saved views' description='Arrange the canvas and save it as a view.' />
      ) : (
        <ul className='space-y-2'>
          {views.map((view) => {
            const isActive = view.id === activeViewId
            return (
              <li
                key={view.id}
                className={cn(
                  'rounded-lg border p-3 transition-colors',
                  isActive ? 'border-primary/50 bg-primary/5' : 'border-border/70 bg-card/40',
                )}
              >
                {editingId === view.id ? (
                  <div className='flex items-center gap-1'>
                    <Input
                      autoFocus
                      value={editName}
                      onChange={(event) => setEditName(event.target.value)}
                      onKeyDown={(event) => {
                        if (event.key === 'Enter') {
                          event.preventDefault()
                          commitRename(view.id)
                        }
                        if (event.key === 'Escape') {
                          setEditingId(null)
                        }
                      }}
                      className='h-8 text-sm'
                    />
                    <Button size='icon' variant='ghost' className='size-7' onClick={() => commitRename(view.id)}>
                      <Check />
                    </Button>
                    <Button size='icon' variant='ghost' className='size-7' onClick={() => setEditingId(null)}>
                      <X />
                    </Button>
                  </div>
                ) : (
                  <div className='flex items-start justify-between gap-2'>
                    <button type='button' className='min-w-0 flex-1 text-left' onClick={() => onApplyView(view)}>
                      <div className='flex items-center gap-1.5'>
                        <span className='truncate text-sm font-medium'>{view.name}</span>
                        {isActive ? <Check className='size-3.5 shrink-0 text-primary' /> : null}
                      </div>
                      <p className='mt-0.5 text-xs text-muted-foreground'>
                        {view.pinnedCount} pinned · {formatRelative(view.createdAt)}
                      </p>
                    </button>
                    {canManage ? (
                      <div className='flex shrink-0 items-center gap-1'>
                        <Button
                          size='icon'
                          variant='ghost'
                          className='size-7'
                          title='Rename view'
                          onClick={() => {
                            setEditingId(view.id)
                            setEditName(view.name)
                          }}
                        >
                          <Pencil />
                        </Button>
                        <Button
                          size='icon'
                          variant='ghost'
                          className={cn('size-7', view.isShared && 'text-accent')}
                          title={view.isShared ? 'Shared — make private' : 'Private — share'}
                          onClick={() =>
                            setSharing.mutate(
                              { viewId: view.id, isShared: !view.isShared },
                              { onError: (error) => toast.error(getErrorMessage(error, 'Failed to update sharing.')) },
                            )
                          }
                        >
                          <Share2 />
                        </Button>
                        <Button
                          size='icon'
                          variant='ghost'
                          className='size-7 text-muted-foreground hover:text-destructive'
                          title='Delete view'
                          onClick={() =>
                            deleteView.mutate(view.id, {
                              onSuccess: () => toast.success('View deleted'),
                              onError: (error) => toast.error(getErrorMessage(error, 'Failed to delete view.')),
                            })
                          }
                        >
                          <Trash2 />
                        </Button>
                      </div>
                    ) : null}
                  </div>
                )}
              </li>
            )
          })}
        </ul>
      )}
    </div>
  )
}
