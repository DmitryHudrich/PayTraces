import { Loader2 } from 'lucide-react'
import { useState, type FormEvent } from 'react'

import { useAuth } from '@/entities/session'
import { getErrorMessage } from '@/shared/api'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'

export function SignInForm({ onSuccess }: { onSuccess?: () => void }) {
  const { login } = useAuth()
  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [pending, setPending] = useState(false)

  const submit = async (event: FormEvent) => {
    event.preventDefault()
    setError(null)
    setPending(true)
    try {
      await login(email.trim(), password)
      onSuccess?.()
    } catch (loginError) {
      setError(getErrorMessage(loginError, 'Could not sign in.'))
    } finally {
      setPending(false)
    }
  }

  return (
    <form onSubmit={submit} className='space-y-4'>
      <div className='space-y-2'>
        <Label htmlFor='signin-email'>Email</Label>
        <Input
          id='signin-email'
          type='email'
          autoComplete='email'
          required
          value={email}
          onChange={(event) => setEmail(event.target.value)}
          placeholder='you@agency.gov'
        />
      </div>
      <div className='space-y-2'>
        <Label htmlFor='signin-password'>Password</Label>
        <Input
          id='signin-password'
          type='password'
          autoComplete='current-password'
          required
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          placeholder='••••••••'
        />
      </div>
      {error ? <p className='text-sm text-destructive'>{error}</p> : null}
      <Button type='submit' className='w-full' disabled={pending}>
        {pending ? <Loader2 className='animate-spin' /> : null}
        Sign in
      </Button>
    </form>
  )
}
