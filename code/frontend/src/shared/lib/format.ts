/** Truncates a long hash/address to `0x1234…cdef`. */
export function shortAddress(address: string, lead = 6, tail = 4): string {
  const value = address.trim()
  if (value.length <= lead + tail + 1) {
    return value
  }
  return `${value.slice(0, lead)}…${value.slice(-tail)}`
}

export function normalizeAddress(address: string): string {
  return address.trim().toLowerCase()
}

const dateTimeFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: 'medium',
  timeStyle: 'short',
})

const dateFormatter = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium' })

export function formatDateTime(value: string | number | Date | null | undefined): string {
  if (value == null) {
    return '—'
  }
  const date = value instanceof Date ? value : new Date(value)
  return Number.isNaN(date.getTime()) ? '—' : dateTimeFormatter.format(date)
}

export function formatDate(value: string | number | Date | null | undefined): string {
  if (value == null) {
    return '—'
  }
  const date = value instanceof Date ? value : new Date(value)
  return Number.isNaN(date.getTime()) ? '—' : dateFormatter.format(date)
}

const RELATIVE_UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ['year', 1000 * 60 * 60 * 24 * 365],
  ['month', 1000 * 60 * 60 * 24 * 30],
  ['day', 1000 * 60 * 60 * 24],
  ['hour', 1000 * 60 * 60],
  ['minute', 1000 * 60],
  ['second', 1000],
]

const relativeFormatter = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' })

export function formatRelative(value: string | number | Date | null | undefined): string {
  if (value == null) {
    return '—'
  }
  const date = value instanceof Date ? value : new Date(value)
  if (Number.isNaN(date.getTime())) {
    return '—'
  }
  const diff = date.getTime() - Date.now()
  for (const [unit, ms] of RELATIVE_UNITS) {
    if (Math.abs(diff) >= ms || unit === 'second') {
      return relativeFormatter.format(Math.round(diff / ms), unit)
    }
  }
  return relativeFormatter.format(0, 'second')
}

/** Initials for an email/name, for avatars. */
export function initials(value: string): string {
  const base = value.split('@')[0] ?? value
  const parts = base.split(/[.\-_\s]+/).filter(Boolean)
  const chars = parts.slice(0, 2).map((part) => part[0])
  return (chars.join('') || base.slice(0, 2)).toUpperCase()
}
