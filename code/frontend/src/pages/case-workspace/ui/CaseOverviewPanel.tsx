import { StickyNote, Wallet } from 'lucide-react'

import type { CaseDetail } from '@/entities/case'
import { AddAddressDialog } from '@/features/case-address-add'
import { AssignMemberDialog } from '@/features/case-assign'
import { formatDateTime, shortAddress } from '@/shared/lib/format'
import { Avatar } from '@/shared/ui/avatar'
import { ScrollArea } from '@/shared/ui/scroll-area'

type CaseOverviewPanelProps = {
  detail: CaseDetail
  canAddAddress: boolean
  canAssign: boolean
  onFocusAddress: (address: string, chainId: number) => void
}

export function CaseOverviewPanel({ detail, canAddAddress, canAssign, onFocusAddress }: CaseOverviewPanelProps) {
  return (
    <ScrollArea className='h-full'>
      <div className='space-y-5 p-4'>
        {detail.description ? (
          <section className='space-y-1.5'>
            <h3 className='text-xs font-medium text-muted-foreground'>Description</h3>
            <p className='text-sm text-foreground/90'>{detail.description}</p>
          </section>
        ) : null}

        <section className='space-y-2'>
          <div className='flex items-center justify-between'>
            <h3 className='text-xs font-medium text-muted-foreground'>Addresses · {detail.addresses.length}</h3>
            {canAddAddress ? <AddAddressDialog caseId={detail.id} /> : null}
          </div>
          {detail.addresses.length === 0 ? (
            <p className='flex items-center gap-2 rounded-md border border-dashed border-border/70 px-3 py-4 text-xs text-muted-foreground'>
              <Wallet className='size-4' /> No addresses recorded yet.
            </p>
          ) : (
            <ul className='space-y-1.5'>
              {detail.addresses.map((item) => (
                <li key={`${item.chainId}:${item.address}`}>
                  <button
                    type='button'
                    onClick={() => onFocusAddress(item.address, item.chainId)}
                    className='w-full rounded-md border border-border/70 bg-card/40 px-3 py-2 text-left transition-colors hover:border-primary/40'
                  >
                    <div className='flex items-center justify-between gap-2'>
                      <span className='font-mono text-xs'>{shortAddress(item.address, 12, 8)}</span>
                      <span className='text-[11px] text-muted-foreground'>chain {item.chainId}</span>
                    </div>
                    {item.note ? <p className='mt-1 truncate text-xs text-muted-foreground'>{item.note}</p> : null}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>

        <section className='space-y-2'>
          <div className='flex items-center justify-between'>
            <h3 className='text-xs font-medium text-muted-foreground'>Team · {detail.assignments.length}</h3>
            {canAssign ? <AssignMemberDialog caseId={detail.id} /> : null}
          </div>
          {detail.assignments.length === 0 ? (
            <p className='rounded-md border border-dashed border-border/70 px-3 py-4 text-xs text-muted-foreground'>
              No members assigned.
            </p>
          ) : (
            <ul className='space-y-1.5'>
              {detail.assignments.map((assignment) => (
                <li
                  key={`${assignment.userId}:${assignment.roleName}`}
                  className='flex items-center gap-2 rounded-md border border-border/70 bg-card/40 px-3 py-2'
                >
                  <Avatar className='size-7'>{assignment.userId.slice(0, 2).toUpperCase()}</Avatar>
                  <div className='min-w-0 flex-1'>
                    <p className='truncate font-mono text-xs'>{shortAddress(assignment.userId, 8, 6)}</p>
                    <p className='text-[11px] text-muted-foreground'>{assignment.roleName}</p>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </section>

        {detail.notes.length > 0 ? (
          <section className='space-y-2'>
            <h3 className='text-xs font-medium text-muted-foreground'>Notes · {detail.notes.length}</h3>
            <ul className='space-y-1.5'>
              {detail.notes.map((note) => (
                <li key={note.id} className='rounded-md border border-border/70 bg-card/40 px-3 py-2'>
                  <p className='flex items-center gap-1.5 text-[11px] text-muted-foreground'>
                    <StickyNote className='size-3' />
                    {formatDateTime(note.createdAt)}
                  </p>
                  <p className='mt-1 text-sm'>{note.text}</p>
                </li>
              ))}
            </ul>
          </section>
        ) : null}

        <p className='pt-2 text-[11px] text-muted-foreground'>Created {formatDateTime(detail.createdAt)}</p>
      </div>
    </ScrollArea>
  )
}
