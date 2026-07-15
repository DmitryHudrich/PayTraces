import { getJobStatus, type JobStatus } from '@/entities/graph-insight'

const DONE = new Set(['done', 'succeeded', 'completed'])
const FAILED = new Set(['failed', 'error'])

function delay(ms: number) {
  return new Promise((resolve) => window.setTimeout(resolve, ms))
}

/**
 * Polls an ingest job until it reaches a terminal state. Resolves on success,
 * throws on failure/timeout.
 */
export async function waitForIngestJob(
  caseId: string,
  jobId: string,
  options: { onProgress?: (status: JobStatus) => void; intervalMs?: number; maxAttempts?: number } = {},
): Promise<JobStatus> {
  const { onProgress, intervalMs = 1200, maxAttempts = 600 } = options
  for (let attempt = 0; attempt < maxAttempts; attempt += 1) {
    const status = await getJobStatus(caseId, jobId)
    onProgress?.(status)
    const value = status.status.toLowerCase()
    if (DONE.has(value)) {
      return status
    }
    if (FAILED.has(value)) {
      throw new Error(status.error ?? 'Ingest job failed.')
    }
    await delay(intervalMs)
  }
  throw new Error('Ingest job timed out.')
}
