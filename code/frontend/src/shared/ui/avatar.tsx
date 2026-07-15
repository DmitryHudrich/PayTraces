import * as React from 'react'

import { cn } from '@/shared/lib/cn'

function Avatar({ className, ...props }: React.ComponentProps<'span'>) {
  return (
    <span
      data-slot='avatar'
      className={cn(
        'relative flex size-8 shrink-0 items-center justify-center overflow-hidden rounded-full bg-primary/15 text-xs font-semibold text-primary select-none',
        className,
      )}
      {...props}
    />
  )
}

export { Avatar }
