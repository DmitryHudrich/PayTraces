import type { CaseGraphNode } from '@/entities/case-graph/model/graph'

export type RiskBand = 'critical' | 'high' | 'medium' | 'low' | 'unknown'

export function riskBand(node: Pick<CaseGraphNode, 'riskScore' | 'isHighRisk'>): RiskBand {
  const score = node.riskScore
  if (score == null) {
    return node.isHighRisk ? 'high' : 'unknown'
  }
  if (score >= 75 || node.isHighRisk) {
    return score >= 90 || node.isHighRisk ? 'critical' : 'high'
  }
  if (score >= 50) {
    return 'high'
  }
  if (score >= 25) {
    return 'medium'
  }
  return 'low'
}

export const RISK_BAND_LABEL: Record<RiskBand, string> = {
  critical: 'Critical',
  high: 'High',
  medium: 'Medium',
  low: 'Low',
  unknown: 'Unrated',
}

/** Tailwind classes for a risk pill. */
export function riskBandClasses(band: RiskBand): string {
  switch (band) {
    case 'critical':
      return 'bg-destructive/15 text-destructive border-destructive/30'
    case 'high':
      return 'bg-warning/15 text-warning border-warning/30'
    case 'medium':
      return 'bg-accent/15 text-accent border-accent/30'
    case 'low':
      return 'bg-success/15 text-success border-success/30'
    case 'unknown':
      return 'bg-muted text-muted-foreground border-border'
  }
}

function isService(node: CaseGraphNode): boolean {
  if (node.serviceName && node.serviceName.trim().length > 0) {
    return true
  }
  const kind = node.kind?.toLowerCase() ?? ''
  return kind === 'exchange' || kind === 'service' || kind === 'cex' || kind === 'dex'
}

/** Group id consumed by the sigma adapter to pick a node color. */
export function nodeGroup(node: CaseGraphNode): string {
  const band = riskBand(node)
  if (band === 'critical' || band === 'high' || band === 'medium') {
    return band
  }
  if (isService(node)) {
    return 'service'
  }
  return 'wallet'
}
