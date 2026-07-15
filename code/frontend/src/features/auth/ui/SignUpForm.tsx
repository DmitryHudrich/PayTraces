import { Loader2 } from 'lucide-react'
import { useState, type FormEvent } from 'react'

import { useAuth } from '@/entities/session'
import { getErrorMessage } from '@/shared/api'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'

export function SignUpForm({ onSuccess }: { onSuccess?: () => void }) {
  const { register } = useAuth()
  const [displayName, setDisplayName] = useState('')
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [pending, setPending] = useState(false)

  const submit = async (event: FormEvent) => {
    event.preventDefault()
    setError(null)
    setPending(true)
    try {
      await register({ email: email.trim(), password, displayName: displayName.trim() })
      onSuccess?.()
    } catch (registerError) {
      setError(getErrorMessage(registerError, 'Could not create the account.'))
    } finally {
      setPending(false)
    }
  }

  return (
    <form onSubmit={submit} className='space-y-4'>
      <div className='space-y-2'>
        <Label htmlFor='signup-name'>Display name</Label>
        <Input
          id='signup-name'
          autoComplete='name'
          value={displayName}
          onChange={(event) => setDisplayName(event.target.value)}
          placeholder='Alex Investigator'
        />
      </div>
      <div className='space-y-2'>
        <Label htmlFor='signup-email'>Email</Label>
        <Input
          id='signup-email'
          type='email'
          autoComplete='email'
          required
          value={email}
          onChange={(event) => setEmail(event.target.value)}
          placeholder='you@agency.gov'
        />
      </div>
      <div className='space-y-2'>
        <Label htmlFor='signup-password'>Password</Label>
        <Input
          id='signup-password'
          type='password'
          autoComplete='new-password'
          required
          minLength={8}
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          placeholder='At least 8 characters'
        />
      </div>
      {error ? <p className='text-sm text-destructive'>{error}</p> : null}
      <Button type='submit' className='w-full' disabled={pending}>
        {pending ? <Loader2 className='animate-spin' /> : null}
        Create account
      </Button>
    </form>
  )
}
