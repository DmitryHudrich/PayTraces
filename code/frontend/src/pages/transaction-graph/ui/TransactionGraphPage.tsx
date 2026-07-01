import { motion } from 'framer-motion'
import { useMemo, useState } from 'react'

import {
  fetchTransactionGraph,
  ingestWallet,
  mockTransactionGraphPage,
  transactionGraphPageToGraphData,
} from '@/entities/transaction'
import { TransactionGraphControls } from '@/features/transaction-graph-controls'
import { TransactionGraphFlowForm } from '@/features/transaction-graph-flow'
import { type GraphData, type GraphLayoutMode } from '@/shared/graph'
import { TransactionGraphWidget } from '@/widgets/transaction-graph'

const mockPage = mockTransactionGraphPage

export const TransactionGraphPage = () => {
  const [layout, setLayout] = useState<GraphLayoutMode>('force')
  const [selectedNodeId, setSelectedNodeId] = useState('')
  const [query, setQuery] = useState('')
  const [graphPage, setGraphPage] = useState(mockPage)
  const [sourceMode, setSourceMode] = useState<'mock' | 'backend'>('mock')

  const [form, setForm] = useState({
    address: '',
    fromBlock: '',
    maxDepth: '3',
    maxNodes: '500',
  })

  const [isIngesting, setIsIngesting] = useState(false)
  const [isDrawing, setIsDrawing] = useState(false)
  const [ingestJobId, setIngestJobId] = useState<string | null>(null)
  const [statusMessage, setStatusMessage] = useState<string | null>(
    'Сейчас отображаются моковые данные. Заполни форму и стяни данные с backend.',
  )
  const [errorMessage, setErrorMessage] = useState<string | null>(null)

  const baseGraph = useMemo(() => transactionGraphPageToGraphData(graphPage), [graphPage])

  const filteredGraph = useMemo(() => {
    const normalized = query.trim().toLowerCase()
    if (!normalized) {
      return baseGraph
    }

    const visibleNodeIds = new Set(
      baseGraph.nodes
        .filter((node) => node.id.toLowerCase().includes(normalized) || node.label.toLowerCase().includes(normalized))
        .map((node) => node.id),
    )

    const matchingEdges = graphPage.edges.filter((edge) => {
      const edgeText = `${edge.formatted} ${edge.symbol} ${edge.tx_hash}`.toLowerCase()
      return edgeText.includes(normalized) || visibleNodeIds.has(edge.from) || visibleNodeIds.has(edge.to)
    })

    matchingEdges.forEach((edge) => {
      visibleNodeIds.add(edge.from)
      visibleNodeIds.add(edge.to)
    })

    const nodes = baseGraph.nodes.filter((node) => visibleNodeIds.has(node.id))
    const edgeIds = new Set(
      matchingEdges.map((edge, idx) => `${edge.tx_hash}-${edge.index}-${idx}`),
    )
    const edges = baseGraph.edges.filter((edge) => edgeIds.has(edge.id))

    return { nodes, edges } satisfies GraphData
  }, [baseGraph, graphPage.edges, query])

  const selectedNodeLabel = useMemo(() => {
    if (!selectedNodeId) {
      return null
    }

    return baseGraph.nodes.find((node) => node.id === selectedNodeId)?.label ?? selectedNodeId
  }, [baseGraph.nodes, selectedNodeId])

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
      throw new Error('Поле address обязательно.')
    }
    if (fromBlock === null) {
      throw new Error('Поле from_block обязательно и должно быть целым числом >= 0.')
    }

    return { address, fromBlock }
  }

  const onIngest = async () => {
    setErrorMessage(null)
    try {
      const { address, fromBlock } = validateRequiredFields()
      const maxDepth = parsePositiveInt(form.maxDepth)
      const maxNodes = parsePositiveInt(form.maxNodes)

      setIsIngesting(true)
      setStatusMessage('Запускаем ingest job...')

      const result = await ingestWallet({
        address,
        from_block: fromBlock,
        max_depth: maxDepth ?? 3,
        max_nodes: maxNodes ?? 500,
      })

      setIngestJobId(result.job_id)
      setStatusMessage(`Ingest job accepted: ${result.job_id}. Теперь нажми "Отрисовать граф".`)
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : 'Не удалось запустить ingest.')
    } finally {
      setIsIngesting(false)
    }
  }

  const onDrawGraph = async () => {
    setErrorMessage(null)
    try {
      const { address, fromBlock } = validateRequiredFields()
      const maxDepth = parsePositiveInt(form.maxDepth)
      const maxNodes = parsePositiveInt(form.maxNodes)

      setIsDrawing(true)
      setStatusMessage('Загружаем граф из backend...')

      const page = await fetchTransactionGraph({
        address,
        from_block: fromBlock,
        max_depth: maxDepth ?? 3,
        max_nodes: maxNodes ?? 500,
      })

      setGraphPage(page)
      setSourceMode('backend')
      setSelectedNodeId('')
      setStatusMessage(`Граф загружен: ${page.total_nodes} nodes, ${page.total_edges} edges.`)
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : 'Не удалось загрузить граф.')
    } finally {
      setIsDrawing(false)
    }
  }

  return (
    <main className='min-h-screen bg-background text-foreground'>
      <section className='mx-auto flex w-full max-w-7xl flex-col gap-6 px-6 py-10'>
        <motion.h1
          className='text-4xl font-semibold tracking-tight'
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.35 }}
        >
          Transaction Graph
        </motion.h1>

        <p className='text-muted-foreground'>
          Flow: ingest by wallet {'->'} fetch /graph {'->'} render Sigma graph. Current source: {sourceMode}.
        </p>

        <TransactionGraphFlowForm
          value={form}
          onChange={setForm}
          onIngest={onIngest}
          onDrawGraph={onDrawGraph}
          isIngesting={isIngesting}
          isDrawing={isDrawing}
          ingestJobId={ingestJobId}
          statusMessage={statusMessage}
          errorMessage={errorMessage}
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

        <TransactionGraphWidget
          graph={filteredGraph}
          layout={layout}
          selectedNodeId={selectedNodeId}
          onSelectNode={setSelectedNodeId}
        />
      </section>
    </main>
  )
}
