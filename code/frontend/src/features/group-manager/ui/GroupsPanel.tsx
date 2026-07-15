import { Boxes, Check, ChevronRight, Loader2, Pencil, Plus, Trash2, X } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'

import {
  useAddGroupMemberMutation,
  useCreateGroupMutation,
  useDeleteGroupMutation,
  useGroupQuery,
  useGroupsQuery,
  useRemoveGroupMemberMutation,
  useRenameGroupMutation,
} from '@/entities/group'
import { getErrorMessage } from '@/shared/api'
import { cn } from '@/shared/lib/cn'
import { shortAddress } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { EmptyState } from '@/shared/ui/empty-state'
import { Input } from '@/shared/ui/input'

export function GroupsPanel({
  caseId,
  canCreate,
  canManage,
}: {
  caseId: string
  canCreate: boolean
  canManage: boolean
}) {
  const groupsQuery = useGroupsQuery(caseId)
  const createGroup = useCreateGroupMutation(caseId)
  const deleteGroup = useDeleteGroupMutation(caseId)
  const renameGroup = useRenameGroupMutation(caseId)

  const [name, setName] = useState('')
  const [expanded, setExpanded] = useState<string | null>(null)
  const [editingId, setEditingId] = useState<string | null>(null)
  const [editName, setEditName] = useState('')

  const commitRename = (groupId: string) => {
    const trimmed = editName.trim()
    setEditingId(null)
    if (!trimmed) {
      return
    }
    renameGroup.mutate(
      { groupId, name: trimmed },
      {
        onSuccess: () => toast.success('Group renamed'),
        onError: (error) => toast.error(getErrorMessage(error, 'Failed to rename group.')),
      },
    )
  }

  const create = async () => {
    const trimmed = name.trim()
    if (!trimmed) {
      return
    }
    try {
      await createGroup.mutateAsync({ name: trimmed })
      setName('')
      toast.success('Group created')
    } catch (error) {
      toast.error(getErrorMessage(error, 'Could not create the group.'))
    }
  }

  const groups = groupsQuery.data ?? []

  return (
    <div className='flex flex-col gap-4'>
      {canCreate ? (
        <div className='flex gap-2'>
          <Input
            value={name}
            onChange={(event) => setName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                event.preventDefault()
                void create()
              }
            }}
            placeholder='New group name'
            className='h-8 text-sm'
          />
          <Button size='sm' className='h-8' onClick={create} disabled={createGroup.isPending || !name.trim()}>
            {createGroup.isPending ? <Loader2 className='animate-spin' /> : <Plus />}
          </Button>
        </div>
      ) : null}

      {groupsQuery.isPending ? (
        <div className='flex items-center gap-2 text-xs text-muted-foreground'>
          <Loader2 className='size-3 animate-spin' /> Loading groups…
        </div>
      ) : groups.length === 0 ? (
        <EmptyState icon={Boxes} title='No groups' description='Cluster related addresses into named groups.' />
      ) : (
        <ul className='space-y-2'>
          {groups.map((group) => {
            const isOpen = expanded === group.id
            return (
              <li key={group.id} className='rounded-lg border border-border/70 bg-card/40'>
                {editingId === group.id ? (
                  <div className='flex items-center gap-1 px-3 py-2'>
                    <Input
                      autoFocus
                      value={editName}
                      onChange={(event) => setEditName(event.target.value)}
                      onKeyDown={(event) => {
                        if (event.key === 'Enter') {
                          event.preventDefault()
                          commitRename(group.id)
                        }
                        if (event.key === 'Escape') {
                          setEditingId(null)
                        }
                      }}
                      className='h-8 text-sm'
                    />
                    <Button size='icon' variant='ghost' className='size-7' onClick={() => commitRename(group.id)}>
                      <Check />
                    </Button>
                    <Button size='icon' variant='ghost' className='size-7' onClick={() => setEditingId(null)}>
                      <X />
                    </Button>
                  </div>
                ) : (
                  <div className='flex items-center justify-between gap-2 px-3 py-2'>
                    <button
                      type='button'
                      className='flex min-w-0 flex-1 items-center gap-1.5 text-left'
                      onClick={() => setExpanded(isOpen ? null : group.id)}
                    >
                      <ChevronRight className={cn('size-3.5 shrink-0 transition-transform', isOpen && 'rotate-90')} />
                      <span className='truncate text-sm font-medium'>{group.name}</span>
                      <span className='shrink-0 text-xs text-muted-foreground'>· {group.memberCount}</span>
                    </button>
                    {canManage ? (
                      <div className='flex shrink-0 items-center gap-1'>
                        <Button
                          size='icon'
                          variant='ghost'
                          className='size-7'
                          title='Rename group'
                          onClick={() => {
                            setEditingId(group.id)
                            setEditName(group.name)
                          }}
                        >
                          <Pencil />
                        </Button>
                        <Button
                          size='icon'
                          variant='ghost'
                          className='size-7 text-muted-foreground hover:text-destructive'
                          title='Delete group'
                          onClick={() =>
                            deleteGroup.mutate(group.id, {
                              onSuccess: () => toast.success('Group deleted'),
                              onError: (error) => toast.error(getErrorMessage(error, 'Failed to delete group.')),
                            })
                          }
                        >
                          <Trash2 />
                        </Button>
                      </div>
                    ) : null}
                  </div>
                )}
                {isOpen ? <GroupMembers caseId={caseId} groupId={group.id} canManage={canManage} /> : null}
              </li>
            )
          })}
        </ul>
      )}
    </div>
  )
}

