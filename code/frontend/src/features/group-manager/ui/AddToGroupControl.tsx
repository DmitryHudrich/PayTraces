import { Boxes } from 'lucide-react'
import { toast } from 'sonner'

import { useAddGroupMemberMutation, useGroupsQuery } from '@/entities/group'
import { getErrorMessage } from '@/shared/api'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/shared/ui/select'

type AddToGroupControlProps = {
  caseId: string
  address: string
  chainId: number
}

export function AddToGroupControl({ caseId, address, chainId }: AddToGroupControlProps) {
  const groupsQuery = useGroupsQuery(caseId)
  const addMember = useAddGroupMemberMutation(caseId)
  const groups = groupsQuery.data ?? []

  if (groups.length === 0) {
    return null
  }

  return (
    <Select
      value=''
      onValueChange={(groupId) =>
        addMember.mutate(
          { groupId, address, chainId },
          {
            onSuccess: () => toast.success('Added to group'),
            onError: (error) => toast.error(getErrorMessage(error, 'Failed to add to group.')),
          },
        )
      }
    >
      <SelectTrigger size='sm' className='h-8'>
        <Boxes className='size-3.5 text-muted-foreground' />
        <SelectValue placeholder='Add to group…' />
      </SelectTrigger>
      <SelectContent>
        {groups.map((group) => (
          <SelectItem key={group.id} value={group.id}>
            {group.name}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  )
}
