import { Loader2, Plus, Tag, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'

import {
  LABEL_COLORS,
  labelColor,
  useCaseLabelsQuery,
  useCreateLabelMutation,
  useDeleteLabelMutation,
} from '@/entities/case-label'
import { getErrorMessage } from '@/shared/api'
import { cn } from '@/shared/lib/cn'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { Input } from '@/shared/ui/input'

export function LabelsPanel({ caseId, canCreate }: { caseId: string; canCreate: boolean }) {
  const labelsQuery = useCaseLabelsQuery(caseId)
  const createLabel = useCreateLabelMutation(caseId)
  const deleteLabel = useDeleteLabelMutation(caseId)

  const [text, setText] = useState('')
  const [color, setColor] = useState<string>(LABEL_COLORS[0])

  const create = async () => {
    const trimmed = text.trim()
    if (!trimmed) {
      return
    }
    try {
      await createLabel.mutateAsync({ text: trimmed, color })
      setText('')
      toast.success('Label created')
    } catch (error) {
      toast.error(getErrorMessage(error, 'Could not create the label.'))
    }
  }

  const labels = labelsQuery.data ?? []

  return (
    <div className='flex flex-col gap-4'>
      {canCreate ? (
        <div className='space-y-2 rounded-lg border border-border/70 bg-card/40 p-3'>
          <p className='text-xs font-medium text-muted-foreground'>New label</p>
          <div className='flex gap-2'>
            <Input
              value={text}
              onChange={(event) => setText(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  event.preventDefault()
                  void create()
                }
              }}
              placeholder='Mixer, Exchange, Victim…'
              className='h-8 text-sm'
            />
            <Button size='sm' className='h-8' onClick={create} disabled={createLabel.isPending || !text.trim()}>
              {createLabel.isPending ? <Loader2 className='animate-spin' /> : <Plus />}
            </Button>
          </div>
          <div className='flex flex-wrap gap-1.5'>
            {LABEL_COLORS.map((value) => (
              <button
                key={value}
                type='button'
                onClick={() => setColor(value)}
                className={cn(
                  'size-5 rounded-full border-2 transition-transform',
                  color === value ? 'scale-110 border-foreground' : 'border-transparent',
                )}
                style={{ backgroundColor: value }}
                aria-label={`Colour ${value}`}
              />
            ))}
          </div>
        </div>
      ) : null}

      {labelsQuery.isPending ? (
        <div className='flex items-center gap-2 text-xs text-muted-foreground'>
          <Loader2 className='size-3 animate-spin' /> Loading labels…
        </div>
      ) : labels.length === 0 ? (
        <EmptyState icon={Tag} title='No labels' description='Create labels to annotate addresses.' />
      ) : (
        <ul className='space-y-1.5'>
          {labels.map((label) => (
            <li
              key={label.id}
              className='flex items-center justify-between gap-2 rounded-md border border-border/70 bg-card/40 px-3 py-2'
            >
              <span className='flex min-w-0 items-center gap-2'>
                <span className='size-2.5 shrink-0 rounded-full' style={{ backgroundColor: labelColor(label.color) }} />
                <span className='truncate text-sm'>{label.text}</span>
              </span>
              <Button
                size='icon'
                variant='ghost'
                className='size-7 text-muted-foreground hover:text-destructive'
                title='Delete label'
                onClick={() =>
                  deleteLabel.mutate(label.id, {
                    onSuccess: () => toast.success('Label deleted'),
                    onError: (error) => toast.error(getErrorMessage(error, 'Failed to delete label.')),
                  })
                }
              >
                <Trash2 />
              </Button>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
