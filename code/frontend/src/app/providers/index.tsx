import type { PropsWithChildren } from 'react'

import { QueryProvider } from '@/app/providers/query-provider'

export const Providers = ({ children }: PropsWithChildren) => {
  return <QueryProvider>{children}</QueryProvider>
}
