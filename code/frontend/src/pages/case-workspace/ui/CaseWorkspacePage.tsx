import { AlertCircle, ArrowLeft, Ban, Loader2, Search } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { toast } from 'sonner'

import {
  priorityClasses,
  STATUS_LABEL,
  statusClasses,
  useCaseQuery,
  useCloseCaseMutation,
} from '@/entities/case'
import { blockBounds, buildGraphData, buildNodeDetails, edgeKey } from '@/entities/case-graph'
import { Permission, useMyPermissions } from '@/entities/permission'
import {
  createView,
  pinNode,
  getView,
  useViewsQuery,
  viewKeys,
  type CaseGraphViewSummary,
} from '@/entities/view'
import { GraphTraceBar, useCaseGraph, type StreamParams } from '@/features/case-graph-stream'
import { IngestDialog } from '@/features/graph-ingest'
import { GraphTimeline } from '@/features/graph-timeline'
import { GroupsPanel } from '@/features/group-manager'
import { LabelsPanel } from '@/features/label-manager'
import { ViewsPanel } from '@/features/view-manager'
import { CaseGraphCanvas, NodeInspector } from '@/widgets/case-graph'
import { CaseOverviewPanel } from '@/pages/case-workspace/ui/CaseOverviewPanel'
import { getErrorMessage } from '@/shared/api'
import { GRAPH_LAYOUT_OPTIONS, type GraphLayoutMode, type XY } from '@/shared/graph'
import { useDebouncedValue } from '@/shared/lib/use-debounced-value'
import { useQueryClient } from '@tanstack/react-query'
import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Progress } from '@/shared/ui/progress'
import { ScrollArea } from '@/shared/ui/scroll-area'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/shared/ui/select'
import { Skeleton } from '@/shared/ui/skeleton'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/shared/ui/tabs'

type PanelTab = 'inspector' | 'views' | 'labels' | 'groups' | 'case'

