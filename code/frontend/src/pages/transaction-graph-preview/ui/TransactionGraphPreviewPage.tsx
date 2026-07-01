import { useMemo, useState } from 'react'

import { mockTransactionGraphPage, transactionGraphPageToGraphData } from '@/entities/transaction'
import { TransactionGraphControls } from '@/features/transaction-graph-controls'
import { TransactionGraphFlowForm } from '@/features/transaction-graph-flow'
import { type GraphData, type GraphLayoutMode } from '@/shared/graph'
import { TransactionGraphWidget } from '@/widgets/transaction-graph'

const baseGraph = transactionGraphPageToGraphData({
  ...mockTransactionGraphPage,
  edges: mockTransactionGraphPage.edges.slice(0, 8),
})

export const TransactionGraphPreviewPage = () => {
  const [layout, setLayout] = useState<GraphLayoutMode>('concentric')
  const [selectedNodeId, setSelectedNodeId] = useState('')
  const [query, setQuery] = useState('')
  const [form, setForm] = useState({
    address: '',
    fromBlock: '',
    maxDepth: '3',
    maxNodes: '500',
  })

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

    const edges = baseGraph.edges.filter((edge) => {
      return (
        edge.source.toLowerCase().includes(normalized) ||
        edge.target.toLowerCase().includes(normalized) ||
        edge.label?.toLowerCase().includes(normalized) ||
        visibleNodeIds.has(edge.source) ||
        visibleNodeIds.has(edge.target)
      )
    })

    edges.forEach((edge) => {
      visibleNodeIds.add(edge.source)
      visibleNodeIds.add(edge.target)
    })

    const nodes = baseGraph.nodes.filter((node) => visibleNodeIds.has(node.id))
    return { nodes, edges } satisfies GraphData
  }, [query])

  const selectedNodeLabel = useMemo(() => {
    if (!selectedNodeId) {
      return null
    }

    return baseGraph.nodes.find((node) => node.id === selectedNodeId)?.label ?? selectedNodeId
  }, [selectedNodeId])

  return (
    <main className='min-h-screen bg-background text-foreground'>
      <section className='mx-auto flex w-full max-w-7xl flex-col gap-6 px-6 py-10'>
        <h1 className='text-2xl font-semibold tracking-tight'>Transaction Graph Preview</h1>
        <TransactionGraphFlowForm
          value={form}
          onChange={setForm}
          onIngest={() => {}}
          onDrawGraph={() => {}}
          isIngesting={false}
          isDrawing={false}
          statusMessage='Preview mode: this page reuses flow form for FSD slice validation.'
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
