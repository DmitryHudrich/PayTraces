export type CustomLabel = {
  id: string
  caseId: string | null
  text: string
  color: string | null
  createdBy: string
  createdAt: string
}

export type AppliedLabel = {
  labelId: string
  text: string
  color: string | null
  appliedAt: string
}

export const LABEL_COLORS = [
  '#a78bfa',
  '#38bdf8',
  '#34d399',
  '#fbbf24',
  '#f87171',
  '#f472b6',
  '#94a3b8',
] as const

export function labelColor(color: string | null | undefined): string {
  return color && color.trim().length > 0 ? color : '#94a3b8'
}