function GroupMembers({ caseId, groupId, canManage }: { caseId: string; groupId: string; canManage: boolean }) {
  const groupQuery = useGroupQuery(caseId, groupId)
  const removeMember = useRemoveGroupMemberMutation(caseId)
  const addMember = useAddGroupMemberMutation(caseId)
  const [address, setAddress] = useState('')
  const [chainId, setChainId] = useState('1')

  const submitMember = () => {
    const trimmed = address.trim()
    if (!trimmed) {
      return
    }
    addMember.mutate(
      { groupId, address: trimmed, chainId: Number(chainId) || 1 },
      {
        onSuccess: () => {
          setAddress('')
          toast.success('Member added')
        },
        onError: (error) => toast.error(getErrorMessage(error, 'Failed to add member.')),
      },
    )
  }

  if (groupQuery.isPending) {
    return (
      <div className='flex items-center gap-2 border-t border-border/70 px-3 py-2 text-xs text-muted-foreground'>
        <Loader2 className='size-3 animate-spin' /> Loading members…
      </div>
    )
  }

  const members = groupQuery.data?.members ?? []

  return (
    <div className='border-t border-border/70'>
      {members.length === 0 ? (
        <p className='px-3 py-2 text-xs text-muted-foreground'>No members yet.</p>
      ) : (
        <ul>
          {members.map((member) => (
            <li
              key={`${member.chainId}:${member.address}`}
              className='flex items-center justify-between gap-2 px-3 py-1.5'
            >
              <span className='truncate font-mono text-xs text-muted-foreground'>
                {shortAddress(member.address, 10, 6)}
              </span>
              {canManage ? (
                <button
                  type='button'
                  className='rounded p-0.5 text-muted-foreground hover:text-destructive'
                  title='Remove member'
                  onClick={() =>
                    removeMember.mutate(
                      { groupId, chainId: member.chainId, address: member.address },
                      { onError: (error) => toast.error(getErrorMessage(error, 'Failed to remove member.')) },
                    )
                  }
                >
                  <X className='size-3.5' />
                </button>
              ) : null}
            </li>
          ))}
        </ul>
      )}

      {canManage ? (
        <div className='flex items-center gap-1 px-3 py-2'>
          <Input
            value={address}
            onChange={(event) => setAddress(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                event.preventDefault()
                submitMember()
              }
            }}
            placeholder='0x… address'
            className='h-8 flex-1 font-mono text-xs'
          />
          <Input
            value={chainId}
            onChange={(event) => setChainId(event.target.value)}
            inputMode='numeric'
            className='h-8 w-14 text-xs'
            title='Chain ID'
          />
          <Button size='icon' variant='ghost' className='size-8' onClick={submitMember} disabled={addMember.isPending}>
            {addMember.isPending ? <Loader2 className='animate-spin' /> : <Plus />}
          </Button>
        </div>
      ) : null}
    </div>
  )
}
