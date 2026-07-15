export type CaseStatus = 'Open' | 'InProgress' | 'Closed' | 'Reopened'
export type CasePriority = 'Low' | 'Medium' | 'High' | 'Critical'

export const CASE_PRIORITIES: CasePriority[] = ['Low', 'Medium', 'High', 'Critical']
export const CASE_STATUSES: CaseStatus[] = ['Open', 'InProgress', 'Closed', 'Reopened']

export type CaseSummary = {
  id: string
  title: string
  status: CaseStatus
  priority: CasePriority
  organizationId: string
  createdAt: string
}

export type CaseAddress = {
  address: string
  chainId: number
  addedBy: string
  addedAt: string
  note: string | null
}

export type CaseAssignment = {
  userId: string
  roleName: string
  assignedAt: string
}

export type CaseNote = {
  id: string
  authorId: string
  text: string
  createdAt: string
}

export type CaseDetail = {
  id: string
  title: string
  description: string
  status: CaseStatus
  priority: CasePriority
  organizationId: string
  createdBy: string
  createdAt: string
  closedAt: string | null
  addresses: CaseAddress[]
  assignments: CaseAssignment[]
  notes: CaseNote[]
}

export const STATUS_LABEL: Record<CaseStatus, string> = {
  Open: 'Open',
  InProgress: 'In progress',
  Closed: 'Closed',
  Reopened: 'Reopened',
}

/** Tailwind classes for a status pill. */
export function statusClasses(status: CaseStatus): string {
  switch (status) {
    case 'Open':
      return 'bg-accent/15 text-accent border-accent/30'
    case 'InProgress':
      return 'bg-primary/15 text-primary border-primary/30'
    case 'Reopened':
      return 'bg-warning/15 text-warning border-warning/30'
    case 'Closed':
      return 'bg-muted text-muted-foreground border-border'
  }
}

/** Tailwind classes for a priority pill. */
export function priorityClasses(priority: CasePriority): string {
  switch (priority) {
    case 'Low':
      return 'bg-muted text-muted-foreground border-border'
    case 'Medium':
      return 'bg-accent/15 text-accent border-accent/30'
    case 'High':
      return 'bg-warning/15 text-warning border-warning/30'
    case 'Critical':
      return 'bg-destructive/15 text-destructive border-destructive/30'
  }
}
