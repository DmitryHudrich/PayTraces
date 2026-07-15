import { Loader2, UserPlus } from 'lucide-react'
import { useState, type FormEvent } from 'react'
import { toast } from 'sonner'

import { useAssignCaseMutation } from '@/entities/case'
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

const CASE_ROLES = ['Lead', 'Collaborator']

export function AssignMemberDialog({ caseId }: { caseId: string }) {
  const assign = useAssignCaseMutation(caseId)
  const [open, setOpen] = useState(false)
  const [userId, setUserId] = useState('')
  const [roleName, setRoleName] = useState('Collaborator')

  const submit = async (event: FormEvent) => {
    event.preventDefault()
    try {
      await assign.mutateAsync({ userId: userId.trim(), roleName })
      toast.success('Member assigned')
      setOpen(false)
      setUserId('')
    } catch (error) {
      toast.error(getErrorMessage(error, 'Could not assign the member.'))
    }
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button size='sm' variant='outline'>
          <UserPlus />
          Assign
        </Button>
      </DialogTrigger>
      <DialogContent>
        <form onSubmit={submit}>
          <DialogHeader>
            <DialogTitle>Assign a member</DialogTitle>
            <DialogDescription>Grant a teammate a role on this case.</DialogDescription>
          </DialogHeader>
          <div className='space-y-4 py-4'>
            <div className='space-y-2'>
              <Label htmlFor='assign-user'>User ID</Label>
              <Input
                id='assign-user'
                required
                value={userId}
                onChange={(event) => setUserId(event.target.value)}
                placeholder='00000000-0000-0000-0000-000000000000'
                className='font-mono text-xs'
              />
            </div>
            <div className='space-y-2'>
              <Label>Role</Label>
              <Select value={roleName} onValueChange={setRoleName}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {CASE_ROLES.map((role) => (
                    <SelectItem key={role} value={role}>
                      {role}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
          <DialogFooter>
            <Button type='submit' disabled={assign.isPending || userId.trim().length === 0}>
              {assign.isPending ? <Loader2 className='animate-spin' /> : null}
              Assign member
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
