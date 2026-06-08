import { useQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import * as d3 from 'd3'
import type { RefObject } from 'react'
import { useEffect, useMemo, useRef, useState } from 'react'

export const Route = createFileRoute('/callgraph/$versionID')({
  component: CallgraphPage,
})

type Version = {
  id: string
  codebase_id: string
  commit_hash: string
  message: string
  author_name: string
  author_email: string
  committed_at: string | null
  created_at: string
}

type FunctionCall = {
  name: string
  file: string
  line: number
  column: number
  definition_found?: boolean
}

type VersionFunction = {
  function_id: string
  file_id: string
  version_id: string
  file_path: string
  file_language: string | null
  name: string
  start_line: number
  end_line: number
  metrics: {
    definition_position_range?: {
      start?: {
        line?: number
        column?: number
      }
    }
    function_calls?: FunctionCall[]
    syntax_function_calls?: FunctionCall[]
    functions_called?: string[]
    indegree?: number
    outdegree?: number
  }
  created_at: string
}

type GraphNode = d3.SimulationNodeDatum & {
  id: string
  name: string
  filePath: string
  startLine: number | null
  endLine: number | null
  functionId: string | null
  internal: boolean
  degree: number
}

type GraphLink = d3.SimulationLinkDatum<GraphNode> & {
  source: string | GraphNode
  target: string | GraphNode
  lspResolved: boolean
}

type GraphData = {
  nodes: GraphNode[]
  links: GraphLink[]
  internalCount: number
  externalCount: number
}

type ForceSettings = {
  resolvedDistance: number
  unresolvedDistance: number
  resolvedStrength: number
  unresolvedStrength: number
  chargeStrength: number
  collisionRadius: number
  centerStrength: number
}

type ForceGraphApi = {
  focusNode: (nodeId: string) => void
}

const defaultForceSettings: ForceSettings = {
  resolvedDistance: 58,
  unresolvedDistance: 92,
  resolvedStrength: 0.62,
  unresolvedStrength: 0.24,
  chargeStrength: -360,
  collisionRadius: 34,
  centerStrength: 0.012,
}

function CallgraphPage() {
  const { versionID } = Route.useParams()
  const svgRef = useRef<SVGSVGElement | null>(null)
  const graphApiRef = useRef<ForceGraphApi | null>(null)
  const [forceSettings, setForceSettings] = useState(defaultForceSettings)
  const [showExternalFunctions, setShowExternalFunctions] = useState(false)
  const [nodeSearch, setNodeSearch] = useState('')
  const [selectedSearchIndex, setSelectedSearchIndex] = useState(-1)

  const versionQuery = useQuery({
    queryKey: ['version', versionID],
    queryFn: async () => {
      const res = await fetch(`/api/version/${versionID}`)
      if (!res.ok) {
        throw new Error(`Failed to fetch version (${res.status})`)
      }
      return res.json() as Promise<Version>
    },
  })

  const functionsQuery = useQuery({
    queryKey: ['version-functions-callgraph', versionID],
    queryFn: async () => {
      const res = await fetch(`/api/version/${versionID}/callgraph`)
      if (!res.ok) {
        throw new Error(`Failed to fetch functions (${res.status})`)
      }
      return res.json() as Promise<VersionFunction[]>
    },
  })

  const graph = useMemo(
    () => buildCallgraph(functionsQuery.data ?? []),
    [functionsQuery.data],
  )
  const displayedGraph = useMemo(
    () => filterGraph(graph, showExternalFunctions),
    [graph, showExternalFunctions],
  )
  const nodeSearchMatches = useMemo(
    () => searchNodes(displayedGraph.nodes, nodeSearch),
    [displayedGraph.nodes, nodeSearch],
  )

  useForceGraph(svgRef, displayedGraph, forceSettings, graphApiRef)

  useEffect(() => {
    setSelectedSearchIndex(-1)
  }, [displayedGraph, nodeSearch])

  const selectedSearchNode =
    selectedSearchIndex >= 0 ? (nodeSearchMatches[selectedSearchIndex] ?? null) : null

  function focusNextSearchMatch() {
    if (nodeSearchMatches.length === 0) {
      return
    }

    const nextIndex = (selectedSearchIndex + 1) % nodeSearchMatches.length
    const nextNode = nodeSearchMatches[nextIndex]
    setSelectedSearchIndex(nextIndex)
    graphApiRef.current?.focusNode(nextNode.id)
  }

  if (versionQuery.isPending || functionsQuery.isPending) {
    return <p className="text-sm text-[#6b6e73]">Loading version...</p>
  }

  if (versionQuery.error || functionsQuery.error) {
    const error = versionQuery.error ?? functionsQuery.error
    const message = error instanceof Error ? error.message : 'Unknown error'
    return <p className="text-sm text-[#6b6e73]">Error... {message}</p>
  }

  const version = versionQuery.data

  return (
    <div className="flex flex-col gap-6">
      <header className="rounded border border-[#d0d7de] bg-white p-4">
        <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
          <div>
            <p className="text-xs font-semibold uppercase tracking-wide text-[#6b6e73]">
              Version Callgraph
            </p>
            <h2 className="break-all font-mono text-xl font-semibold text-[#0f3f88]">
              {version.commit_hash}
            </h2>
          </div>
          <Link
            to="/version/$versionId"
            params={{ versionId: versionID }}
            className="text-sm font-medium text-[#0f3f88] underline"
          >
            Back to version
          </Link>
        </div>
      </header>

      <section className="flex min-h-[24rem] items-center justify-center rounded border border-[#d0d7de] bg-white p-4">
        {displayedGraph.nodes.length === 0 ? (
          <p className="text-sm text-[#6b6e73]">No functions available.</p>
        ) : (
          <div className="flex w-full flex-col gap-3">
            <div className="flex flex-wrap gap-4 text-xs text-[#6b6e73]">
              <span>{displayedGraph.internalCount} functions</span>
              <span>{displayedGraph.externalCount} external calls</span>
              <span>{displayedGraph.links.length} calls</span>
              <span>Drag nodes to pin the layout</span>
            </div>
            <ForceControls
              settings={forceSettings}
              showExternalFunctions={showExternalFunctions}
              onChange={setForceSettings}
              onShowExternalFunctionsChange={setShowExternalFunctions}
            />
            <NodeSearch
              value={nodeSearch}
              matches={nodeSearchMatches}
              selectedNode={selectedSearchNode}
              onChange={setNodeSearch}
              onFindNext={focusNextSearchMatch}
            />
            <svg
              ref={svgRef}
              className="h-[70vh] min-h-[32rem] w-full rounded bg-[#0d1117]"
              role="img"
              aria-label="Version function callgraph"
            />
          </div>
        )}
      </section>
    </div>
  )
}

function useForceGraph(
  svgRef: RefObject<SVGSVGElement | null>,
  graph: GraphData,
  settings: ForceSettings,
  graphApiRef: RefObject<ForceGraphApi | null>,
) {
  useEffect(() => {
    const svgElement = svgRef.current
    if (!svgElement || graph.nodes.length === 0) {
      return
    }

    const svg = d3.select(svgElement)
    svg.selectAll('*').remove()

    const width = svgElement.clientWidth || 960
    const height = svgElement.clientHeight || 640
    const nodes = graph.nodes.map((node) => ({ ...node }))
    const links = graph.links.map((link) => ({ ...link }))

    svg.attr('viewBox', `0 0 ${width} ${height}`)

    const zoomLayer = svg.append('g')
    const zoom = d3
      .zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.04, 5])
      .on('zoom', (event) => {
        zoomLayer.attr('transform', event.transform.toString())
      })
    svg.call(zoom)

    const link = zoomLayer
      .append('g')
      .attr('stroke', '#6b7280')
      .attr('stroke-opacity', 0.34)
      .selectAll('line')
      .data(links)
      .join('line')
      .attr('stroke-width', (edge) => (edge.lspResolved ? 1.4 : 0.8))
      .attr('stroke-dasharray', (edge) => (edge.lspResolved ? null : '4 4'))

    const node = zoomLayer
      .append('g')
      .selectAll('g')
      .data(nodes)
      .join('g')
      .attr('cursor', 'grab')

    const circle = node
      .append('circle')
      .attr('r', (item) => Math.min(16, 4 + Math.sqrt(item.degree + 1) * 2.2))
      .attr('fill', (item) => (item.internal ? '#58a6ff' : '#8b949e'))
      .attr('stroke', '#f0f6fc')
      .attr('stroke-opacity', 0.72)
      .attr('stroke-width', 1)

    graphApiRef.current = {
      focusNode(nodeId) {
        const focusedNode = nodes.find((item) => item.id === nodeId)
        if (!focusedNode) {
          return
        }

        const scale = 1.8
        const x = focusedNode.x ?? width / 2
        const y = focusedNode.y ?? height / 2
        const transform = d3.zoomIdentity
          .translate(width / 2 - x * scale, height / 2 - y * scale)
          .scale(scale)

        circle
          .attr('stroke', (item) => (item.id === nodeId ? '#f2cc60' : '#f0f6fc'))
          .attr('stroke-opacity', (item) => (item.id === nodeId ? 1 : 0.72))
          .attr('stroke-width', (item) => (item.id === nodeId ? 3 : 1))

        svg
          .transition()
          .duration(450)
          .call(zoom.transform, transform)
      },
    }

    node
      .append('text')
      .text((item) => item.name)
      .attr('x', 10)
      .attr('y', 4)
      .attr('fill', '#c9d1d9')
      .attr('font-size', 11)
      .attr('paint-order', 'stroke')
      .attr('stroke', '#0d1117')
      .attr('stroke-width', 3)

    node.append('title').text((item) => {
      const location = item.startLine
        ? `${item.filePath}:${item.startLine}`
        : item.filePath
      return `${item.name}\n${location}\n${item.degree} connected calls`
    })

    const simulation = d3
      .forceSimulation(nodes)
      .force(
        'link',
        d3
          .forceLink<GraphNode, GraphLink>(links)
          .id((item) => item.id)
          .distance((edge) =>
            edge.lspResolved
              ? settings.resolvedDistance
              : settings.unresolvedDistance,
          )
          .strength((edge) =>
            edge.lspResolved
              ? settings.resolvedStrength
              : settings.unresolvedStrength,
          ),
      )
      .force('charge', d3.forceManyBody().strength(settings.chargeStrength))
      .force('center', d3.forceCenter(width / 2, height / 2))
      .force(
        'collide',
        d3.forceCollide<GraphNode>().radius(settings.collisionRadius),
      )
      .force('x', d3.forceX(width / 2).strength(settings.centerStrength))
      .force('y', d3.forceY(height / 2).strength(settings.centerStrength))
      .on('tick', () => {
        link
          .attr('x1', (edge) => getNode(edge.source).x ?? 0)
          .attr('y1', (edge) => getNode(edge.source).y ?? 0)
          .attr('x2', (edge) => getNode(edge.target).x ?? 0)
          .attr('y2', (edge) => getNode(edge.target).y ?? 0)

        node.attr(
          'transform',
          (item) => `translate(${item.x ?? 0},${item.y ?? 0})`,
        )
      })

    node.call(
      d3
        .drag<SVGGElement, GraphNode>()
        .on('start', (event, item) => {
          if (!event.active) {
            simulation.alphaTarget(0.3).restart()
          }
          item.fx = item.x
          item.fy = item.y
        })
        .on('drag', (event, item) => {
          item.fx = event.x
          item.fy = event.y
        })
        .on('end', (event, item) => {
          if (!event.active) {
            simulation.alphaTarget(0)
          }
          item.fx = event.x
          item.fy = event.y
        }),
    )

    return () => {
      simulation.stop()
      graphApiRef.current = null
      svg.selectAll('*').remove()
    }
  }, [graph, graphApiRef, settings, svgRef])
}

