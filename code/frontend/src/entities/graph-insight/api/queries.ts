import { useQuery } from '@tanstack/react-query'

import { getCluster, getEntity, getHeuristics, getScore } from '@/entities/graph-insight/api/insight'

export const insightKeys = {
  score: (caseId: string, chainId: number, address: string) =>
    ['insight', caseId, 'score', chainId, address.toLowerCase()] as const,
  heuristics: (caseId: string, chainId: number, address: string) =>
    ['insight', caseId, 'heuristics', chainId, address.toLowerCase()] as const,
  cluster: (caseId: string, chainId: number, address: string) =>
    ['insight', caseId, 'cluster', chainId, address.toLowerCase()] as const,
  entity: (caseId: string, chainId: number, address: string) =>
    ['insight', caseId, 'entity', chainId, address.toLowerCase()] as const,
}

const shared = { staleTime: 60_000, retry: 0 as const }

export function useScoreQuery(caseId: string | undefined, chainId: number, address: string | null) {
  return useQuery({
    queryKey: insightKeys.score(caseId ?? '', chainId, address ?? ''),
    queryFn: () => getScore(caseId as string, chainId, address as string),
    enabled: Boolean(caseId && address),
    ...shared,
  })
}

export function useHeuristicsQuery(caseId: string | undefined, chainId: number, address: string | null) {
  return useQuery({
    queryKey: insightKeys.heuristics(caseId ?? '', chainId, address ?? ''),
    queryFn: () => getHeuristics(caseId as string, chainId, address as string),
    enabled: Boolean(caseId && address),
    ...shared,
  })
}

export function useEntityQuery(caseId: string | undefined, chainId: number, address: string | null) {
  return useQuery({
    queryKey: insightKeys.entity(caseId ?? '', chainId, address ?? ''),
    queryFn: () => getEntity(caseId as string, chainId, address as string),
    enabled: Boolean(caseId && address),
    ...shared,
  })
}

export function useClusterQuery(
  caseId: string | undefined,
  chainId: number,
  address: string | null,
  enabled: boolean,
) {
  return useQuery({
    queryKey: insightKeys.cluster(caseId ?? '', chainId, address ?? ''),
    queryFn: () => getCluster(caseId as string, chainId, address as string),
    enabled: Boolean(caseId && address && enabled),
    ...shared,
  })
}
