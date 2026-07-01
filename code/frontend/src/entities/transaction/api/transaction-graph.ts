import { apiRequest } from '@/shared/api'
import type { TransactionGraphPage } from '@/entities/transaction/model/transaction'

export type IngestWalletPayload = {
  address: string
  from_block: number
  max_depth?: number
  max_nodes?: number
}

type IngestJobResponse = {
  job_id: string
}

export async function ingestWallet(payload: IngestWalletPayload) {
  return apiRequest<IngestJobResponse>('/jobs/ingest', {
    method: 'POST',
    body: JSON.stringify({
      address: payload.address,
      from_block: payload.from_block,
      max_depth: payload.max_depth ?? 3,
      max_nodes: payload.max_nodes ?? 500,
      chain_id: 1,
    }),
  })
}

export type FetchGraphPayload = {
  address: string
  from_block: number
  max_depth?: number
  max_nodes?: number
}

export async function fetchTransactionGraph(payload: FetchGraphPayload) {
  const params = new URLSearchParams({
    address: payload.address,
    chain_id: '1',
    from_block: String(payload.from_block),
    max_depth: String(payload.max_depth ?? 3),
    max_nodes: String(payload.max_nodes ?? 500),
    page: '0',
    page_size: '500',
  })

  return apiRequest<TransactionGraphPage>(`/graph?${params.toString()}`)
}
