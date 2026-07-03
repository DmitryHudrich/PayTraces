import { lazy, Suspense, useMemo, useState } from 'react'

import {
  emptyTransactionGraphPage,
  filterGraphData,
  getTransactionNodeDetails,
  mockTransactionGraphPage,
  transactionGraphPageToGraphData,
  useFetchTransactionGraphMutation,
  useIngestJobStatusQuery,
  useIngestWalletMutation,
  type TransactionGraphPage as TransactionGraphPageData,
} from '@/entities/transaction'
import { TransactionGraphControls } from '@/features/transaction-graph-controls'
import { TransactionGraphFlowForm } from '@/features/transaction-graph-flow'
import { TransactionGraphSourceToggle, type GraphSourceMode } from '@/features/transaction-graph-source'
import { TransactionNodeDetailsDrawer } from '@/features/transaction-node-details'
import { getErrorMessage } from '@/shared/api'
import { type GraphLayoutMode } from '@/shared/graph'
import { useDebouncedValue } from '@/shared/lib/use-debounced-value'

const TransactionGraphWidget = lazy(async () => {
  const module = await import('@/widgets/transaction-graph')
  return { default: module.TransactionGraphWidget }
})

export const TransactionGraphPage = () => {
  const [layout, setLayout] = useState<GraphLayoutMode>('force')
  const [selectedNodeId, setSelectedNodeId] = useState('')
  const [query, setQuery] = useState('')
  const [sourceMode, setSourceMode] = useState<GraphSourceMode>('mock')
  const [backendGraph, setBackendGraph] = useState<TransactionGraphPageData | null>(null)
  const [ingestJobId, setIngestJobId] = useState<string | null>(null)
  const [statusMessage, setStatusMessage] = useState<string | null>('Showing mock graph data.')

  const [form, setForm] = useState({
    address: '',
    fromBlock: '',
    maxDepth: '3',
    maxNodes: '500',
  })

  const debouncedQuery = useDebouncedValue(query, 200)
  const ingestMutation = useIngestWalletMutation()
  const drawGraphMutation = useFetchTransactionGraphMutation()
  const jobStatusQuery = useIngestJobStatusQuery(ingestJobId)

  const graphPage =
    sourceMode === 'mock' ? mockTransactionGraphPage : (backendGraph ?? emptyTransactionGraphPage)
  const hasBackendData = backendGraph !== null
  const showBackendEmptyState = sourceMode === 'backend' && !hasBackendData

  const baseGraph = useMemo(() => transactionGraphPageToGraphData(graphPage), [graphPage])

  const filteredGraph = useMemo(
    () => filterGraphData(baseGraph, graphPage, debouncedQuery),
    [baseGraph, debouncedQuery, graphPage],
  )

  const visibleNodeIds = useMemo(() => {
    if (!debouncedQuery.trim()) {
      return null
    }
    return new Set(filteredGraph.nodes.map((node) => node.id))
  }, [debouncedQuery, filteredGraph.nodes])

  const visibleEdgeIds = useMemo(() => {
    if (!debouncedQuery.trim()) {
      return null
    }
    return new Set(filteredGraph.edges.map((edge) => edge.id))
  }, [debouncedQuery, filteredGraph.edges])

  const selectedNodeLabel = useMemo(() => {
    if (!selectedNodeId) {
      return null
    }

    return baseGraph.nodes.find((node) => node.id === selectedNodeId)?.label ?? selectedNodeId
  }, [baseGraph.nodes, selectedNodeId])

  const selectedNodeDetails = useMemo(() => {
    if (!selectedNodeId) {
      return null
    }

    return getTransactionNodeDetails(graphPage, baseGraph, selectedNodeId)
  }, [baseGraph, graphPage, selectedNodeId])

  const parsePositiveInt = (value: string) => {
    const trimmed = value.trim()
    if (!trimmed) {
      return null
    }
    const parsed = Number(trimmed)
    if (!Number.isInteger(parsed) || parsed < 0) {
      return null
    }
    return parsed
  }

  const validateRequiredFields = () => {
    const address = form.address.trim()
    const fromBlock = parsePositiveInt(form.fromBlock)

    if (!address) {
      throw new Error('Field address is required.')
    }
    if (fromBlock === null) {
      throw new Error('Field from_block is required and must be a non-negative integer.')
    }

    return { address, fromBlock }
  }

  const buildRequestPayload = () => {
    const { address, fromBlock } = validateRequiredFields()
    const maxDepth = parsePositiveInt(form.maxDepth)
    const maxNodes = parsePositiveInt(form.maxNodes)

    return {
      address,
      from_block: fromBlock,
      max_depth: maxDepth ?? 3,
      max_nodes: maxNodes ?? 500,
    }
  }

  const onSourceModeChange = (mode: GraphSourceMode) => {
    setSourceMode(mode)
    setSelectedNodeId('')

    if (mode === 'mock') {
      setStatusMessage('Showing mock graph data.')
      return
    }

    if (!backendGraph) {
      setStatusMessage(null)
      return
    }

    setStatusMessage(`Graph loaded: ${backendGraph.total_nodes} nodes, ${backendGraph.total_edges} edges.`)
  }

  const onIngest = async () => {
    try {
      const payload = buildRequestPayload()
      const result = await ingestMutation.mutateAsync(payload)
      setIngestJobId(result.job_id)
      setStatusMessage(`Ingest job accepted: ${result.job_id}. Waiting for completion...`)
    } catch (error) {
      setStatusMessage(null)
    }
  }

  const onDrawGraph = async () => {
    try {
      const payload = buildRequestPayload()
      const page = await drawGraphMutation.mutateAsync(payload)
      setBackendGraph(page)
      setSourceMode('backend')
      setSelectedNodeId('')
      setStatusMessage(`Graph loaded: ${page.total_nodes} nodes, ${page.total_edges} edges.`)
    } catch (error) {
      setStatusMessage(null)
    }
  }

  const jobStatus = jobStatusQuery.data?.status.toLowerCase()
  const jobStatusMessage = useMemo(() => {
    if (!ingestJobId || !jobStatusQuery.data) {
      return null
    }

    if (jobStatus === 'done') {
      return `Ingest job ${ingestJobId} completed. Click "Draw graph" to render.`
    }

    if (jobStatus === 'failed') {
      return jobStatusQuery.data.error ?? `Ingest job ${ingestJobId} failed.`
    }

    return `Ingest job ${ingestJobId}: ${jobStatusQuery.data.status}`
  }, [ingestJobId, jobStatus, jobStatusQuery.data])

  const resolvedStatusMessage = jobStatusMessage ?? statusMessage
  const resolvedErrorMessage =
    drawGraphMutation.error || ingestMutation.error
      ? getErrorMessage(drawGraphMutation.error ?? ingestMutation.error, 'Request failed.')
      : null

  return (
    <main className='flex h-screen w-full flex-col overflow-hidden bg-background text-foreground lg:flex-row'>
      <section className='relative min-h-0 w-full flex-1 p-3 lg:h-full lg:w-4/5'>
        <Suspense
          fallback={
            <div className='flex h-full min-h-90 items-center justify-center rounded-xl border border-border bg-card/40 text-sm text-muted-foreground'>
              Loading graph...
            </div>
          }
        >
          <TransactionGraphWidget
            graph={baseGraph}
            layout={layout}
            selectedNodeId={selectedNodeId}
            visibleNodeIds={visibleNodeIds}
            visibleEdgeIds={visibleEdgeIds}
            onSelectNode={setSelectedNodeId}
          />
        </Suspense>

        {showBackendEmptyState ? (
          <div className='pointer-events-none absolute inset-3 flex items-center justify-center rounded-xl border border-dashed border-border bg-background/80 backdrop-blur-sm'>
            <div className='max-w-sm px-6 text-center'>
              <p className='text-sm font-medium'>No backend data loaded</p>
              <p className='mt-2 text-xs text-muted-foreground'>
                Fill in the form on the right and use Fetch data, then Draw graph to load transactions from the
                backend.
              </p>
            </div>
          </div>
        ) : null}
      </section>

      <aside className='flex h-full w-full flex-col gap-4 overflow-y-auto border-t border-border bg-card/40 p-4 lg:w-1/5 lg:min-w-[320px] lg:border-l lg:border-t-0'>
        <div className='flex flex-col gap-1'>
          <h1 className='text-2xl font-semibold tracking-tight'>PayTraces</h1>
          <p className='text-xs text-muted-foreground'>Transaction graph explorer</p>
        </div>

        <TransactionGraphSourceToggle value={sourceMode} onChange={onSourceModeChange} />

        <TransactionGraphFlowForm
          value={form}
          onChange={setForm}
          onIngest={onIngest}
          onDrawGraph={onDrawGraph}
          isIngesting={ingestMutation.isPending}
          isDrawing={drawGraphMutation.isPending}
          ingestJobId={ingestJobId}
          statusMessage={resolvedStatusMessage}
          errorMessage={resolvedErrorMessage}
        />

        <TransactionGraphControls
          query={query}
          onQueryChange={setQuery}
          layout={layout}
          onLayoutChange={setLayout}
          nodeCount={filteredGraph.nodes.length}
          edgeCount={filteredGraph.edges.length}
          selectedNodeLabel={selectedNodeLabel}
        />
      </aside>

      <TransactionNodeDetailsDrawer
        open={Boolean(selectedNodeId)}
        onOpenChange={(open) => {
          if (!open) {
            setSelectedNodeId('')
          }
        }}
        details={selectedNodeDetails}
      />
    </main>
  )
}
