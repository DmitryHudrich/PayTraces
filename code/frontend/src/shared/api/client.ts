import { env } from '@/shared/config/env'
import { ApiError } from '@/shared/api/errors'
import { clearSession, getToken } from '@/shared/auth/token-storage'

type ErrorBody = {
  error?: string
}

async function readErrorMessage(response: Response) {
  try {
    const body = (await response.json()) as ErrorBody
    if (body.error) {
      return body.error
    }
  } catch {
    // ignore invalid JSON bodies
  }

  return `Request failed with status ${response.status}`
}

export async function apiRequest<T>(path: string, init?: RequestInit): Promise<T> {
  const token = getToken()
  const response = await fetch(`${env.apiBaseUrl}${path}`, {
    ...init,
    headers: {
      'Content-Type': 'application/json',
      'X-API-Version': env.apiVersion,
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...(init?.headers ?? {}),
    },
  })

  if (response.status === 401) {
    clearSession()
    throw new ApiError('Your session has expired. Please sign in again.', 401)
  }

  if (!response.ok) {
    const message = await readErrorMessage(response)
    throw new ApiError(message, response.status)
  }

  // 204 No Content (and empty 200 bodies) are common on the C# API's commands.
  if (response.status === 204) {
    return undefined as T
  }

  const text = await response.text()
  return (text ? JSON.parse(text) : undefined) as T
}
