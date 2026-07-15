import { Loader2, Plus, X } from 'lucide-react'
import { useState } from 'react'
import { toast } from 'sonner'

import {
  labelColor,
  useAddressLabelsQuery,
  useApplyLabelMutation,
  useCaseLabelsQuery,
  useRemoveAddressLabelMutation,
} from '@/entities/case-label'
import { getErrorMessage } from '@/shared/api'
import { Badge } from '@/shared/ui/badge'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/shared/ui/select'

type AddressLabelsControlProps = {
  caseId: string
  address: string
  chainId: number
  canApply: boolean
}

export function AddressLabelsControl({ caseId, address, chainId, canApply }: AddressLabelsControlProps) {
  const catalog = useCaseLabelsQuery(caseId)
  const applied = useAddressLabelsQuery(caseId, chainId, address)
  const applyLabel = useApplyLabelMutation(caseId)
  const removeLabel = useRemoveAddressLabelMutation(caseId)
  const [picked, setPicked] = useState('')

  const appliedLabels = applied.data ?? []
  const appliedIds = new Set(appliedLabels.map((label) => label.labelId))
  const available = (catalog.data ?? []).filter((label) => !appliedIds.has(label.id))

  const apply = (labelId: string) => {
    applyLabel.mutate(
      { labelId, address, chainId },
      {
        onSuccess: () => {
          setPicked('')
          toast.success('Label applied')
        },
        onError: (error) => toast.error(getErrorMessage(error, 'Failed to apply label.')),
      },
    )
  }

  return (
    <div className='space-y-2'>
      <div className='flex flex-wrap gap-1.5'>
        {applied.isPending ? (
          <span className='flex items-center gap-1 text-xs text-muted-foreground'>
            <Loader2 className='size-3 animate-spin' /> Loading…
          </span>
        ) : appliedLabels.length === 0 ? (
          <span className='text-xs text-muted-foreground'>No labels applied.</span>
        ) : (
          appliedLabels.map((label) => (
            <Badge key={label.labelId} variant='outline' className='gap-1 pr-1'>
              <span className='size-2 rounded-full' style={{ backgroundColor: labelColor(label.color) }} />
              {label.text}
              {canApply ? (
                <button
                  type='button'
                  className='ml-0.5 rounded-full p-0.5 text-muted-foreground hover:text-destructive'
                  onClick={() =>
                    removeLabel.mutate(
                      { labelId: label.labelId, chainId, address },
                      { onError: (error) => toast.error(getErrorMessage(error, 'Failed to remove label.')) },
                    )
                  }
                >
                  <X className='size-3' />
                </button>
              ) : null}
            </Badge>
          ))
        )}
      </div>

      {canApply && available.length > 0 ? (
        <Select value={picked} onValueChange={apply}>
          <SelectTrigger size='sm' className='h-8'>
            <Plus className='size-3.5 text-muted-foreground' />
            <SelectValue placeholder='Apply a label…' />
          </SelectTrigger>
          <SelectContent>
            {available.map((label) => (
              <SelectItem key={label.id} value={label.id}>
                <span className='flex items-center gap-2'>
                  <span className='size-2 rounded-full' style={{ backgroundColor: labelColor(label.color) }} />
                  {label.text}
                </span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      ) : null}
    </div>
  )
}