function NodeSearch({
  value,
  matches,
  selectedNode,
  onChange,
  onFindNext,
}: {
  value: string
  matches: GraphNode[]
  selectedNode: GraphNode | null
  onChange: (value: string) => void
  onFindNext: () => void
}) {
  const trimmedValue = value.trim()
  const hasSearch = trimmedValue.length > 0

  return (
    <form
      className="flex flex-col gap-2 rounded border border-[#d0d7de] bg-[#f6f8fa] p-3 sm:flex-row sm:items-center"
      onSubmit={(event) => {
        event.preventDefault()
        onFindNext()
      }}
    >
      <label className="flex flex-1 flex-col gap-1 text-xs font-medium text-[#24292f]">
        Find node by name
        <input
          type="search"
          value={value}
          onChange={(event) => onChange(event.target.value)}
          placeholder="Search function names or paths"
          className="rounded border border-[#d0d7de] bg-white px-3 py-2 text-sm font-normal text-[#24292f] outline-none focus:border-[#0f3f88]"
        />
      </label>
      <button
        type="submit"
        disabled={!hasSearch || matches.length === 0}
        className="rounded border border-[#d0d7de] bg-white px-3 py-2 text-sm font-medium text-[#0f3f88] hover:bg-[#f6f8fa] disabled:cursor-not-allowed disabled:text-[#8c959f]"
      >
        Find next
      </button>
      <p className="text-xs text-[#6b6e73] sm:min-w-52">
        {hasSearch
          ? `${matches.length} match${matches.length === 1 ? '' : 'es'}`
          : 'Enter a function name'}
        {selectedNode ? `: ${selectedNode.name}` : ''}
      </p>
    </form>
  )
}

