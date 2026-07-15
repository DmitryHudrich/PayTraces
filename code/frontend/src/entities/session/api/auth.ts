import { apiRequest } from '@/shared/api'
import type { AuthTokenResponse, RegisterResponse } from '@/entities/session/model/types'

export function login(email: string, password: string): Promise<AuthTokenResponse> {
  return apiRequest<AuthTokenResponse>('/auth/login', {
    method: 'POST',
    body: JSON.stringify({ email, password }),
  })
}

export function register(input: {
  email: string
  password: string
  displayName?: string
}): Promise<RegisterResponse> {
  return apiRequest<RegisterResponse>('/auth/register', {
    method: 'POST',
    body: JSON.stringify({
      email: input.email,
      password: input.password,
      displayName: input.displayName?.trim() ? input.displayName.trim() : null,
    }),
  })
}
