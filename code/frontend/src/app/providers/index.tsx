import type { PropsWithChildren } from 'react'

import { QueryProvider } from '@/app/providers/query-provider'
import { AuthProvider } from '@/entities/session'
import { Toaster } from '@/shared/ui/sonner'
import { TooltipProvider } from '@/shared/ui/tooltip'

export const AppProviders = ({ children }: PropsWithChildren) => {
  return (
    <QueryProvider>
      <AuthProvider>
        <TooltipProvider delayDuration={200}>
          {children}
          <Toaster richColors position='bottom-right' theme='dark' />
        </TooltipProvider>
      </AuthProvider>
    </QueryProvider>
  )
}
