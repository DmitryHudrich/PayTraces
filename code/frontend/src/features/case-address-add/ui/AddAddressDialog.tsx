import { Loader2, Plus } from 'lucide-react'
import { useState, type FormEvent, type ReactNode } from 'react'
import { toast } from 'sonner'

import { useAddCaseAddressMutation } from '@/entities/case'
import { getErrorMessage } from '@/shared/api'
import { Button } from '@/shared/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Textarea } from '@/shared/ui/textarea'

type AddAddressDialogProps = {
  caseId: string
  trigger?: ReactNode
  defaultAddress?: string
  defaultChainId?: number
}

export function AddAddressDialog({ caseId, trigger, defaultAddress = '', defaultChainId = 1 }: AddAddressDialogProps) {
  const addAddress = useAddCaseAddressMutation(caseId)
  const [open, setOpen] = useState(false)
  const [address, setAddress] = useState(defaultAddress)
  const [chainId, setChainId] = useState(String(defaultChainId))
  const [note, setNote] = useState('')

  const submit = async (event: FormEvent) => {
    event.preventDefault()
    try {
      await addAddress.mutateAsync({ address: address.trim(), chainId: Number(chainId) || 1, note })
      toast.success('Address added to case')
      setOpen(false)
      setAddress('')
      setNote('')
    } catch (error) {
      toast.error(getErrorMessage(error, 'Could not add the address.'))
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (next) {
          setAddress(defaultAddress)
          setChainId(String(defaultChainId))
        }
      }}
    >
      <DialogTrigger asChild>
        {trigger ?? (
          <Button size='sm' variant='outline'>
            <Plus />
            Add address
          </Button>
        )}
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={submit}>
          <DialogHeader>
            <DialogTitle>Add an address</DialogTitle>
            <DialogDescription>Record an address on this case so it can be traced.</DialogDescription>
          </DialogHeader>
          <div className='space-y-4 py-4'>
            <div className='space-y-2'>
              <Label htmlFor='addr-value'>Address</Label>
              <Input
                id='addr-value'
                required
                value={address}
                onChange={(event) => setAddress(event.target.value)}
                placeholder='0x…'
                className='font-mono text-sm'
              />
            </div>
            <div className='space-y-2'>
              <Label htmlFor='addr-chain'>Chain ID</Label>
              <Input
                id='addr-chain'
                inputMode='numeric'
                value={chainId}
                onChange={(event) => setChainId(event.target.value)}
                placeholder='1'
              />
            </div>
            <div className='space-y-2'>
              <Label htmlFor='addr-note'>Note</Label>
              <Textarea
                id='addr-note'
                value={note}
                onChange={(event) => setNote(event.target.value)}
                placeholder='Why is this address relevant?'
              />
            </div>
          </div>
          <DialogFooter>
            <Button type='submit' disabled={addAddress.isPending || address.trim().length === 0}>
              {addAddress.isPending ? <Loader2 className='animate-spin' /> : null}
              Add address
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
