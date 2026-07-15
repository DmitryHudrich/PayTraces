export type JwtClaims = {
  sub?: string
  exp?: number
  [key: string]: unknown
}

/** Decodes a JWT payload without verifying the signature (display only). */
export function decodeJwt(token: string): JwtClaims | null {
  const payload = token.split('.')[1]
  if (!payload) {
    return null
  }

  try {
    const normalized = payload.replace(/-/g, '+').replace(/_/g, '/')
    const padded = normalized.padEnd(normalized.length + ((4 - (normalized.length % 4)) % 4), '=')
    const json = atob(padded)
    return JSON.parse(json) as JwtClaims
  } catch {
    return null
  }
}

export function isJwtExpired(token: string, skewSeconds = 30): boolean {
  const claims = decodeJwt(token)
  if (!claims?.exp) {
    return false
  }
  return claims.exp * 1000 <= Date.now() + skewSeconds * 1000
}
