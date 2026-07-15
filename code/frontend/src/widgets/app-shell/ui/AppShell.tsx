import { LogOut, Radar } from 'lucide-react'
import { Link, NavLink, Outlet } from 'react-router-dom'

import { useAuth } from '@/entities/session'
import { initials } from '@/shared/lib/format'
import { cn } from '@/shared/lib/cn'
import { Avatar } from '@/shared/ui/avatar'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/shared/ui/dropdown-menu'

export function AppShell() {
  const { session, logout } = useAuth()
  const email = session?.email ?? 'account'

  return (
    <div className='flex h-screen w-full flex-col overflow-hidden bg-background text-foreground'>
      <header className='z-20 flex h-14 shrink-0 items-center justify-between border-b border-border/70 bg-background/80 px-4 backdrop-blur-md'>
        <div className='flex items-center gap-6'>
          <Link to='/' className='flex items-center gap-2'>
            <span className='flex size-8 items-center justify-center rounded-lg bg-primary/15 text-primary'>
              <Radar className='size-4' />
            </span>
            <span className='text-sm font-semibold tracking-tight'>
              Ledgerscope<span className='text-muted-foreground'> · console</span>
            </span>
          </Link>
          <nav className='hidden items-center gap-1 sm:flex'>
            <NavLink
              to='/'
              end
              className={({ isActive }) =>
                cn(
                  'rounded-md px-3 py-1.5 text-sm font-medium transition-colors',
                  isActive ? 'bg-accent/15 text-accent' : 'text-muted-foreground hover:text-foreground',
                )
              }
            >
              Cases
            </NavLink>
          </nav>
        </div>

        <DropdownMenu>
          <DropdownMenuTrigger className='flex items-center gap-2 rounded-full outline-none focus-visible:ring-2 focus-visible:ring-ring'>
            <Avatar>{initials(email)}</Avatar>
            <span className='hidden max-w-40 truncate text-sm text-muted-foreground md:inline'>{email}</span>
          </DropdownMenuTrigger>
          <DropdownMenuContent align='end' className='w-56'>
            <DropdownMenuLabel className='truncate normal-case text-foreground'>{email}</DropdownMenuLabel>
            <DropdownMenuSeparator />
            <DropdownMenuItem variant='destructive' onSelect={() => logout()}>
              <LogOut />
              Sign out
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </header>

      <div className='min-h-0 flex-1 overflow-hidden'>
        <Outlet />
      </div>
    </div>
  )
}
