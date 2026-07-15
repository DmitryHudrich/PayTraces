import type { LucideIcon } from 'lucide-react'
import type { ReactNode } from 'react'

import { cn } from '@/shared/lib/cn'

type EmptyStateProps = {
  icon?: LucideIcon
  title: string
  description?: string
  action?: ReactNode
  className?: string
}

export function EmptyState({ icon: Icon, title, description, action, className }: EmptyStateProps) {
  return (
    <div
      className={cn(
        'flex flex-col items-center justify-center gap-3 rounded-xl border border-dashed border-border/70 bg-card/30 px-6 py-12 text-center',
        className,
      )}
    >
      {Icon ? (
        <div className='flex size-11 items-center justify-center rounded-full bg-primary/10 text-primary'>
          <Icon className='size-5' />
        </div>
      ) : null}
      <div className='space-y-1'>
        <p className='text-sm font-medium'>{title}</p>
        {description ? <p className='mx-auto max-w-sm text-xs text-muted-foreground'>{description}</p> : null}
      </div>
      {action}
    </div>
  )
}
