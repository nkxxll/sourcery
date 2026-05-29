import { Link, Outlet, createFileRoute } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import * as d3 from 'd3'
import { useEffect, useMemo, useRef, useState } from 'react'

import { Button } from '#/components/ui/button'
import type { Codebase } from '#/lib/models'

export const Route = createFileRoute('/codebase/$id')({
  component: CodebasePage,
})

type CodebaseMetricsVersion = {
  id: string
  committed_at: string | null
  created_at: string
  metrics: unknown
}

type MetricsSeriesPoint = {
  date: Date
  value: number | null
}

const METRIC_OPTIONS = [
  { key: 'total_lines_of_code', label: 'Total LOC' },
  { key: 'total_effective_lines_of_code', label: 'Effective LOC' },
  { key: 'total_comment_lines_of_code', label: 'Comment LOC' },
  { key: 'total_cyclomatic', label: 'Cyclomatic' },
  { key: 'files', label: 'Files' },
]

const toMetricsRecord = (value: unknown): Record<string, unknown> => {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return {}
  }
  return value as Record<string, unknown>
}

const toNumber = (value: unknown): number | null => {
  if (typeof value === 'number' && Number.isFinite(value)) {
    return value
  }
  if (typeof value === 'string') {
    const parsed = Number(value)
    return Number.isFinite(parsed) ? parsed : null
  }
  return null
}

function CodebasePage() {
  const { id } = Route.useParams()
  const { isPending, error, data } = useQuery({
    queryKey: ['codebase', id],
    queryFn: async () => {
      const res = await fetch(`/api/codebases/${id}`)
      if (!res.ok) {
        throw new Error(`Failed to fetch codebase (${res.status})`)
      }
      return res.json() as Promise<Codebase>
    },
  })

  const metricsQuery = useQuery({
    queryKey: ['codebase-metrics', id],
    queryFn: async () => {
      const res = await fetch(`/api/codebases/${id}/metrics`)
      if (!res.ok) {
        throw new Error(`Failed to fetch codebase metrics (${res.status})`)
      }
      return res.json() as Promise<CodebaseMetricsVersion[]>
    },
    enabled: Boolean(id),
    initialData: [],
  })
  const isMetricsLoading =
    metricsQuery.isFetching && metricsQuery.data.length === 0

  if (isPending) return <>Loading...</>
  if (error) {
    const message = error instanceof Error ? error.message : 'Unknown error'
    return <>Error... {message}</>
  }

  return (
    <div className="flex flex-col gap-6">
      <header className="flex flex-col gap-3 rounded border border-[#d0d7de] bg-white p-4">
        <div>
          <p className="text-xs font-semibold uppercase tracking-wide text-[#6b6e73]">
            Codebase
          </p>
          <h1 className="text-2xl font-semibold text-[#0f3f88]">{data.name}</h1>
        </div>
        <dl className="grid gap-4 text-sm text-[#4d4f53] sm:grid-cols-2">
          <div>
            <dt className="font-medium text-[#0f3f88]">ID</dt>
            <dd className="break-all">{data.id}</dd>
          </div>
          <div>
            <dt className="font-medium text-[#0f3f88]">Programming language</dt>
            <dd>{data.programming_language}</dd>
          </div>
          <div className="sm:col-span-2">
            <dt className="font-medium text-[#0f3f88]">Repository</dt>
            <dd>
              <a
                className="break-all text-[#0f3f88] underline"
                href={data.url}
                rel="noreferrer"
              >
                {data.url}
              </a>
            </dd>
          </div>
          <div>
            <dt className="font-medium text-[#0f3f88]">Created</dt>
            <dd>{data.created_at}</dd>
          </div>
        </dl>
      </header>
      <section className="flex flex-col gap-3 rounded border border-[#d0d7de] bg-white p-4">
        <div>
          <p className="text-xs font-semibold uppercase tracking-wide text-[#6b6e73]">
            Metrics timeline
          </p>
          <p className="text-sm text-[#4d4f53]">
            Aggregated file metrics per version.
          </p>
        </div>
        {isMetricsLoading ? (
          <p className="text-sm text-[#6b6e73]">Loading metrics...</p>
        ) : metricsQuery.error ? (
          <p className="text-sm text-[#6b6e73]">
            Error...{' '}
            {metricsQuery.error instanceof Error
              ? metricsQuery.error.message
              : 'Unknown error'}
          </p>
        ) : (
          <CodebaseMetricsChart versions={metricsQuery.data} />
        )}
      </section>
      <Outlet />
    </div>
  )
}

