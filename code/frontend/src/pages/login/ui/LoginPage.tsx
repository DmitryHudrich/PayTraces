import { Radar, ShieldCheck } from 'lucide-react'
import { useState } from 'react'
import { Navigate, useLocation, useNavigate } from 'react-router-dom'

import { SignInForm, SignUpForm } from '@/features/auth'
import { useAuth } from '@/entities/session'
import { cn } from '@/shared/lib/cn'

type Mode = 'signin' | 'signup'

type LocationState = { from?: { pathname?: string } }

export function LoginPage() {
  const { isAuthenticated } = useAuth()
  const navigate = useNavigate()
  const location = useLocation()
  const [mode, setMode] = useState<Mode>('signin')

  const target = (location.state as LocationState | null)?.from?.pathname ?? '/'

  if (isAuthenticated) {
    return <Navigate to={target} replace />
  }

  const goToApp = () => navigate(target, { replace: true })

  return (
    <div className='relative flex min-h-screen items-center justify-center overflow-hidden bg-background px-4'>
      <div className='pointer-events-none absolute -top-40 left-1/2 h-96 w-[46rem] -translate-x-1/2 rounded-full bg-primary/20 blur-[120px]' />
      <div className='pointer-events-none absolute bottom-0 right-10 h-72 w-72 rounded-full bg-accent/10 blur-[120px]' />

      <div className='relative grid w-full max-w-4xl overflow-hidden rounded-2xl border border-border/70 bg-card/70 shadow-2xl backdrop-blur-xl lg:grid-cols-2'>
        <div className='hidden flex-col justify-between gap-8 border-r border-border/70 bg-gradient-to-b from-primary/10 to-transparent p-8 lg:flex'>
          <div className='flex items-center gap-2'>
            <span className='flex size-9 items-center justify-center rounded-lg bg-primary/20 text-primary'>
              <Radar className='size-5' />
            </span>
            <span className='text-base font-semibold tracking-tight'>Ledgerscope</span>
          </div>
          <div className='space-y-4'>
            <h1 className='text-2xl font-semibold leading-tight tracking-tight'>
              Trace funds. Build the case.
            </h1>
            <p className='text-sm text-muted-foreground'>
              A live, permissioned workspace for blockchain investigations — stream transaction graphs,
              pin your canvas, label entities and organise addresses into groups.
            </p>
            <ul className='space-y-2 text-sm text-muted-foreground'>
              {['Progressive graph streaming', 'Case-scoped access control', 'Shared canvas views'].map((item) => (
                <li key={item} className='flex items-center gap-2'>
                  <ShieldCheck className='size-4 text-accent' />
                  {item}
                </li>
              ))}
            </ul>
          </div>
          <p className='text-xs text-muted-foreground'>Authorized personnel only.</p>
        </div>

        <div className='p-8'>
          <div className='mb-6 flex gap-1 rounded-lg border border-border/70 bg-background/40 p-1'>
            {(['signin', 'signup'] as Mode[]).map((value) => (
              <button
                key={value}
                type='button'
                onClick={() => setMode(value)}
                className={cn(
                  'flex-1 rounded-md px-3 py-1.5 text-sm font-medium transition-colors',
                  mode === value
                    ? 'bg-primary text-primary-foreground shadow-sm'
                    : 'text-muted-foreground hover:text-foreground',
                )}
              >
                {value === 'signin' ? 'Sign in' : 'Create account'}
              </button>
            ))}
          </div>

          <div className='mb-6 space-y-1'>
            <h2 className='text-lg font-semibold'>
              {mode === 'signin' ? 'Welcome back' : 'Create your account'}
            </h2>
            <p className='text-sm text-muted-foreground'>
              {mode === 'signin'
                ? 'Sign in to continue your investigations.'
                : 'Register to start tracing on-chain activity.'}
            </p>
          </div>

          {mode === 'signin' ? <SignInForm onSuccess={goToApp} /> : <SignUpForm onSuccess={goToApp} />}
        </div>
      </div>
    </div>
  )
}