function ForceControls({
  settings,
  showExternalFunctions,
  onChange,
  onShowExternalFunctionsChange,
}: {
  settings: ForceSettings
  showExternalFunctions: boolean
  onChange: (settings: ForceSettings) => void
  onShowExternalFunctionsChange: (showExternalFunctions: boolean) => void
}) {
  return (
    <div className="grid gap-3 rounded border border-[#d0d7de] bg-[#f6f8fa] p-3 sm:grid-cols-2 lg:grid-cols-4">
      <ForceSlider
        label="Resolved distance"
        min={20}
        max={220}
        step={1}
        value={settings.resolvedDistance}
        onChange={(resolvedDistance) =>
          onChange({ ...settings, resolvedDistance })
        }
      />
      <ForceSlider
        label="Unresolved distance"
        min={20}
        max={260}
        step={1}
        value={settings.unresolvedDistance}
        onChange={(unresolvedDistance) =>
          onChange({ ...settings, unresolvedDistance })
        }
      />
      <ForceSlider
        label="Resolved strength"
        min={0}
        max={1}
        step={0.01}
        value={settings.resolvedStrength}
        onChange={(resolvedStrength) =>
          onChange({ ...settings, resolvedStrength })
        }
      />
      <ForceSlider
        label="Unresolved strength"
        min={0}
        max={1}
        step={0.01}
        value={settings.unresolvedStrength}
        onChange={(unresolvedStrength) =>
          onChange({ ...settings, unresolvedStrength })
        }
      />
      <ForceSlider
        label="Charge"
        min={-700}
        max={0}
        step={5}
        value={settings.chargeStrength}
        onChange={(chargeStrength) => onChange({ ...settings, chargeStrength })}
      />
      <ForceSlider
        label="Collision radius"
        min={4}
        max={80}
        step={1}
        value={settings.collisionRadius}
        onChange={(collisionRadius) =>
          onChange({ ...settings, collisionRadius })
        }
      />
      <ForceSlider
        label="Center pull"
        min={0}
        max={0.2}
        step={0.005}
        value={settings.centerStrength}
        onChange={(centerStrength) => onChange({ ...settings, centerStrength })}
      />
      <button
        type="button"
        className="self-end rounded border border-[#d0d7de] bg-white px-3 py-2 text-sm font-medium text-[#0f3f88] hover:bg-[#f6f8fa]"
        onClick={() => onChange(defaultForceSettings)}
      >
        Reset forces
      </button>
      <label className="flex items-center gap-2 self-end rounded border border-[#d0d7de] bg-white px-3 py-2 text-sm font-medium text-[#24292f]">
        <input
          type="checkbox"
          checked={showExternalFunctions}
          onChange={(event) =>
            onShowExternalFunctionsChange(event.target.checked)
          }
        />
        Show external functions
      </label>
    </div>
  )
}