function CodebaseMetricsChart({
  versions,
}: {
  versions: CodebaseMetricsVersion[]
}) {
  const svgRef = useRef<SVGSVGElement | null>(null)
  const containerRef = useRef<HTMLDivElement | null>(null)
  const [visibleMetrics, setVisibleMetrics] = useState<Set<string>>(() => {
    return new Set(METRIC_OPTIONS.map(({ key }) => key))
  })
  const [hoveredDate, setHoveredDate] = useState<Date | null>(null)
  const [tooltipPos, setTooltipPos] = useState<{ x: number; y: number } | null>(
    null,
  )

  const timeline = useMemo(() => {
    return versions
      .map((version) => ({
        date: new Date(version.committed_at ?? version.created_at),
        metrics: toMetricsRecord(version.metrics),
      }))
      .filter((entry) => !Number.isNaN(entry.date.valueOf()))
      .sort((a, b) => a.date.getTime() - b.date.getTime())
  }, [versions])

  const series = useMemo(() => {
    const availableMetrics = METRIC_OPTIONS.filter(({ key }) =>
      timeline.some((entry) => toNumber(entry.metrics[key]) !== null),
    )
    return availableMetrics.map(({ key, label }) => ({
      key,
      label,
      values: timeline.map((entry) => ({
        date: entry.date,
        value: toNumber(entry.metrics[key]),
      })),
    }))
  }, [timeline])

  const colorScale = useMemo(() => {
    return d3
      .scaleOrdinal<string>()
      .domain(series.map((metric) => metric.key))
      .range(d3.schemeTableau10)
  }, [series])

  const toggleMetric = (key: string) => {
    setVisibleMetrics((prev) => {
      const updated = new Set(prev)
      if (updated.has(key)) {
        updated.delete(key)
      } else {
        updated.add(key)
      }
      return updated
    })
  }

  useEffect(() => {
    if (!svgRef.current) {
      return
    }

    const svg = d3.select(svgRef.current)
    svg.selectAll('*').remove()

    if (timeline.length === 0 || series.length === 0) {
      return
    }

    const [minDate, maxDate] = d3.extent(timeline, (entry) => entry.date)
    if (minDate === undefined) {
      return
    }

    const width = 1000
    const height = 400
    const margin = { top: 12, right: 24, bottom: 30, left: 56 }

    const visibleSeries = series.filter((metric) =>
      visibleMetrics.has(metric.key),
    )

    if (visibleSeries.length === 0) {
      return
    }

    const maxValue =
      d3.max(visibleSeries, (metric) =>
        d3.max(metric.values, (entry) => entry.value ?? 0),
      ) ?? 0

    const xScale = d3
      .scaleTime()
      .domain([minDate, maxDate])
      .range([margin.left, width - margin.right])
    const yScale = d3
      .scaleLinear()
      .domain([0, maxValue])
      .nice()
      .range([height - margin.bottom, margin.top])

    svg
      .attr('viewBox', `0 0 ${width} ${height}`)
      .attr('preserveAspectRatio', 'xMidYMid meet')

    const xAxis = d3.axisBottom(xScale).ticks(6).tickSizeOuter(0)
    const yAxis = d3.axisLeft(yScale).ticks(4).tickSizeOuter(0)

    const xAxisGroup = svg
      .append('g')
      .attr('transform', `translate(0,${height - margin.bottom})`)
      .call(xAxis)
    const yAxisGroup = svg
      .append('g')
      .attr('transform', `translate(${margin.left},0)`)
      .call(yAxis)

    xAxisGroup.selectAll('text').attr('fill', '#4d4f53').attr('font-size', 10)
    yAxisGroup.selectAll('text').attr('fill', '#4d4f53').attr('font-size', 10)
    svg
      .selectAll('path,line')
      .attr('stroke', '#d0d7de')
      .attr('shape-rendering', 'crispEdges')

    const line = d3
      .line<MetricsSeriesPoint>()
      .defined((entry) => entry.value !== null)
      .x((entry) => xScale(entry.date))
      .y((entry) => yScale(entry.value ?? 0))

    const lineGroup = svg
      .append('g')
      .attr('fill', 'none')
      .attr('stroke-width', 2.5)

    visibleSeries.forEach((metric) => {
      lineGroup
        .append('path')
        .datum(metric.values)
        .attr('stroke', colorScale(metric.key))
        .attr('d', line)
    })
  }, [timeline, series, colorScale, visibleMetrics])

  if (versions.length === 0) {
    return <p className="text-sm text-[#6b6e73]">No metrics available yet.</p>
  }

  if (timeline.length === 0 || series.length === 0) {
    return (
      <p className="text-sm text-[#6b6e73]">
        No numeric metrics available yet.
      </p>
    )
  }

  return (
    <div className="flex flex-col gap-4">
      <svg
        ref={svgRef}
        className="h-96 w-full"
        role="img"
        aria-label="Codebase metrics timeline"
      />
      <div className="flex flex-wrap gap-4 text-sm text-[#4d4f53]">
        {series.map((metric) => (
          <label
            key={metric.key}
            className="flex cursor-pointer items-center gap-2 rounded px-2 py-1 hover:bg-gray-50"
          >
            <input
              type="checkbox"
              checked={visibleMetrics.has(metric.key)}
              onChange={() => toggleMetric(metric.key)}
              className="h-4 w-4 cursor-pointer rounded border-gray-300"
              aria-label={`Toggle ${metric.label} metric`}
            />
            <span
              className="h-2.5 w-2.5 rounded-full"
              style={{ backgroundColor: colorScale(metric.key) }}
            />
            <span>{metric.label}</span>
          </label>
        ))}
      </div>
    </div>
  )
}
