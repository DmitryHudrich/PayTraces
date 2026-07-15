import { Loader2, Plus } from 'lucide-react'
import { useState, type FormEvent } from 'react'
import { useNavigate } from 'react-router-dom'
import { toast } from 'sonner'

import { CASE_PRIORITIES, useCreateCaseMutation, type CasePriority } from '@/entities/case'
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
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/shared/ui/select'
import { Textarea } from '@/shared/ui/textarea'

export function CreateCaseDialog() {
  const navigate = useNavigate()
  const createCase = useCreateCaseMutation()
  const [open, setOpen] = useState(false)
  const [title, setTitle] = useState('')
  const [description, setDescription] = useState('')
  const [priority, setPriority] = useState<CasePriority>('Medium')

  const reset = () => {
    setTitle('')
    setDescription('')
    setPriority('Medium')
  }

  const submit = async (event: FormEvent) => {
    event.preventDefault()
    try {
      const { id } = await createCase.mutateAsync({ title: title.trim(), description, priority })
      toast.success('Case created')
      setOpen(false)
      reset()
      navigate(`/cases/${id}`)
    } catch (error) {
      toast.error(getErrorMessage(error, 'Could not create the case.'))
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (!next) {
          reset()
        }
      }}
    >
      <DialogTrigger asChild>
        <Button>
          <Plus />
          New case
        </Button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={submit}>
          <DialogHeader>
            <DialogTitle>Create a case</DialogTitle>
            <DialogDescription>Open an investigation to start tracing addresses.</DialogDescription>
          </DialogHeader>

          <div className='space-y-4 py-4'>
            <div className='space-y-2'>
              <Label htmlFor='case-title'>Title</Label>
              <Input
                id='case-title'
                required
                value={title}
                onChange={(event) => setTitle(event.target.value)}
                placeholder='Operation Nightshade'
              />
            </div>
            <div className='space-y-2'>
              <Label htmlFor='case-description'>Description</Label>
              <Textarea
                id='case-description'
                value={description}
                onChange={(event) => setDescription(event.target.value)}
                placeholder='Optional context for the investigation…'
              />
            </div>
            <div className='space-y-2'>
              <Label>Priority</Label>
              <Select value={priority} onValueChange={(value) => setPriority(value as CasePriority)}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {CASE_PRIORITIES.map((value) => (
                    <SelectItem key={value} value={value}>
                      {value}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          <DialogFooter>
            <Button type='submit' disabled={createCase.isPending || title.trim().length === 0}>
              {createCase.isPending ? <Loader2 className='animate-spin' /> : null}
              Create case
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