export function CaseWorkspacePage() {
  const { caseId } = useParams<{ caseId: string }>()
  const queryClient = useQueryClient()
  const caseQuery = useCaseQuery(caseId)
  const { permissions } = useMyPermissions(caseId)
  const viewsQuery = useViewsQuery(caseId)
  const closeCase = useCloseCaseMutation(caseId ?? '')
  const graph = useCaseGraph(caseId)

  const [layout, setLayout] = useState<GraphLayoutMode>('force')
  const [selectedNodeId, setSelectedNodeId] = useState('')
  const [query, setQuery] = useState('')
  const [tab, setTab] = useState<PanelTab>('case')
  const [tracedChainId, setTracedChainId] = useState(1)
  const [activeViewId, setActiveViewId] = useState<string | null>(null)
  const [pinnedPositions, setPinnedPositions] = useState<Map<string, XY> | null>(null)
  const [canvasKey, setCanvasKey] = useState(0)
  const [expanding, setExpanding] = useState(false)
  const [selectedBlockRange, setSelectedBlockRange] = useState<{ from: number; to: number } | null>(null)
  const [clusterNodeIds, setClusterNodeIds] = useState<Set<string> | null>(null)

  const exportRef = useRef<(() => Map<string, XY>) | null>(null)
  const debouncedQuery = useDebouncedValue(query, 200)

  const graphData = useMemo(() => buildGraphData(graph.nodes, graph.edges), [graph.nodes, graph.edges])

  const rootNodeIds = useMemo(
    () => (graph.rootAddress ? new Set([graph.rootAddress]) : null),
    [graph.rootAddress],
  )

  const bounds = useMemo(() => blockBounds(graph.edges), [graph.edges])

  // Timeline filter: which transfers fall inside the selected block window.
  const timeline = useMemo(() => {
    if (!bounds || !selectedBlockRange) {
      return null
    }
    if (selectedBlockRange.from <= bounds.min && selectedBlockRange.to >= bounds.max) {
      return null
    }
    const edgeIds = new Set<string>()
    const nodeIds = new Set<string>()
    graph.edges.forEach((edge, index) => {
      if (edge.block >= selectedBlockRange.from && edge.block <= selectedBlockRange.to) {
        edgeIds.add(`${edgeKey(edge)}:${index}`)
        nodeIds.add(edge.from.toLowerCase())
        nodeIds.add(edge.to.toLowerCase())
      }
    })
    return { edgeIds, nodeIds }
  }, [bounds, selectedBlockRange, graph.edges])

  const filter = debouncedQuery.trim().toLowerCase()
  const searchNodeIds = useMemo(() => {
    if (!filter) {
      return null
    }
    return new Set(
      graphData.nodes
        .filter((node) => node.id.includes(filter) || node.label.toLowerCase().includes(filter))
        .map((node) => node.id),
    )
  }, [filter, graphData.nodes])

  const visibleNodeIds = useMemo(() => {
    const active = [searchNodeIds, timeline?.nodeIds ?? null, clusterNodeIds].filter(
      (set): set is Set<string> => set !== null,
    )
    if (active.length === 0) {
      return null
    }
    return active.reduce<Set<string>>(
      (acc, set) => new Set([...acc].filter((id) => set.has(id))),
      new Set(active[0]),
    )
  }, [searchNodeIds, timeline, clusterNodeIds])

  const visibleEdgeIds = useMemo(() => {
    if (!visibleNodeIds && !timeline) {
      return null
    }
    return new Set(
      graphData.edges
        .filter((edge) => {
          const nodeOk = !visibleNodeIds || (visibleNodeIds.has(edge.source) && visibleNodeIds.has(edge.target))
          const timeOk = !timeline || timeline.edgeIds.has(edge.id)
          return nodeOk && timeOk
        })
        .map((edge) => edge.id),
    )
  }, [visibleNodeIds, timeline, graphData.edges])

  const selectedDetails = useMemo(
    () => (selectedNodeId ? buildNodeDetails(selectedNodeId, graph.nodesByAddress, graph.edges) : null),
    [selectedNodeId, graph.nodesByAddress, graph.edges],
  )

  // Switching cases reuses this component instance — wipe the previous graph.
  const clearGraph = graph.clearGraph
  useEffect(() => {
    setSelectedNodeId('')
    setActiveViewId(null)
    setPinnedPositions(null)
    setClusterNodeIds(null)
    setSelectedBlockRange(null)
    setTab('case')
    clearGraph()
  }, [caseId, clearGraph])

  // Reset the timeline window to the full span whenever the graph's block
  // bounds change (new trace / expand).
  useEffect(() => {
    setSelectedBlockRange(bounds ? { from: bounds.min, to: bounds.max } : null)
  }, [bounds?.min, bounds?.max])

  // Pins delivered with a streamed view seed the canvas layout.
  useEffect(() => {
    if (graph.positions) {
      setPinnedPositions(new Map(graph.positions.map((position) => [position.address.toLowerCase(), { x: position.x, y: position.y }])))
    }
  }, [graph.positions])

  const handleSelectNode = useCallback((nodeId: string) => {
    setSelectedNodeId(nodeId)
    if (nodeId) {
      setTab('inspector')
    }
  }, [])

  const onTrace = useCallback(
    (params: StreamParams) => {
      setSelectedNodeId('')
      setClusterNodeIds(null)
      setTracedChainId(params.chainId)
      setActiveViewId(params.viewId ?? null)
      if (!params.viewId) {
        setPinnedPositions(null)
      }
      void graph.stream(params)
    },
    [graph],
  )

  // After an ingest completes, immediately stream the freshly-pulled graph.
  const onIngested = useCallback(
    (address: string, chainId: number) => {
      onTrace({ address, chainId, maxDepth: 2, viewId: null })
    },
    [onTrace],
  )

  const highlightCluster = useCallback((addresses: string[]) => {
    setClusterNodeIds(new Set(addresses.map((address) => address.toLowerCase())))
  }, [])

  const onExpand = useCallback(
    async (address: string, chainId: number) => {
      setExpanding(true)
      try {
        await graph.expand(address, chainId, 1)
      } catch (error) {
        toast.error(getErrorMessage(error, 'Could not expand the node.'))
      } finally {
        setExpanding(false)
      }
    },
    [graph],
  )

  const onSaveCurrent = useCallback(
    async (name: string, isShared: boolean) => {
      if (!caseId) {
        return
      }
      const positions = exportRef.current?.() ?? new Map<string, XY>()
      const { id } = await createView(caseId, { name, isShared })
      const entries = Array.from(positions.entries())
      await Promise.all(entries.map(([address, xy]) => pinNode(caseId, id, { address, x: xy.x, y: xy.y })))
      await queryClient.invalidateQueries({ queryKey: viewKeys.list(caseId) })
      setActiveViewId(id)
    },
    [caseId, queryClient],
  )

  const applyView = useCallback(
    async (view: CaseGraphViewSummary) => {
      if (!caseId) {
        return
      }
      try {
        const detail = await getView(caseId, view.id)
        setPinnedPositions(new Map(detail.positions.map((position) => [position.address.toLowerCase(), { x: position.x, y: position.y }])))
        setActiveViewId(view.id)
        setCanvasKey((key) => key + 1)
        toast.success(`Applied “${view.name}”`)
      } catch (error) {
        toast.error(getErrorMessage(error, 'Could not load the view.'))
      }
    },
    [caseId],
  )

  const focusAddress = useCallback((address: string, chainId: number) => {
    setSelectedNodeId(address.toLowerCase())
    setTab('inspector')
    setTracedChainId(chainId)
  }, [])

  if (!caseId) {
    return null
  }

  if (caseQuery.isPending) {
    return <WorkspaceSkeleton />
  }

  if (caseQuery.isError || !caseQuery.data) {
    return (
      <div className='mx-auto max-w-lg p-8'>
        <Alert variant='destructive'>
          <AlertCircle />
          <AlertTitle>Could not load case</AlertTitle>
          <AlertDescription>{getErrorMessage(caseQuery.error, 'Request failed.')}</AlertDescription>
        </Alert>
        <Button asChild variant='outline' className='mt-4'>
          <Link to='/'>
            <ArrowLeft />
            Back to cases
          </Link>
        </Button>
      </div>
    )
  }

  const detail = caseQuery.data
  const views = viewsQuery.data ?? []
  const nodeCount = graphData.nodes.length
  const edgeCount = graphData.edges.length
  const streamProgress =
    graph.progress.totalPages > 0
      ? Math.round((graph.progress.pagesLoaded / graph.progress.totalPages) * 100)
      : null

  const canClose = permissions.can(Permission.CaseClose) && detail.status !== 'Closed'

  return (
    <div className='flex h-full min-h-0 flex-col'>
      <div className='flex flex-wrap items-center justify-between gap-3 border-b border-border/70 px-4 py-3'>
        <div className='flex min-w-0 items-center gap-3'>
          <Button asChild variant='ghost' size='icon' className='size-8 shrink-0'>
            <Link to='/'>
              <ArrowLeft />
            </Link>
          </Button>
          <div className='min-w-0'>
            <h1 className='truncate text-lg font-semibold tracking-tight'>{detail.title}</h1>
            <div className='mt-0.5 flex items-center gap-1.5'>
              <Badge variant='outline' className={statusClasses(detail.status)}>
                {STATUS_LABEL[detail.status]}
              </Badge>
              <Badge variant='outline' className={priorityClasses(detail.priority)}>
                {detail.priority}
              </Badge>
            </div>
          </div>
        </div>
        {canClose ? (
          <Button
            variant='outline'
            size='sm'
            onClick={() =>
              closeCase.mutate(undefined, {
                onSuccess: () => toast.success('Case closed'),
                onError: (error) => toast.error(getErrorMessage(error, 'Failed to close case.')),
              })
            }
            disabled={closeCase.isPending}
          >
            {closeCase.isPending ? <Loader2 className='animate-spin' /> : <Ban />}
            Close case
          </Button>
        ) : null}
      </div>

      <div className='flex flex-wrap items-center gap-2 border-b border-border/70 px-4 py-2'>
        <GraphTraceBar addresses={detail.addresses} views={views} isStreaming={graph.isBusy} onTrace={onTrace} />
        {permissions.can(Permission.CaseAddressAdd) ? (
          <IngestDialog caseId={caseId} addresses={detail.addresses} onIngested={onIngested} />
        ) : null}
        <div className='ml-auto flex items-center gap-2'>
          <div className='relative'>
            <Search className='pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground' />
            <Input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder='Filter nodes…'
              className='h-8 w-44 pl-8 text-sm'
            />
          </div>
          <Select value={layout} onValueChange={(value) => setLayout(value as GraphLayoutMode)}>
            <SelectTrigger size='sm' className='w-32'>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {GRAPH_LAYOUT_OPTIONS.map((option) => (
                <SelectItem key={option.value} value={option.value}>
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <span className='hidden text-xs text-muted-foreground tabular-nums sm:inline'>
            {nodeCount} nodes · {edgeCount} edges
          </span>
        </div>
      </div>

      {graph.isBusy ? (
        <Progress value={streamProgress ?? undefined} className='h-0.5 rounded-none' />
      ) : null}

      <div className='flex min-h-0 flex-1'>
        <section className='flex min-w-0 flex-1 flex-col gap-2 p-3'>
          {graph.error ? (
            <Alert variant='destructive'>
              <AlertCircle />
              <AlertTitle>Graph error</AlertTitle>
              <AlertDescription>{graph.error}</AlertDescription>
            </Alert>
          ) : null}
          <div className='min-h-0 flex-1'>
            <CaseGraphCanvas
              key={canvasKey}
              graph={graphData}
              layout={layout}
              rootNodeIds={rootNodeIds}
              selectedNodeId={selectedNodeId}
              visibleNodeIds={visibleNodeIds}
              visibleEdgeIds={visibleEdgeIds}
              pinnedPositions={pinnedPositions}
              isStreaming={graph.isBusy}
              isEmpty={nodeCount === 0}
              onSelectNode={handleSelectNode}
              onExportReady={(getter) => {
                exportRef.current = getter
              }}
            />
          </div>
          {bounds && bounds.max > bounds.min && selectedBlockRange ? (
            <GraphTimeline bounds={bounds} value={selectedBlockRange} onChange={setSelectedBlockRange} />
          ) : null}
        </section>

        <aside className='flex w-96 shrink-0 flex-col border-l border-border/70'>
          <Tabs value={tab} onValueChange={(value) => setTab(value as PanelTab)} className='flex min-h-0 flex-1 flex-col'>
            <TabsList className='m-2 grid grid-cols-5'>
              <TabsTrigger value='inspector'>Node</TabsTrigger>
              <TabsTrigger value='views'>Views</TabsTrigger>
              <TabsTrigger value='labels'>Labels</TabsTrigger>
              <TabsTrigger value='groups'>Groups</TabsTrigger>
              <TabsTrigger value='case'>Case</TabsTrigger>
            </TabsList>

            <div className='min-h-0 flex-1'>
              <TabsContent value='inspector' className='h-full'>
                <NodeInspector
                  caseId={caseId}
                  details={selectedDetails}
                  chainId={tracedChainId}
                  canApplyLabel={permissions.can(Permission.LabelApply)}
                  canAddAddress={permissions.can(Permission.CaseAddressAdd)}
                  isExpanding={expanding}
                  clusterActive={clusterNodeIds !== null}
                  onExpand={onExpand}
                  onHighlightCluster={highlightCluster}
                  onClearHighlight={() => setClusterNodeIds(null)}
                />
              </TabsContent>
              <TabsContent value='views' className='h-full'>
                <ScrollArea className='h-full'>
                  <div className='p-4'>
                    <ViewsPanel
                      caseId={caseId}
                      canCreate={permissions.can(Permission.ViewCreate)}
                      canManage={permissions.canAny(
                        Permission.ViewUpdate,
                        Permission.ViewDelete,
                        Permission.ViewManageSharing,
                      )}
                      hasGraph={nodeCount > 0}
                      activeViewId={activeViewId}
                      onSaveCurrent={onSaveCurrent}
                      onApplyView={applyView}
                    />
                  </div>
                </ScrollArea>
              </TabsContent>
              <TabsContent value='labels' className='h-full'>
                <ScrollArea className='h-full'>
                  <div className='p-4'>
                    <LabelsPanel caseId={caseId} canCreate={permissions.can(Permission.LabelCreate)} />
                  </div>
                </ScrollArea>
              </TabsContent>
              <TabsContent value='groups' className='h-full'>
                <ScrollArea className='h-full'>
                  <div className='p-4'>
                    <GroupsPanel
                      caseId={caseId}
                      canCreate={permissions.can(Permission.GroupCreate)}
                      canManage={permissions.canAny(Permission.GroupUpdate, Permission.GroupDelete)}
                    />
                  </div>
                </ScrollArea>
              </TabsContent>
              <TabsContent value='case' className='h-full'>
                <CaseOverviewPanel
                  detail={detail}
                  canAddAddress={permissions.can(Permission.CaseAddressAdd)}
                  canAssign={permissions.can(Permission.CaseAssign)}
                  onFocusAddress={focusAddress}
                />
              </TabsContent>
            </div>
          </Tabs>
        </aside>
      </div>
    </div>
  )
}

function WorkspaceSkeleton() {
  return (
    <div className='flex h-full flex-col'>
      <div className='border-b border-border/70 px-4 py-3'>
        <Skeleton className='h-6 w-64' />
      </div>
      <div className='flex flex-1 gap-3 p-3'>
        <Skeleton className='flex-1 rounded-xl' />
        <Skeleton className='w-96 rounded-xl' />
      </div>
    </div>
  )
}
