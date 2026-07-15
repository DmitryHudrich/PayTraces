import { decodeJwt } from '@/shared/auth/jwt'

export type StoredSession = {
  token: string
  email: string
  userId: string | null
  expiresAt: string | null
}

const STORAGE_KEY = 'ledgerscope.session'

type Listener = (session: StoredSession | null) => void

const listeners = new Set<Listener>()

function read(): StoredSession | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return raw ? (JSON.parse(raw) as StoredSession) : null
  } catch {
    return null
  }
}

let current: StoredSession | null = read()

export function getSession(): StoredSession | null {
  return current
}

export function getToken(): string | null {
  return current?.token ?? null
}

export function setSession(input: { token: string; email: string; expiresAt: string | null }): StoredSession {
  const claims = decodeJwt(input.token)
  const session: StoredSession = {
    token: input.token,
    email: input.email,
    userId: typeof claims?.sub === 'string' ? claims.sub : null,
    expiresAt: input.expiresAt,
  }
  current = session
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(session))
  } catch {
    // ignore storage failures (private mode, quota)
  }
  listeners.forEach((listener) => listener(session))
  return session
}

export function clearSession(): void {
  if (current === null) {
    return
  }
  current = null
  try {
    localStorage.removeItem(STORAGE_KEY)
  } catch {
    // ignore
  }
  listeners.forEach((listener) => listener(null))
}

export function subscribeSession(listener: Listener): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}
