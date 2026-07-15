import { createContext, use, useCallback, useEffect, useMemo, useState, type PropsWithChildren } from 'react'
import { useQueryClient } from '@tanstack/react-query'

import { login as apiLogin, register as apiRegister } from '@/entities/session/api/auth'
import {
  clearSession,
  getSession,
  setSession,
  subscribeSession,
  type StoredSession,
} from '@/shared/auth/token-storage'

type AuthContextValue = {
  session: StoredSession | null
  isAuthenticated: boolean
  login: (email: string, password: string) => Promise<void>
  register: (input: { email: string; password: string; displayName?: string }) => Promise<void>
  logout: () => void
}

const AuthContext = createContext<AuthContextValue | null>(null)

export function AuthProvider({ children }: PropsWithChildren) {
  const [session, setSessionState] = useState<StoredSession | null>(() => getSession())
  const queryClient = useQueryClient()

  useEffect(() => subscribeSession(setSessionState), [])

  const login = useCallback(async (email: string, password: string) => {
    const result = await apiLogin(email, password)
    setSession({ token: result.access_token, email, expiresAt: result.expires_at })
  }, [])

  const register = useCallback(
    async (input: { email: string; password: string; displayName?: string }) => {
      await apiRegister(input)
      const result = await apiLogin(input.email, input.password)
      setSession({ token: result.access_token, email: input.email, expiresAt: result.expires_at })
    },
    [],
  )

  const logout = useCallback(() => {
    clearSession()
    queryClient.clear()
  }, [queryClient])

  const value = useMemo<AuthContextValue>(
    () => ({ session, isAuthenticated: session !== null, login, register, logout }),
    [session, login, register, logout],
  )

  return <AuthContext value={value}>{children}</AuthContext>
}

export function useAuth(): AuthContextValue {
  const context = use(AuthContext)
  if (!context) {
    throw new Error('useAuth must be used within an AuthProvider')
  }
  return context
}