function ForceSlider({
  label,
  min,
  max,
  step,
  value,
  onChange,
}: {
  label: string
  min: number
  max: number
  step: number
  value: number
  onChange: (value: number) => void
}) {
  return (
    <label className="flex flex-col gap-1 text-xs font-medium text-[#24292f]">
      <span className="flex items-center justify-between gap-3">
        {label}
        <span className="font-mono text-[#6b6e73]">{value}</span>
      </span>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </label>
  )
}

function filterGraph(
  graph: GraphData,
  showExternalFunctions: boolean,
): GraphData {
  if (showExternalFunctions) {
    return graph
  }

  const nodes = graph.nodes.filter((node) => node.internal)
  const internalIds = new Set(nodes.map((node) => node.id))
  const links = graph.links.filter(
    (link) =>
      internalIds.has(linkNodeId(link.source)) &&
      internalIds.has(linkNodeId(link.target)),
  )

  return {
    nodes,
    links,
    internalCount: nodes.length,
    externalCount: 0,
  }
}

function linkNodeId(node: string | GraphNode) {
  return typeof node === 'string' ? node : node.id
}

function searchNodes(nodes: GraphNode[], search: string) {
  const query = search.trim().toLowerCase()
  if (query.length === 0) {
    return []
  }

  return nodes.filter((node) => {
    const name = node.name.toLowerCase()
    const filePath = node.filePath.toLowerCase()
    return name.includes(query) || filePath.includes(query)
  })
}

