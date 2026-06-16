import { useQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import * as d3 from 'd3'
import { useEffect, useMemo, useRef, useState } from 'react'

export const Route = createFileRoute('/treemap/$versionID/$kind')({
  component: VersionTreemapPage,
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
  codebase_name: string
}

type Metrics = Record<string, unknown>

type FileTreemapItem = {
  id: string
  path: string
  metrics: Metrics
  total_functions?: number
}

type FunctionTreemapItem = {
  function_id: string
  file_path: string
  name: string
  start_line: number
  end_line: number
  metrics: Metrics
}

type TreemapItem = {
  id: string
  label: string
  path: string
  href: string
  metrics: Metrics
}

type TreemapNode = {
  name: string
  path: string
  item: TreemapItem | null
  children: TreemapNode[]
}

const fallbackMetric = 'count'
const totalHalsteadMetricPrefix = 'total_halstead'
const totalHalsteadMetricOrder = [
  'total_halstead_unique_operators',
  'total_halstead_unique_operands',
  'total_halstead_operators',
  'total_halstead_operands',
  'total_halstead_length',
  'total_halstead_vocabulary',
  'total_halstead_calculated_length',
  'total_halstead_volume',
  'total_halstead_difficulty',
  'total_halstead_effort',
  'total_halstead_time_seconds',
  'total_halstead_bugs',
]

function VersionTreemapPage() {
  const { versionID, kind } = Route.useParams()
  const treemapKind = kind === 'functions' ? 'functions' : 'files'
  const [selectedMetric, setSelectedMetric] = useState(fallbackMetric)
  const svgRef = useRef<SVGSVGElement | null>(null)

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

  const sampleRoutePrefix = isSampleCodebaseName(
    versionQuery.data?.codebase_name ?? '',
  )
    ? '/sample'
    : ''

  const itemsQuery = useQuery({
    queryKey: ['version-treemap', versionID, treemapKind, sampleRoutePrefix],
    queryFn: async () => {
      const res = await fetch(
        `/api/version/${versionID}${sampleRoutePrefix}/treemap/${treemapKind}`,
      )
      if (!res.ok) {
        throw new Error(`Failed to fetch treemap data (${res.status})`)
      }
      const rows = await res.json()
      return treemapKind === 'functions'
        ? mapFunctionItems(rows as FunctionTreemapItem[])
        : mapFileItems(rows as FileTreemapItem[])
    },
    enabled: versionQuery.data !== undefined,
  })

  const items = itemsQuery.data ?? []
  const metricOptions = useMemo(() => getMetricOptions(items), [items])
  const root = useMemo(
    () => buildTree(items, selectedMetric),
    [items, selectedMetric],
  )

  useEffect(() => {
    if (!metricOptions.includes(selectedMetric)) {
      setSelectedMetric(metricOptions[0] ?? fallbackMetric)
    }
  }, [metricOptions, selectedMetric])

  useTreemap(svgRef, root, selectedMetric)

  if (versionQuery.isPending || itemsQuery.isPending) {
    return <p className="text-sm text-[#6b6e73]">Loading treemap...</p>
  }

  if (versionQuery.error || itemsQuery.error) {
    const error = versionQuery.error ?? itemsQuery.error
    const message = error instanceof Error ? error.message : 'Unknown error'
    return <p className="text-sm text-[#6b6e73]">Error... {message}</p>
  }

  const version = versionQuery.data
  const title =
    treemapKind === 'functions' ? 'Function Treemap' : 'File Treemap'

  return (
    <div className="flex flex-col gap-6">
      <header className="rounded border border-[#d0d7de] bg-white p-4">
        <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
          <div>
            <p className="text-xs font-semibold uppercase tracking-wide text-[#6b6e73]">
              {title}
            </p>
            <h2 className="break-all font-mono text-xl font-semibold text-[#0f3f88]">
              {version.commit_hash}
            </h2>
          </div>
          <div className="flex items-center gap-4">
            <Link
              to="/treemap/$versionID/$kind"
              params={{ versionID, kind: 'files' }}
              className="text-sm font-medium text-[#0f3f88] underline"
            >
              Files
            </Link>
            <Link
              to="/treemap/$versionID/$kind"
              params={{ versionID, kind: 'functions' }}
              className="text-sm font-medium text-[#0f3f88] underline"
            >
              Functions
            </Link>
            <Link
              to="/version/$versionId"
              params={{ versionId: versionID }}
              className="text-sm font-medium text-[#0f3f88] underline"
            >
              Back to version
            </Link>
          </div>
        </div>
      </header>

      <section className="flex flex-col gap-4 rounded border border-[#d0d7de] bg-white p-4">
        <fieldset className="flex flex-wrap gap-3 text-sm text-[#4d4f53]">
          <legend className="mb-2 w-full text-xs font-semibold uppercase tracking-wide text-[#6b6e73]">
            Metric
          </legend>
          {metricOptions.map((metric) => (
            <label key={metric} className="flex items-center gap-2">
              <input
                checked={selectedMetric === metric}
                name="treemap-metric"
                onChange={() => setSelectedMetric(metric)}
                type="radio"
              />
              <span className="font-mono text-xs">{metric}</span>
            </label>
          ))}
        </fieldset>

        {items.length === 0 ? (
          <p className="text-sm text-[#6b6e73]">No {treemapKind} available.</p>
        ) : (
          <svg
            ref={svgRef}
            className="h-[72vh] min-h-[32rem] w-full rounded bg-[#0d1117]"
            role="img"
            aria-label={`${title} sized by ${selectedMetric}`}
          />
        )}
      </section>
    </div>
  )
}

function useTreemap(
  svgRef: React.RefObject<SVGSVGElement | null>,
  rootNode: TreemapNode,
  metric: string,
) {
  useEffect(() => {
    const svgElement = svgRef.current
    if (!svgElement || rootNode.children.length === 0) {
      return
    }

    const render = () => {
      const width = svgElement.clientWidth || 960
      const height = svgElement.clientHeight || 640
      const svg = d3.select(svgElement)
      svg.selectAll('*').remove()
      svg.attr('viewBox', `0 0 ${width} ${height}`)

      const root = d3
        .hierarchy(rootNode)
        .sum((node) => (node.item ? metricValue(node.item.metrics, metric) : 0))
        .sort((a, b) => (b.value ?? 0) - (a.value ?? 0))

      d3
        .treemap<TreemapNode>()
        .size([width, height])
        .paddingOuter(2)
        .paddingInner(1)(root)

      const color = d3.scaleOrdinal(d3.schemeTableau10)
      const leaves = root.leaves().filter((leaf) => (leaf.value ?? 0) > 0)
      const groups = svg
        .selectAll('g')
        .data(leaves)
        .join('g')
        .attr('transform', (node) => `translate(${node.x0},${node.y0})`)
        .attr('cursor', 'pointer')
        .on('click', (_, node) => {
          if (node.data.item) {
            window.location.href = node.data.item.href
          }
        })

      groups
        .append('rect')
        .attr('width', (node) => Math.max(0, node.x1 - node.x0))
        .attr('height', (node) => Math.max(0, node.y1 - node.y0))
        .attr('fill', (node) => color(topLevelName(node)))
        .attr('stroke', '#0d1117')
        .attr('stroke-width', 1)

      groups.append('title').text((node) => leafTitle(node, metric))

      groups
        .filter((node) => node.x1 - node.x0 > 72 && node.y1 - node.y0 > 34)
        .append('text')
        .attr('x', 5)
        .attr('y', 16)
        .attr('fill', '#f0f6fc')
        .attr('font-size', 11)
        .attr('font-family', 'ui-monospace, SFMono-Regular, Menlo, monospace')
        .text((node) =>
          truncate(node.data.name, Math.floor((node.x1 - node.x0) / 7)),
        )

      groups
        .filter((node) => node.x1 - node.x0 > 96 && node.y1 - node.y0 > 52)
        .append('text')
        .attr('x', 5)
        .attr('y', 31)
        .attr('fill', '#c9d1d9')
        .attr('font-size', 10)
        .attr('font-family', 'ui-monospace, SFMono-Regular, Menlo, monospace')
        .text((node) => formatMetric(node.value ?? 0))
    }

    render()
    const resizeObserver = new ResizeObserver(render)
    resizeObserver.observe(svgElement)
    return () => resizeObserver.disconnect()
  }, [metric, rootNode, svgRef])
}

function mapFileItems(rows: FileTreemapItem[]): TreemapItem[] {
  return rows.map((row) => ({
    id: row.id,
    label: row.path.split('/').at(-1) ?? row.path,
    path: row.path,
    href: `/file/${row.id}`,
    metrics: normalizeFileMetrics(row),
  }))
}

function isSampleCodebaseName(name: string) {
  return name.toLowerCase().includes('sample')
}

function normalizeFileMetrics(row: FileTreemapItem): Metrics {
  const metrics = flattenNumericMetrics({
    ...row.metrics,
    functions:
      typeof row.total_functions === 'number'
        ? row.total_functions
        : row.metrics.functions,
  })
  const totalHalstead = row.metrics[totalHalsteadMetricPrefix]

  if (isMetricObject(totalHalstead)) {
    for (const [key, value] of Object.entries(totalHalstead)) {
      const flatKey = `${totalHalsteadMetricPrefix}_${key}`
      if (typeof value === 'number' && Number.isFinite(value)) {
        metrics[flatKey] = value
      }
    }
  }

  return metrics
}

function mapFunctionItems(rows: FunctionTreemapItem[]): TreemapItem[] {
  return rows.map((row) => ({
    id: row.function_id,
    label: `${row.name}:${row.start_line}-${row.end_line}`,
    path: `${row.file_path}/${row.name}:${row.start_line}-${row.end_line}`,
    href: `/function/${row.function_id}`,
    metrics: flattenNumericMetrics(row.metrics),
  }))
}

function flattenNumericMetrics(metrics: Metrics): Metrics {
  const flat: Metrics = { ...metrics }
  for (const [key, value] of Object.entries(metrics)) {
    if (!isMetricObject(value)) {
      continue
    }
    for (const [nestedKey, nestedValue] of Object.entries(value)) {
      const flatKey = `${key}_${nestedKey}`
      if (
        flat[flatKey] === undefined &&
        typeof nestedValue === 'number' &&
        Number.isFinite(nestedValue)
      ) {
        flat[flatKey] = nestedValue
      }
    }
  }
  return flat
}

function isMetricObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function getMetricOptions(items: TreemapItem[]) {
  const metrics = new Set<string>()
  for (const item of items) {
    for (const [key, value] of Object.entries(item.metrics)) {
      if (typeof value === 'number' && Number.isFinite(value)) {
        metrics.add(key)
      }
    }
  }
  const totalHalsteadMetrics = totalHalsteadMetricOrder.filter((metric) =>
    metrics.delete(metric),
  )
  metrics.delete(fallbackMetric)

  return [...metrics].sort().concat(totalHalsteadMetrics, fallbackMetric)
}

function buildTree(items: TreemapItem[], metric: string): TreemapNode {
  const root: TreemapNode = { name: 'root', path: '', item: null, children: [] }
  for (const item of items) {
    if (metricValue(item.metrics, metric) <= 0) {
      continue
    }
    const segments = item.path.split('/').filter(Boolean)
    let cursor = root
    let currentPath = ''
    for (const segment of segments.slice(0, -1)) {
      currentPath = currentPath ? `${currentPath}/${segment}` : segment
      let next = cursor.children.find(
        (child) => child.name === segment && child.item === null,
      )
      if (!next) {
        next = { name: segment, path: currentPath, item: null, children: [] }
        cursor.children.push(next)
      }
      cursor = next
    }
    cursor.children.push({
      name: item.label,
      path: item.path,
      item,
      children: [],
    })
  }
  return root
}

function metricValue(metrics: Metrics, metric: string) {
  if (metric === fallbackMetric) {
    return 1
  }
  const value = metrics[metric]
  return typeof value === 'number' && Number.isFinite(value)
    ? Math.max(0, value)
    : 0
}

function topLevelName(node: d3.HierarchyRectangularNode<TreemapNode>) {
  return node.ancestors().at(-2)?.data.name ?? node.data.name
}

function leafTitle(
  node: d3.HierarchyRectangularNode<TreemapNode>,
  metric: string,
) {
  const item = node.data.item
  const value = formatMetric(node.value ?? 0)
  return item
    ? `${item.path}\n${metric}: ${value}`
    : `${node.data.path}\n${metric}: ${value}`
}

function truncate(value: string, maxLength: number) {
  if (value.length <= maxLength) {
    return value
  }
  return `${value.slice(0, Math.max(1, maxLength - 1))}...`
}

function formatMetric(value: number) {
  return Number.isInteger(value) ? value.toString() : value.toFixed(2)
}
