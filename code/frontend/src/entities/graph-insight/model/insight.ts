export type IngestAccepted = { jobId: string }

export type JobStatus = {
  id: string
  status: string
  error: string | null
  createdAt: string
  updatedAt: string
}

export type RiskSignal = {
  kind: string
  severity: number
  description: string
  tagId: string | null
}

export type AddressScore = {
  address: string
  chainId: number
  score: number
  isHighRisk: boolean
  signals: RiskSignal[]
  generatedAt: string
}

export type HeuristicEvidence = {
  heuristic: string
  confidence: string
  addresses: string[]
  notes: string | null
}

export type Heuristics = {
  address: string
  fanOut: HeuristicEvidence | null
  fanIn: HeuristicEvidence | null
  smurfingCycle: HeuristicEvidence | null
  temporalBurst: HeuristicEvidence | null
  fixedAmountClustering: HeuristicEvidence | null
  dwellTimePassThrough: HeuristicEvidence | null
  peelingChain: HeuristicEvidence | null
  depositAddressReuse: HeuristicEvidence | null
}

export const HEURISTIC_LABELS: Record<keyof Omit<Heuristics, 'address'>, string> = {
  fanOut: 'Fan-out',
  fanIn: 'Fan-in',
  smurfingCycle: 'Smurfing cycle',
  temporalBurst: 'Temporal burst',
  fixedAmountClustering: 'Fixed-amount clustering',
  dwellTimePassThrough: 'Pass-through',
  peelingChain: 'Peeling chain',
  depositAddressReuse: 'Deposit reuse',
}

/** Returns only the heuristics that fired, with a display label. */
export function firedHeuristics(heuristics: Heuristics): { label: string; evidence: HeuristicEvidence }[] {
  return (Object.keys(HEURISTIC_LABELS) as (keyof typeof HEURISTIC_LABELS)[])
    .map((key) => ({ label: HEURISTIC_LABELS[key], evidence: heuristics[key] }))
    .filter((item): item is { label: string; evidence: HeuristicEvidence } => item.evidence !== null)
}

export type ClusterResult = {
  address: string
  components: string[][]
}

export type EntityTag = {
  tagId: string
  category: string
  labelName: string | null
  source: string
  confidence: number
  riskScore: number
  sanctionList: string | null
  active: boolean
  supersededBy: string | null
  createdAt: string
  expiresAt: string | null
  evidenceUrl: string | null
}

export type EntityAddress = {
  address: string
  chainId: number
  attachedAt: string
}

export type AddressEntity = {
  entityId: string
  addresses: EntityAddress[]
  tags: EntityTag[]
  aggregateRiskScore: number
}

export type IngestParams = {
  address: string
  chainId: number
  fromBlock?: number | null
  toBlock?: number | null
  maxDepth?: number | null
  maxNodes?: number | null
}
