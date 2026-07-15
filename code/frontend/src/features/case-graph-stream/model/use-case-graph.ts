import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import { getErrorMessage } from '@/shared/api'
import {
  HubConnectionState,
  createGraphConnection,
  type GraphStreamItem,
  type HubConnection,
} from '@/shared/realtime/graph-hub'
import {
  edgeKey,
  type CaseGraphEdge,
  type CaseGraphNode,
  type CaseGraphPage,
  type NodePosition,
} from '@/entities/case-graph'

export type GraphStreamStatus = 'idle' | 'connecting' | 'streaming' | 'ready' | 'error'

export type StreamParams = {
  address: string
  chainId: number
  maxDepth: number
  viewId?: string | null
}

export type GraphProgress = {
  pagesLoaded: number
  totalPages: number
  totalNodes: number
  totalEdges: number
}

const ZERO_PROGRESS: GraphProgress = { pagesLoaded: 0, totalPages: 0, totalNodes: 0, totalEdges: 0 }

function mergeNode(prev: CaseGraphNode | undefined, next: CaseGraphNode): CaseGraphNode {
  if (!prev) {
    return next
  }
  return {
    address: next.address,
    kind: next.kind ?? prev.kind,
    serviceName: next.serviceName ?? prev.serviceName,
    riskScore: next.riskScore ?? prev.riskScore,
    isHighRisk: next.isHighRisk ?? prev.isHighRisk,
    inDegree: next.inDegree ?? prev.inDegree,
    outDegree: next.outDegree ?? prev.outDegree,
    txCount: next.txCount ?? prev.txCount,
    isViewBoundary: next.isViewBoundary ?? prev.isViewBoundary,
    isIngestBoundary: next.isIngestBoundary ?? prev.isIngestBoundary,
  }
}

/**
 * Owns the SignalR graph connection for one case and accumulates the streamed
 * BFS pages into node/edge collections. Streaming a new root resets the graph;
 * expanding a node merges more neighbours into the current graph.
 */
export function useCaseGraph(caseId: string | undefined) {
  const connectionRef = useRef<HubConnection | null>(null)
  const nodesRef = useRef(new Map<string, CaseGraphNode>())
  const edgesRef = useRef(new Map<string, CaseGraphEdge>())
  const streamSubRef = useRef<{ dispose(): void } | null>(null)

  const [revision, setRevision] = useState(0)
  const [status, setStatus] = useState<GraphStreamStatus>('idle')
  const [error, setError] = useState<string | null>(null)
  const [progress, setProgress] = useState<GraphProgress>(ZERO_PROGRESS)
  const [positions, setPositions] = useState<NodePosition[] | null>(null)
  const [rootAddress, setRootAddress] = useState<string | null>(null)

  const bump = useCallback(() => setRevision((value) => value + 1), [])

  const ensureConnection = useCallback(async () => {
    if (!connectionRef.current) {
      connectionRef.current = createGraphConnection()
    }
    const connection = connectionRef.current
    if (connection.state === HubConnectionState.Disconnected) {
      await connection.start()
    }
    return connection
  }, [])

  const ingestPage = useCallback((page: CaseGraphPage) => {
    for (const node of page.nodes) {
      const key = node.address.toLowerCase()
      nodesRef.current.set(key, mergeNode(nodesRef.current.get(key), node))
    }
    for (const edge of page.edges) {
      edgesRef.current.set(edgeKey(edge), edge)
    }
  }, [])

  const clearGraph = useCallback(() => {
    streamSubRef.current?.dispose()
    streamSubRef.current = null
    nodesRef.current = new Map()
    edgesRef.current = new Map()
    setPositions(null)
    setProgress(ZERO_PROGRESS)
    setError(null)
    setRootAddress(null)
    setStatus('idle')
    bump()
  }, [bump])

  const stream = useCallback(
    async (params: StreamParams) => {
      if (!caseId) {
        return
      }
      streamSubRef.current?.dispose()
      streamSubRef.current = null
      nodesRef.current = new Map()
      edgesRef.current = new Map()
      setPositions(null)
      setError(null)
      setProgress(ZERO_PROGRESS)
      setRootAddress(params.address.toLowerCase())
      bump()
      setStatus('connecting')

      try {
        const connection = await ensureConnection()
        setStatus('streaming')
        const subject = connection.stream(
          'StreamCaseGraph',
          caseId,
          params.address,
          params.chainId,
          params.maxDepth,
          params.viewId ?? null,
        )
        streamSubRef.current = subject.subscribe({
          next: (raw) => {
            const item = raw as GraphStreamItem
            if (item.positions) {
              setPositions(item.positions)
            }
            ingestPage(item.page)
            setProgress({
              pagesLoaded: item.page.page + 1,
              totalPages: item.page.totalPages,
              totalNodes: nodesRef.current.size,
              totalEdges: edgesRef.current.size,
            })
            bump()
          },
          complete: () => setStatus('ready'),
          error: (streamError: unknown) => {
            setError(getErrorMessage(streamError, 'Graph stream failed.'))
            setStatus('error')
          },
        })
      } catch (startError) {
        setError(getErrorMessage(startError, 'Could not connect to the graph hub.'))
        setStatus('error')
      }
    },
    [bump, caseId, ensureConnection, ingestPage],
  )

  const expand = useCallback(
    async (address: string, chainId: number, maxDepth: number) => {
      if (!caseId) {
        return
      }
      const connection = await ensureConnection()
      const page = await connection.invoke<CaseGraphPage>('ExpandNode', caseId, address, chainId, maxDepth)
      ingestPage(page)
      setProgress((prev) => ({
        ...prev,
        totalNodes: nodesRef.current.size,
        totalEdges: edgesRef.current.size,
      }))
      bump()
      return page
    },
    [bump, caseId, ensureConnection, ingestPage],
  )

  useEffect(() => {
    return () => {
      streamSubRef.current?.dispose()
      void connectionRef.current?.stop()
      connectionRef.current = null
    }
  }, [])

  const nodes = useMemo(() => Array.from(nodesRef.current.values()), [revision])
  const edges = useMemo(() => Array.from(edgesRef.current.values()), [revision])
  const nodesByAddress = useMemo(() => new Map(nodesRef.current), [revision])

  return {
    status,
    error,
    progress,
    positions,
    rootAddress,
    nodes,
    edges,
    nodesByAddress,
    isBusy: status === 'connecting' || status === 'streaming',
    stream,
    expand,
    clearGraph,
  }
}

export type UseCaseGraphResult = ReturnType<typeof useCaseGraph>
