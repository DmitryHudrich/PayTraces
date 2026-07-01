import assignCircular from 'graphology-layout/circular'
import forceAtlas2 from 'graphology-layout-forceatlas2'
import Graph from 'graphology'
import Sigma from 'sigma'
import { useEffect, useMemo, useRef } from 'react'

import type { GraphAdapterProps, GraphData, GraphLayoutMode } from '@/shared/graph/contract/graph'

type SigmaNodeAttrs = {
  x: number
  y: number
  size: number
  label: string
  color: string
  group: string
}

type SigmaEdgeAttrs = {
  size: number
  color: string
}

export const SigmaGraphAdapter = ({ graph, layout, selectedNodeId, onNodeSelect }: GraphAdapterProps) => {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const rendererRef = useRef<Sigma<SigmaNodeAttrs, SigmaEdgeAttrs> | null>(null)
  const graphRef = useRef<Graph<SigmaNodeAttrs, SigmaEdgeAttrs> | null>(null)

  const preparedGraph = useMemo(() => buildGraph(graph, layout), [graph, layout])

  useEffect(() => {
    const container = containerRef.current
    if (!container) {
      return
    }

    const renderer = new Sigma<SigmaNodeAttrs, SigmaEdgeAttrs>(preparedGraph, container, {
      renderEdgeLabels: false,
      defaultNodeType: 'circle',
      defaultEdgeType: 'arrow',
      labelColor: { color: '#e4e4e7' },
      edgeLabelColor: { color: '#e4e4e7' },
      labelDensity: 0.05,
      labelGridCellSize: 100,
      minCameraRatio: 0.2,
      maxCameraRatio: 5,
      stagePadding: 24,
      zoomToSizeRatioFunction: (ratio) => ratio,
      nodeReducer: (node, data) => {
        if (!selectedNodeId) {
          return data
        }

        const neighborSet = getNeighborhood(preparedGraph, selectedNodeId)
        const isActive = node === selectedNodeId || neighborSet.has(node)
        return {
          ...data,
          color: isActive ? data.color : '#27272a',
          label: isActive ? data.label : '',
        }
      },
      edgeReducer: (edge, data) => {
        if (!selectedNodeId) {
          return data
        }

        const [source, target] = preparedGraph.extremities(edge)
        const neighborSet = getNeighborhood(preparedGraph, selectedNodeId)
        const isActive =
          source === selectedNodeId ||
          target === selectedNodeId ||
          (neighborSet.has(source) && neighborSet.has(target))

        return {
          ...data,
          color: isActive ? data.color : '#27272a',
          hidden: !isActive,
        }
      },
    })

    rendererRef.current = renderer
    graphRef.current = preparedGraph

    renderer.on('clickNode', ({ node }) => {
      onNodeSelect?.(node)
    })

    renderer.on('clickStage', () => {
      onNodeSelect?.('')
    })

    return () => {
      renderer.kill()
      rendererRef.current = null
      graphRef.current = null
    }
  }, [onNodeSelect, preparedGraph, selectedNodeId])

  useEffect(() => {
    rendererRef.current?.refresh()
  }, [selectedNodeId])

  return <div ref={containerRef} className='h-[70vh] min-h-[540px] w-full rounded-xl border border-border bg-zinc-950' />
}

function buildGraph(graphData: GraphData, layout: GraphLayoutMode) {
  const graph = new Graph<SigmaNodeAttrs, SigmaEdgeAttrs>({ type: 'directed', multi: false, allowSelfLoops: false })

  graphData.nodes.forEach((node) => {
    graph.addNode(node.id, {
      x: Math.random(),
      y: Math.random(),
      size: mapWeightToSize(node.weight ?? 1),
      label: node.label,
      color: colorByGroup(node.group),
      group: node.group ?? 'default',
    })
  })

  graphData.edges.forEach((edge) => {
    if (!graph.hasNode(edge.source) || !graph.hasNode(edge.target) || graph.hasEdge(edge.id)) {
      return
    }
    graph.addEdgeWithKey(edge.id, edge.source, edge.target, {
      size: Math.max(1, Math.min(5, edge.weight ?? 1)),
      color: '#52525b',
    })
  })

  applyLayout(graph, layout)
  return graph
}

function applyLayout(graph: Graph<SigmaNodeAttrs, SigmaEdgeAttrs>, layout: GraphLayoutMode) {
  if (layout === 'concentric') {
    assignCircular(graph)
    return
  }

  if (layout === 'breadthfirst') {
    assignBreadthFirst(graph)
    return
  }

  if (graph.order > 1) {
    forceAtlas2.assign(graph, {
      iterations: 180,
      settings: forceAtlas2.inferSettings(graph),
    })
  }
}

function assignBreadthFirst(graph: Graph<SigmaNodeAttrs, SigmaEdgeAttrs>) {
  if (graph.order === 0) {
    return
  }

  const nodes = graph.nodes()
  const root = nodes.reduce((best: string, current: string) => {
    return graph.degree(current) > graph.degree(best) ? current : best
  }, nodes[0]!)

  const depth = new Map<string, number>([[root, 0]])
  const queue = [root]

  while (queue.length > 0) {
    const current = queue.shift()!
    const nextDepth = (depth.get(current) ?? 0) + 1
    graph.forEachOutboundNeighbor(current, (neighbor: string) => {
      if (!depth.has(neighbor)) {
        depth.set(neighbor, nextDepth)
        queue.push(neighbor)
      }
    })
    graph.forEachInboundNeighbor(current, (neighbor: string) => {
      if (!depth.has(neighbor)) {
        depth.set(neighbor, nextDepth)
        queue.push(neighbor)
      }
    })
  }

  const layers = new Map<number, string[]>()
  nodes.forEach((node: string) => {
    const level = depth.get(node) ?? 0
    const layer = layers.get(level) ?? []
    layer.push(node)
    layers.set(level, layer)
  })

  const maxWidth = Math.max(...Array.from(layers.values()).map((layer) => layer.length), 1)
  for (const [level, layer] of layers.entries()) {
    layer.forEach((node, index) => {
      const x = maxWidth === 1 ? 0 : (index / (maxWidth - 1)) * 2 - 1
      const y = level * 0.55
      graph.setNodeAttribute(node, 'x', x)
      graph.setNodeAttribute(node, 'y', y)
    })
  }
}

function getNeighborhood(graph: Graph<SigmaNodeAttrs, SigmaEdgeAttrs>, nodeId: string) {
  const neighbors = new Set<string>()
  graph.forEachNeighbor(nodeId, (neighbor: string) => neighbors.add(neighbor))
  return neighbors
}

function colorByGroup(group?: string) {
  if (group === 'wallet') {
    return '#2563eb'
  }
  if (group === 'exchange') {
    return '#7c3aed'
  }
  if (group === 'risk') {
    return '#dc2626'
  }
  return '#71717a'
}

function mapWeightToSize(weight: number) {
  return Math.max(6, Math.min(22, 5 + weight * 0.8))
}