function buildCallgraph(functions: VersionFunction[]): GraphData {
  const nodesById = new Map<string, GraphNode>()
  const linksById = new Map<string, GraphLink>()

  for (const func of functions) {
    const node = createFunctionNode(func)
    nodesById.set(node.id, node)
  }

  for (const func of functions) {
    const source = createFunctionNode(func)
    const calls = graphCalls(func)

    for (const call of calls) {
      const target =
        findTargetNode(call, nodesById, func.file_path) ??
        createExternalNode(call)
      nodesById.set(target.id, target)
      const linkId = `${source.id}->${target.id}`
      if (!linksById.has(linkId) && source.id !== target.id) {
        linksById.set(linkId, {
          source: source.id,
          target: target.id,
          lspResolved: call.lspResolved,
        })
      }
    }
  }

  const nodes = Array.from(nodesById.values())
  const links = Array.from(linksById.values())
  for (const link of links) {
    const sourceId =
      typeof link.source === 'string' ? link.source : link.source.id
    const targetId =
      typeof link.target === 'string' ? link.target : link.target.id
    nodesById.get(sourceId)!.degree += 1
    nodesById.get(targetId)!.degree += 1
  }

  return {
    nodes,
    links,
    internalCount: functions.length,
    externalCount: nodes.filter((node) => !node.internal).length,
  }
}

function createFunctionNode(func: VersionFunction): GraphNode {
  const definition = definitionPosition(func)
  return {
    id: functionNodeId(func),
    name: displayFunctionName(func.name),
    filePath: func.file_path,
    startLine: definition.line,
    endLine: func.end_line,
    functionId: func.function_id,
    internal: true,
    degree: 0,
  }
}

function createExternalNode(
  call: FunctionCall & { lspResolved: boolean },
): GraphNode {
  return {
    id: call.lspResolved
      ? `external:${normalizePath(call.file)}:${call.name}:${call.line}:${call.column}`
      : `external:${call.name}`,
    name: call.name,
    filePath: call.file,
    startLine: call.line || null,
    endLine: null,
    functionId: null,
    internal: false,
    degree: 0,
  }
}

function graphCalls(func: VersionFunction) {
  const functionCalls = func.metrics.function_calls ?? []
  if (functionCalls.length === 0) {
    return (func.metrics.functions_called ?? []).map((name) => ({
      name,
      file: '',
      line: 0,
      column: 0,
      lspResolved: false,
    }))
  }

  const calls: Array<FunctionCall & { lspResolved: boolean }> = []
  const seen = new Set<string>()
  for (const call of functionCalls) {
    const lspResolved = call.definition_found === true
    const key = callKey(call, lspResolved)
    if (!seen.has(key)) {
      seen.add(key)
      calls.push({ ...call, lspResolved })
    }
  }
  return calls
}

function findTargetNode(
  call: FunctionCall,
  nodesById: Map<string, GraphNode>,
  sourceFilePath: string,
): GraphNode | null {
  const normalizedCallFile = normalizePath(call.file)
  const sameFileMatches: GraphNode[] = []
  const nameMatches: GraphNode[] = []

  for (const node of nodesById.values()) {
    if (!node.internal || node.name !== call.name) {
      continue
    }
    const sameFile = pathsMatch(node.filePath, normalizedCallFile)
    if (sameFile && call.line > 0 && node.startLine === call.line) {
      return node
    }
    if (sameFile || node.filePath === sourceFilePath) {
      sameFileMatches.push(node)
    }
    nameMatches.push(node)
  }

  if (sameFileMatches.length > 0) {
    return sameFileMatches[0]
  }
  return nameMatches.length === 1 ? nameMatches[0] : null
}

function functionNodeId(func: VersionFunction) {
  const definition = definitionPosition(func)
  return `${normalizePath(func.file_path)}:${displayFunctionName(func.name)}:${definition.line}:${definition.column}`
}

function definitionPosition(func: VersionFunction) {
  return {
    line:
      func.metrics.definition_position_range?.start?.line ?? func.start_line,
    column: func.metrics.definition_position_range?.start?.column ?? 0,
  }
}

function displayFunctionName(name: string) {
  return name.replace(/:\d+:\d+$/, '')
}

function callKey(call: FunctionCall, hasDefinition: boolean) {
  if (!hasDefinition || call.line <= 0 || call.column <= 0) {
    return call.name
  }
  return `${normalizePath(call.file)}:${call.name}:${call.line}:${call.column}`
}

function normalizePath(path: string) {
  return path.replaceAll('\\', '/')
}

function pathsMatch(left: string, right: string) {
  const normalizedLeft = normalizePath(left)
  const normalizedRight = normalizePath(right)
  return (
    normalizedLeft === normalizedRight ||
    normalizedRight.endsWith(`/${normalizedLeft}`) ||
    normalizedLeft.endsWith(`/${normalizedRight}`)
  )
}

function getNode(node: string | GraphNode) {
  return typeof node === 'string' ? ({ x: 0, y: 0 } as GraphNode) : node
}
