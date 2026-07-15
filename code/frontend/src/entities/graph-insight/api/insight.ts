import { apiRequest } from '@/shared/api'
import type {
  AddressEntity,
  AddressScore,
  ClusterResult,
  Heuristics,
  IngestAccepted,
  IngestParams,
  JobStatus,
} from '@/entities/graph-insight/model/insight'

export function startIngest(caseId: string, params: IngestParams): Promise<IngestAccepted> {
  return apiRequest<IngestAccepted>(`/cases/${caseId}/ingest`, {
    method: 'POST',
    body: JSON.stringify({
      address: params.address,
      chainId: params.chainId,
      fromBlock: params.fromBlock ?? null,
      toBlock: params.toBlock ?? null,
      maxDepth: params.maxDepth ?? null,
      maxNodes: params.maxNodes ?? null,
    }),
  })
}

export function getJobStatus(caseId: string, jobId: string): Promise<JobStatus> {
  return apiRequest<JobStatus>(`/cases/${caseId}/jobs/${encodeURIComponent(jobId)}`)
}

function insightPath(caseId: string, chainId: number, address: string, kind: string): string {
  return `/cases/${caseId}/addresses/${chainId}/${encodeURIComponent(address)}/${kind}`
}

export function getScore(caseId: string, chainId: number, address: string): Promise<AddressScore> {
  return apiRequest<AddressScore>(insightPath(caseId, chainId, address, 'score'))
}

export function getHeuristics(caseId: string, chainId: number, address: string): Promise<Heuristics> {
  return apiRequest<Heuristics>(insightPath(caseId, chainId, address, 'heuristics'))
}

export function getCluster(caseId: string, chainId: number, address: string): Promise<ClusterResult> {
  return apiRequest<ClusterResult>(insightPath(caseId, chainId, address, 'cluster'))
}

export function getEntity(caseId: string, chainId: number, address: string): Promise<AddressEntity | null> {
  return apiRequest<AddressEntity | null>(insightPath(caseId, chainId, address, 'entity'))
}
