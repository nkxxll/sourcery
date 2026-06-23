import { createFileRoute } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import * as d3 from 'd3'
import { useEffect, useMemo, useRef, useState } from 'react'

import type { Codebase, CodebasesByLanguage } from '#/lib/models'

export const Route = createFileRoute('/analysis')({
  validateSearch: (search: Record<string, unknown>): AnalysisSearch => ({
    codebases: parseCodebasesSearch(search.codebases) || undefined,
    version: parseVersionSearch(search.version),
  }),
  component: AnalysisPage,
})

type AnalysisSearch = {
  codebases?: string
  version: number
}

type AnalysisVersionSample = {
  codebase_id: string
  codebase_name: string
  programming_language: string
  version_id: string
  version_number: number
  committed_at: string | null
  created_at: string
}

type AnalysisMetricSample = {
  codebase_id: string
  codebase_name: string
  programming_language: string
  version_id: string
  version_number: number
  sample_number: number
  file_path: string
  value: number
}

type MetricOption = {
  key: string
  label: string
  description: string
}

type MetricResult = {
  metric: MetricOption
  samples: AnalysisMetricSample[]
}

type BoxStats = {
  language: string
  count: number
  min: number
  q1: number
  median: number
  q3: number
  max: number
  mean: number
}

type ProjectTimeSpan = {
  codebaseId: string
  codebaseName: string
  language: string
  firstCommitTime: Date
  lastCommitTime: Date
  versionCount: number
}

const ANALYSIS_METRICS: MetricOption[] = [
  {
    key: 'lines_of_code',
    label: 'Lines Of Code/File',
    description: 'Raw lines of code for each file.',
  },
  {
    key: 'effective_lines_of_code_with_brackets',
    label: 'Effective LOC With Brackets/File',
    description:
      'Non-comment lines for each file, including bracket-only lines.',
  },
  {
    key: 'effective_lines_of_code',
    label: 'Effective LOC/File',
    description: 'Non-comment, non-blank lines for each file.',
  },
  {
    key: 'comment_lines_of_code',
    label: 'Comment LOC/File',
    description: 'Comment-only lines for each file.',
  },
  {
    key: 'bracket_lines_of_code',
    label: 'Bracket LOC/File',
    description: 'Bracket-only lines for each file.',
  },
  {
    key: 'total_cyclomatic',
    label: 'Total Cyclomatic/File',
    description: 'Total file-level cyclomatic complexity.',
  },
  {
    key: 'maintainability_index_three_property',
    label: 'Maintainability Index Three Property',
    description: 'Three-property maintainability index.',
  },
  {
    key: 'maintainability_index_four_property',
    label: 'Maintainability Index Four Property',
    description: 'Four-property maintainability index.',
  },
  {
    key: 'maintainability_index_visual_studio',
    label: 'Maintainability Index Visual Studio',
    description: 'Visual Studio maintainability index.',
  },
  {
    key: 'maintainability_index_comment_percentage',
    label: 'Maintainability Index Comment Percentage',
    description: 'Maintainability index comment percentage component.',
  },
  {
    key: 'total_halstead_unique_operators',
    label: 'Total Halstead Unique Operators/File',
    description: 'Total unique Halstead operators for each file.',
  },
  {
    key: 'total_halstead_unique_operands',
    label: 'Total Halstead Unique Operands/File',
    description: 'Total unique Halstead operands for each file.',
  },
  {
    key: 'total_halstead_operators',
    label: 'Total Halstead Operators/File',
    description: 'Total Halstead operators for each file.',
  },
  {
    key: 'total_halstead_operands',
    label: 'Total Halstead Operands/File',
    description: 'Total Halstead operands for each file.',
  },
  {
    key: 'total_halstead_length',
    label: 'Total Halstead Length/File',
    description: 'Total Halstead length for each file.',
  },
  {
    key: 'total_halstead_vocabulary',
    label: 'Total Halstead Vocabulary/File',
    description: 'Total Halstead vocabulary for each file.',
  },
  {
    key: 'total_halstead_calculated_length',
    label: 'Total Halstead Calculated Length/File',
    description: 'Total calculated Halstead length for each file.',
  },
  {
    key: 'total_halstead_volume',
    label: 'Total Halstead Volume/File',
    description: 'Total Halstead volume for each file.',
  },
  {
    key: 'total_halstead_difficulty',
    label: 'Total Halstead Difficulty/File',
    description: 'Total Halstead difficulty for each file.',
  },
  {
    key: 'total_halstead_effort',
    label: 'Total Halstead Effort/File',
    description: 'Total Halstead effort for each file.',
  },
  {
    key: 'total_halstead_time_seconds',
    label: 'Total Halstead Time Seconds/File',
    description: 'Total estimated Halstead time for each file.',
  },
  {
    key: 'total_halstead_bugs',
    label: 'Total Halstead Bugs/File',
    description: 'Total estimated Halstead bugs for each file.',
  },
  {
    key: 'mean_outdegree_per_file',
    label: 'Mean Outdegree/File',
    description: 'Average function outdegree within each file.',
  },
  {
    key: 'mean_indegree_per_file',
    label: 'Mean Indegree/File',
    description: 'Average function indegree within each file.',
  },
  {
    key: 'mean_cyclomatic_per_function_per_file',
    label: 'Mean Function Cyclomatic/File',
    description: 'Average function cyclomatic complexity within each file.',
  },
  {
    key: 'function_length',
    label: 'Function Length',
    description: 'Length of each function.',
  },
  {
    key: 'cyclomatic',
    label: 'Function Cyclomatic',
    description: 'Cyclomatic complexity for each function.',
  },
  {
    key: 'cyclomatic_match_as_single_branch',
    label: 'Function Cyclomatic Match As Single Branch',
    description:
      'Function cyclomatic complexity treating match as a single branch.',
  },
  {
    key: 'indegree',
    label: 'Function Indegree',
    description: 'Indegree for each function.',
  },
  {
    key: 'outdegree',
    label: 'Function Outdegree',
    description: 'Outdegree for each function.',
  },
  {
    key: 'halstead_unique_operators',
    label: 'Function Halstead Unique Operators',
    description: 'Unique Halstead operators for each function.',
  },
  {
    key: 'halstead_unique_operands',
    label: 'Function Halstead Unique Operands',
    description: 'Unique Halstead operands for each function.',
  },
  {
    key: 'halstead_operators',
    label: 'Function Halstead Operators',
    description: 'Halstead operators for each function.',
  },
  {
    key: 'halstead_operands',
    label: 'Function Halstead Operands',
    description: 'Halstead operands for each function.',
  },
  {
    key: 'halstead_length',
    label: 'Function Halstead Length',
    description: 'Halstead length for each function.',
  },
  {
    key: 'halstead_vocabulary',
    label: 'Function Halstead Vocabulary',
    description: 'Halstead vocabulary for each function.',
  },
  {
    key: 'halstead_calculated_length',
    label: 'Function Halstead Calculated Length',
    description: 'Calculated Halstead length for each function.',
  },
  {
    key: 'halstead_volume',
    label: 'Function Halstead Volume',
    description: 'Halstead volume for each function.',
  },
  {
    key: 'halstead_difficulty',
    label: 'Function Halstead Difficulty',
    description: 'Halstead difficulty for each function.',
  },
  {
    key: 'halstead_effort',
    label: 'Function Halstead Effort',
    description: 'Halstead effort for each function.',
  },
  {
    key: 'halstead_time_seconds',
    label: 'Function Halstead Time Seconds',
    description: 'Estimated Halstead time for each function.',
  },
  {
    key: 'halstead_bugs',
    label: 'Function Halstead Bugs',
    description: 'Estimated Halstead bugs for each function.',
  },
]

const LANGUAGES = ['Go', 'OCaml'] as const
const VERSION_NUMBERS = Array.from({ length: 10 }, (_, index) => index + 1)
const DEFAULT_VERSION_NUMBER = 10

const languageKey = (language: string) => language.toLowerCase()
const isComparisonLanguage = (language: string) =>
  ['go', 'golang', 'ocaml'].includes(languageKey(language))

const displayLanguage = (language: string) =>
  ['go', 'golang'].includes(languageKey(language))
    ? 'Go'
    : languageKey(language) === 'ocaml'
      ? 'OCaml'
      : language

const parseCodebasesSearch = (value: unknown) => {
  if (Array.isArray(value)) {
    return value.filter((item) => typeof item === 'string').join(',')
  }

  return typeof value === 'string' ? value : ''
}

const selectedIdsFromSearch = (value: string) => {
  return new Set(
    value
      .split(',')
      .map((id) => id.trim())
      .filter(Boolean),
  )
}

const selectedIdsToSearch = (ids: Set<string>) => {
  return Array.from(ids).sort().join(',')
}

const parseVersionSearch = (value: unknown) => {
  const parsed = typeof value === 'string' ? Number(value) : value
  return typeof parsed === 'number' && VERSION_NUMBERS.includes(parsed)
    ? parsed
    : DEFAULT_VERSION_NUMBER
}

function AnalysisPage() {
  const search = Route.useSearch()
  const navigate = Route.useNavigate()
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() =>
    selectedIdsFromSearch(search.codebases ?? ''),
  )
  const [versionNumber, setVersionNumber] = useState(search.version)

  useEffect(() => {
    setSelectedIds(selectedIdsFromSearch(search.codebases ?? ''))
    setVersionNumber(search.version)
  }, [search.codebases, search.version])

  useEffect(() => {
    const codebases = selectedIdsToSearch(selectedIds)
    if (
      codebases === (search.codebases ?? '') &&
      versionNumber === search.version
    ) {
      return
    }

    void navigate({
      replace: true,
      search: {
        codebases: codebases || undefined,
        version: versionNumber,
      },
    })
  }, [navigate, search.codebases, search.version, selectedIds, versionNumber])

  const codebasesQuery = useQuery({
    queryKey: ['analysis-codebases'],
    queryFn: async () => {
      const res = await fetch('/api/codebases')
      if (!res.ok) {
        throw new Error(`Failed to fetch codebases (${res.status})`)
      }
      return res.json() as Promise<CodebasesByLanguage>
    },
  })

  const codebases = useMemo(() => {
    const rows = Object.values(codebasesQuery.data ?? {}).flat()
    return rows
      .filter((codebase) => isComparisonLanguage(codebase.programming_language))
      .sort((a, b) => {
        const language = displayLanguage(a.programming_language).localeCompare(
          displayLanguage(b.programming_language),
        )
        return language === 0 ? a.name.localeCompare(b.name) : language
      })
  }, [codebasesQuery.data])

  const selectedCodebases = useMemo(() => {
    return codebases.filter((codebase) => selectedIds.has(codebase.id))
  }, [codebases, selectedIds])
  const selectedIdList = useMemo(() => {
    return selectedCodebases.map((codebase) => codebase.id).join(',')
  }, [selectedCodebases])

  const versionsQuery = useQuery({
    queryKey: ['analysis-versions', selectedIdList],
    queryFn: async () => {
      const params = new URLSearchParams({ codebase_ids: selectedIdList })
      const res = await fetch(`/api/analysis/versions?${params.toString()}`)
      if (!res.ok) {
        throw new Error(`Failed to fetch analysis versions (${res.status})`)
      }
      return res.json() as Promise<AnalysisVersionSample[]>
    },
    enabled: selectedCodebases.length > 0,
    initialData: [],
  })

  const metricsQuery = useQuery({
    queryKey: ['analysis-metrics', selectedIdList, versionNumber],
    queryFn: async () => {
      const results = await Promise.all(
        ANALYSIS_METRICS.map(async (metric) => {
          const params = new URLSearchParams({
            codebase_ids: selectedIdList,
            version: String(versionNumber),
            metric: metric.key,
          })
          const res = await fetch(`/api/analysis/metrics?${params.toString()}`)
          if (!res.ok) {
            throw new Error(
              `Failed to fetch ${metric.label} samples (${res.status})`,
            )
          }
          const samples = (await res.json()) as AnalysisMetricSample[]
          return { metric, samples }
        }),
      )
      return results
    },
    enabled: selectedCodebases.length > 0,
    initialData: [],
  })

  const availableVersions = useMemo(() => {
    return new Set(versionsQuery.data.map((version) => version.version_number))
  }, [versionsQuery.data])

  const projectTimeSpans = useMemo(() => {
    return buildProjectTimeSpans(versionsQuery.data)
  }, [versionsQuery.data])

  const toggleCodebase = (codebase: Codebase) => {
    setSelectedIds((current) => {
      const next = new Set(current)
      if (next.has(codebase.id)) {
        next.delete(codebase.id)
      } else {
        next.add(codebase.id)
      }
      return next
    })
  }

  const selectLanguage = (language: string) => {
    setSelectedIds((current) => {
      const next = new Set(current)
      for (const codebase of codebases) {
        if (displayLanguage(codebase.programming_language) === language) {
          next.add(codebase.id)
        }
      }
      return next
    })
  }

  const clearSelection = () => setSelectedIds(new Set())

  if (codebasesQuery.isPending) return <>Loading...</>
  if (codebasesQuery.error) {
    const message =
      codebasesQuery.error instanceof Error
        ? codebasesQuery.error.message
        : 'Unknown error'
    return <>Error... {message}</>
  }

  return (
    <div className="flex flex-col gap-6">
      <header className="rounded border border-[#d0d7de] bg-white p-5">
        <p className="text-xs font-semibold uppercase tracking-wide text-[#6b6e73]">
          Boxplot analysis
        </p>
        <h1 className="mt-1 text-2xl font-semibold text-[#0f3f88]">
          Go vs OCaml codebase metrics
        </h1>
        <p className="mt-2 max-w-3xl text-sm text-[#4d4f53]">
          Select codebases, choose a version sample from 1 to 10, and compare
          file-level metric distributions side by side.
        </p>
      </header>

      <section className="grid gap-4 lg:grid-cols-[minmax(280px,360px)_1fr]">
        <aside className="flex flex-col gap-4 rounded border border-[#d0d7de] bg-white p-4">
          <div className="flex flex-wrap gap-2">
            {LANGUAGES.map((language) => (
              <button
                key={language}
                className="rounded border border-[#d0d7de] px-3 py-1 text-sm font-medium text-[#0f3f88]"
                type="button"
                onClick={() => selectLanguage(language)}
              >
                Select {language}
              </button>
            ))}
            <button
              className="rounded border border-[#d0d7de] px-3 py-1 text-sm font-medium text-[#4d4f53]"
              type="button"
              onClick={clearSelection}
            >
              Clear
            </button>
          </div>

          <div>
            <h2 className="text-sm font-semibold text-[#0f3f88]">Codebases</h2>
            <div className="mt-3 flex max-h-120 flex-col gap-2 overflow-auto pr-1">
              {codebases.map((codebase) => (
                <label
                  key={codebase.id}
                  className="flex cursor-pointer items-start gap-3 rounded border border-[#d0d7de] p-3 text-sm"
                >
                  <input
                    checked={selectedIds.has(codebase.id)}
                    className="mt-1"
                    type="checkbox"
                    onChange={() => toggleCodebase(codebase)}
                  />
                  <span>
                    <span className="block font-medium text-[#24292f]">
                      {codebase.name}
                    </span>
                    <span className="text-xs text-[#6b6e73]">
                      {displayLanguage(codebase.programming_language)}
                    </span>
                  </span>
                </label>
              ))}
            </div>
          </div>

          <div>
            <h2 className="text-sm font-semibold text-[#0f3f88]">
              Version sample
            </h2>
            <div className="mt-3 grid grid-cols-5 gap-2">
              {VERSION_NUMBERS.map((version) => (
                <button
                  key={version}
                  className={`rounded border px-3 py-2 text-sm font-semibold ${
                    versionNumber === version
                      ? 'border-[#0f3f88] bg-[#0f3f88] text-white'
                      : 'border-[#d0d7de] bg-white text-[#4d4f53]'
                  }`}
                  type="button"
                  onClick={() => setVersionNumber(version)}
                >
                  {version}
                </button>
              ))}
            </div>
            {selectedCodebases.length > 0 &&
            !availableVersions.has(versionNumber) ? (
              <p className="mt-2 text-xs text-[#8a5a00]">
                No selected codebase has version sample {versionNumber}.
              </p>
            ) : null}
          </div>
        </aside>

        <main className="flex flex-col gap-4">
          <SelectionSummary
            selectedCodebases={selectedCodebases}
            versionNumber={versionNumber}
            versions={versionsQuery.data}
          />
          {selectedCodebases.length > 0 ? (
            <ProjectTimeSpanCard
              isLoading={versionsQuery.isFetching && versionsQuery.data.length === 0}
              spans={projectTimeSpans}
            />
          ) : null}
          {selectedCodebases.length === 0 ? (
            <div className="rounded border border-dashed border-[#d0d7de] bg-white p-8 text-center text-sm text-[#6b6e73]">
              Select at least one Go or OCaml codebase to load boxplots.
            </div>
          ) : metricsQuery.isFetching && metricsQuery.data.length === 0 ? (
            <div className="rounded border border-[#d0d7de] bg-white p-8 text-sm text-[#6b6e73]">
              Loading metric samples...
            </div>
          ) : metricsQuery.error ? (
            <div className="rounded border border-[#d0d7de] bg-white p-8 text-sm text-[#6b6e73]">
              Error...{' '}
              {metricsQuery.error instanceof Error
                ? metricsQuery.error.message
                : 'Unknown error'}
            </div>
          ) : (
            metricsQuery.data.map((result) => (
              <MetricBoxplotCard key={result.metric.key} result={result} />
            ))
          )}
        </main>
      </section>
    </div>
  )
}

function ProjectTimeSpanCard({
  isLoading,
  spans,
}: {
  isLoading: boolean
  spans: ProjectTimeSpan[]
}) {
  return (
    <section className="rounded border border-[#d0d7de] bg-white p-4">
      <div className="mb-3 flex flex-col gap-1 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h2 className="text-lg font-semibold text-[#0f3f88]">
            Project time span
          </h2>
          <p className="text-sm text-[#6b6e73]">
            First to last commit time for selected Go and OCaml projects.
          </p>
        </div>
        <p className="text-xs text-[#6b6e73]">
          {spans.length.toLocaleString()} projects
        </p>
      </div>
      {isLoading ? (
        <p className="rounded border border-[#d0d7de] p-6 text-sm text-[#6b6e73]">
          Loading project commit times...
        </p>
      ) : spans.length === 0 ? (
        <p className="rounded border border-dashed border-[#d0d7de] p-6 text-sm text-[#6b6e73]">
          No commit times for this selection.
        </p>
      ) : (
        <>
          <ProjectTimeSpanChart spans={spans} />
          <ProjectTimeSpanTable spans={spans} />
        </>
      )}
    </section>
  )
}

function ProjectTimeSpanChart({ spans }: { spans: ProjectTimeSpan[] }) {
  const svgRef = useRef<SVGSVGElement | null>(null)

  useEffect(() => {
    const svg = d3.select(svgRef.current)
    svg.selectAll('*').remove()

    const rowHeight = 30
    const width = 880
    const height = Math.max(220, spans.length * rowHeight + 96)
    const margin = { top: 18, right: 32, bottom: 46, left: 176 }
    const plotWidth = width - margin.left - margin.right
    const plotHeight = height - margin.top - margin.bottom
    const minDate = d3.min(spans, (span) => span.firstCommitTime)
    const maxDate = d3.max(spans, (span) => span.lastCommitTime)

    if (!minDate || !maxDate) {
      return
    }

    const day = 24 * 60 * 60 * 1000
    const sameTime = minDate.getTime() === maxDate.getTime()
    const domainStart = sameTime ? new Date(minDate.getTime() - day) : minDate
    const domainEnd = sameTime ? new Date(maxDate.getTime() + day) : maxDate

    svg.attr('viewBox', `0 0 ${width} ${height}`)
    const root = svg
      .append('g')
      .attr('transform', `translate(${margin.left},${margin.top})`)

    const x = d3
      .scaleUtc()
      .domain([domainStart, domainEnd])
      .range([0, plotWidth])
      .nice()
    const y = d3
      .scaleBand()
      .domain(spans.map((span) => span.codebaseId))
      .range([0, plotHeight])
      .padding(0.34)
    const color = d3
      .scaleOrdinal<string>()
      .domain(['Go', 'OCaml'])
      .range(['#0f3f88', '#9a5b00'])

    root
      .append('g')
      .attr('stroke', '#eef0f3')
      .selectAll('line')
      .data(x.ticks(6))
      .join('line')
      .attr('x1', (tick) => x(tick))
      .attr('x2', (tick) => x(tick))
      .attr('y1', 0)
      .attr('y2', plotHeight)

    root
      .append('g')
      .attr('transform', `translate(0,${plotHeight})`)
      .call(d3.axisBottom(x).ticks(6).tickFormat(d3.utcFormat('%Y-%m')))
      .call((axis) => axis.select('.domain').attr('stroke', '#d0d7de'))
      .call((axis) => axis.selectAll('text').attr('fill', '#4d4f53'))

    root
      .append('g')
      .call(
        d3.axisLeft(y).tickFormat((codebaseId) => {
          const span = spans.find((item) => item.codebaseId === codebaseId)
          return span?.codebaseName ?? String(codebaseId)
        }),
      )
      .call((axis) => axis.select('.domain').remove())
      .call((axis) => axis.selectAll('line').remove())
      .call((axis) => axis.selectAll('text').attr('fill', '#24292f'))

    const rows = root
      .selectAll('g.project-span')
      .data(spans)
      .join('g')
      .attr('class', 'project-span')
      .attr(
        'transform',
        (span) => `translate(0,${(y(span.codebaseId) ?? 0) + y.bandwidth() / 2})`,
      )

    rows
      .append('line')
      .attr('x1', (span) => x(span.firstCommitTime))
      .attr('x2', (span) => x(span.lastCommitTime))
      .attr('y1', 0)
      .attr('y2', 0)
      .attr('stroke', (span) => color(span.language))
      .attr('stroke-width', 8)
      .attr('stroke-linecap', 'round')
      .attr('stroke-opacity', 0.68)

    rows
      .append('circle')
      .attr('cx', (span) => x(span.firstCommitTime))
      .attr('cy', 0)
      .attr('r', 4)
      .attr('fill', (span) => color(span.language))

    rows
      .append('circle')
      .attr('cx', (span) => x(span.lastCommitTime))
      .attr('cy', 0)
      .attr('r', 4)
      .attr('fill', (span) => color(span.language))

    const legend = svg
      .append('g')
      .attr('transform', `translate(${margin.left},${height - 16})`)

    LANGUAGES.forEach((language, index) => {
      const item = legend
        .append('g')
        .attr('transform', `translate(${index * 96},0)`)
      item
        .append('rect')
        .attr('width', 12)
        .attr('height', 12)
        .attr('rx', 2)
        .attr('fill', color(language))
      item
        .append('text')
        .attr('x', 18)
        .attr('y', 10)
        .attr('fill', '#4d4f53')
        .attr('font-size', 12)
        .text(language)
    })
  }, [spans])

  return (
    <svg
      ref={svgRef}
      aria-label="Project commit time spans for selected Go and OCaml codebases"
      className="h-auto w-full"
      role="img"
    />
  )
}

function ProjectTimeSpanTable({ spans }: { spans: ProjectTimeSpan[] }) {
  return (
    <div className="mt-3 overflow-x-auto">
      <table className="w-full min-w-160 border-collapse text-sm">
        <thead>
          <tr className="border-b border-[#d0d7de] text-left text-xs uppercase tracking-wide text-[#6b6e73]">
            <th className="py-2 pr-3">Project</th>
            <th className="py-2 pr-3">Language</th>
            <th className="py-2 pr-3">First commit</th>
            <th className="py-2 pr-3">Last commit</th>
            <th className="py-2 pr-3">Span</th>
            <th className="py-2 pr-3">Versions</th>
          </tr>
        </thead>
        <tbody>
          {spans.map((span) => (
            <tr key={span.codebaseId} className="border-b border-[#eef0f3]">
              <td className="py-2 pr-3 font-medium text-[#0f3f88]">
                {span.codebaseName}
              </td>
              <td className="py-2 pr-3">{span.language}</td>
              <td className="py-2 pr-3">{formatDate(span.firstCommitTime)}</td>
              <td className="py-2 pr-3">{formatDate(span.lastCommitTime)}</td>
              <td className="py-2 pr-3">{formatDuration(span)}</td>
              <td className="py-2 pr-3">{span.versionCount.toLocaleString()}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function SelectionSummary({
  selectedCodebases,
  versionNumber,
  versions,
}: {
  selectedCodebases: Codebase[]
  versionNumber: number
  versions: AnalysisVersionSample[]
}) {
  const counts = selectedCodebases.reduce<Record<string, number>>(
    (acc, codebase) => {
      const language = displayLanguage(codebase.programming_language)
      acc[language] = (acc[language] ?? 0) + 1
      return acc
    },
    {},
  )
  const matchingVersions = versions.filter(
    (version) => version.version_number === versionNumber,
  )

  return (
    <section className="rounded border border-[#d0d7de] bg-white p-4 text-sm text-[#4d4f53]">
      <div className="flex flex-wrap gap-x-6 gap-y-2">
        <span>
          <strong className="text-[#0f3f88]">Selected:</strong>{' '}
          {selectedCodebases.length} codebases
        </span>
        {Object.entries(counts).map(([language, count]) => (
          <span key={language}>
            <strong className="text-[#0f3f88]">{language}:</strong> {count}
          </span>
        ))}
        <span>
          <strong className="text-[#0f3f88]">Version sample:</strong>{' '}
          {versionNumber}
        </span>
        <span>
          <strong className="text-[#0f3f88]">Available versions:</strong>{' '}
          {matchingVersions.length}
        </span>
      </div>
    </section>
  )
}

function MetricBoxplotCard({ result }: { result: MetricResult }) {
  const stats = useMemo(() => buildBoxStats(result.samples), [result.samples])

  return (
    <section className="rounded border border-[#d0d7de] bg-white p-4">
      <div className="mb-3 flex flex-col gap-1 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h2 className="text-lg font-semibold text-[#0f3f88]">
            {result.metric.label}
          </h2>
          <p className="text-sm text-[#6b6e73]">{result.metric.description}</p>
        </div>
        <p className="text-xs text-[#6b6e73]">
          {result.samples.length.toLocaleString()} samples
        </p>
      </div>
      {stats.length === 0 ? (
        <p className="rounded border border-dashed border-[#d0d7de] p-6 text-sm text-[#6b6e73]">
          No samples for this metric and version selection.
        </p>
      ) : (
        <>
          <BoxplotChart stats={stats} />
          <StatsTable stats={stats} />
        </>
      )}
    </section>
  )
}

function BoxplotChart({ stats }: { stats: BoxStats[] }) {
  const svgRef = useRef<SVGSVGElement | null>(null)

  useEffect(() => {
    const svg = d3.select(svgRef.current)
    svg.selectAll('*').remove()

    const width = 760
    const height = 260
    const margin = { top: 18, right: 24, bottom: 42, left: 64 }
    const plotWidth = width - margin.left - margin.right
    const plotHeight = height - margin.top - margin.bottom
    const max = d3.max(stats, (stat) => stat.max) ?? 0
    const min = d3.min(stats, (stat) => stat.min) ?? 0
    const domainPadding = max === min ? Math.max(1, max * 0.1) : 0

    svg.attr('viewBox', `0 0 ${width} ${height}`)
    const root = svg
      .append('g')
      .attr('transform', `translate(${margin.left},${margin.top})`)

    const x = d3
      .scaleBand()
      .domain(stats.map((stat) => stat.language))
      .range([0, plotWidth])
      .padding(0.48)
    const y = d3
      .scaleLinear()
      .domain([Math.min(0, min - domainPadding), max + domainPadding])
      .nice()
      .range([plotHeight, 0])

    root
      .append('g')
      .attr('transform', `translate(0,${plotHeight})`)
      .call(d3.axisBottom(x))
      .call((axis) => axis.select('.domain').attr('stroke', '#d0d7de'))
      .call((axis) => axis.selectAll('text').attr('fill', '#4d4f53'))

    root
      .append('g')
      .call(d3.axisLeft(y).ticks(5))
      .call((axis) => axis.select('.domain').attr('stroke', '#d0d7de'))
      .call((axis) => axis.selectAll('line').attr('stroke', '#d0d7de'))
      .call((axis) => axis.selectAll('text').attr('fill', '#4d4f53'))

    root
      .append('g')
      .attr('stroke', '#eef0f3')
      .selectAll('line')
      .data(y.ticks(5))
      .join('line')
      .attr('x1', 0)
      .attr('x2', plotWidth)
      .attr('y1', (tick) => y(tick))
      .attr('y2', (tick) => y(tick))

    const color = d3.scaleOrdinal<string>().range(['#0f3f88', '#9a5b00'])
    const group = root
      .selectAll('g.boxplot')
      .data(stats)
      .join('g')
      .attr('class', 'boxplot')
      .attr('transform', (stat) => `translate(${x(stat.language) ?? 0},0)`)

    group
      .append('line')
      .attr('x1', x.bandwidth() / 2)
      .attr('x2', x.bandwidth() / 2)
      .attr('y1', (stat) => y(stat.min))
      .attr('y2', (stat) => y(stat.max))
      .attr('stroke', '#24292f')

    group
      .append('rect')
      .attr('x', 0)
      .attr('width', x.bandwidth())
      .attr('y', (stat) => y(stat.q3))
      .attr('height', (stat) => Math.max(1, y(stat.q1) - y(stat.q3)))
      .attr('fill', (stat) => color(stat.language))
      .attr('fill-opacity', 0.16)
      .attr('stroke', (stat) => color(stat.language))
      .attr('stroke-width', 2)

    group
      .append('line')
      .attr('x1', 0)
      .attr('x2', x.bandwidth())
      .attr('y1', (stat) => y(stat.median))
      .attr('y2', (stat) => y(stat.median))
      .attr('stroke', (stat) => color(stat.language))
      .attr('stroke-width', 3)

    group
      .append('circle')
      .attr('cx', x.bandwidth() / 2)
      .attr('cy', (stat) => y(stat.mean))
      .attr('r', 4)
      .attr('fill', '#24292f')

    group
      .append('line')
      .attr('x1', x.bandwidth() * 0.2)
      .attr('x2', x.bandwidth() * 0.8)
      .attr('y1', (stat) => y(stat.min))
      .attr('y2', (stat) => y(stat.min))
      .attr('stroke', '#24292f')

    group
      .append('line')
      .attr('x1', x.bandwidth() * 0.2)
      .attr('x2', x.bandwidth() * 0.8)
      .attr('y1', (stat) => y(stat.max))
      .attr('y2', (stat) => y(stat.max))
      .attr('stroke', '#24292f')
  }, [stats])

  return <svg ref={svgRef} className="h-auto w-full" role="img" />
}

function StatsTable({ stats }: { stats: BoxStats[] }) {
  return (
    <div className="mt-3 overflow-x-auto">
      <table className="w-full min-w-136 border-collapse text-sm">
        <thead>
          <tr className="border-b border-[#d0d7de] text-left text-xs uppercase tracking-wide text-[#6b6e73]">
            <th className="py-2 pr-3">Language</th>
            <th className="py-2 pr-3">Samples</th>
            <th className="py-2 pr-3">Min</th>
            <th className="py-2 pr-3">Q1</th>
            <th className="py-2 pr-3">Median</th>
            <th className="py-2 pr-3">Q3</th>
            <th className="py-2 pr-3">Max</th>
            <th className="py-2 pr-3">Mean</th>
          </tr>
        </thead>
        <tbody>
          {stats.map((stat) => (
            <tr key={stat.language} className="border-b border-[#eef0f3]">
              <td className="py-2 pr-3 font-medium text-[#0f3f88]">
                {stat.language}
              </td>
              <td className="py-2 pr-3">{stat.count.toLocaleString()}</td>
              <td className="py-2 pr-3">{formatNumber(stat.min)}</td>
              <td className="py-2 pr-3">{formatNumber(stat.q1)}</td>
              <td className="py-2 pr-3">{formatNumber(stat.median)}</td>
              <td className="py-2 pr-3">{formatNumber(stat.q3)}</td>
              <td className="py-2 pr-3">{formatNumber(stat.max)}</td>
              <td className="py-2 pr-3">{formatNumber(stat.mean)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function buildBoxStats(samples: AnalysisMetricSample[]): BoxStats[] {
  const grouped = d3.group(samples, (sample) =>
    displayLanguage(sample.programming_language),
  )

  return Array.from(grouped, ([language, rows]) => {
    const values = rows
      .map((row) => row.value)
      .filter((value) => Number.isFinite(value))
      .sort(d3.ascending)
    return {
      language,
      count: values.length,
      min: values[0] ?? 0,
      q1: d3.quantileSorted(values, 0.25) ?? 0,
      median: d3.quantileSorted(values, 0.5) ?? 0,
      q3: d3.quantileSorted(values, 0.75) ?? 0,
      max: values.at(-1) ?? 0,
      mean: d3.mean(values) ?? 0,
    }
  })
    .filter((stat) => stat.count > 0)
    .sort(
      (a, b) =>
        LANGUAGES.indexOf(a.language as (typeof LANGUAGES)[number]) -
        LANGUAGES.indexOf(b.language as (typeof LANGUAGES)[number]),
    )
}

function buildProjectTimeSpans(
  versions: AnalysisVersionSample[],
): ProjectTimeSpan[] {
  const grouped = d3.group(versions, (version) => version.codebase_id)

  return Array.from(grouped, ([codebaseId, rows]) => {
    const commitTimes = rows
      .map((row) => new Date(row.committed_at ?? row.created_at))
      .filter((date) => Number.isFinite(date.getTime()))
      .sort((a, b) => a.getTime() - b.getTime())

    if (commitTimes.length === 0) {
      return null
    }

    const first = commitTimes[0]
    const last = commitTimes.at(-1) ?? first
    const sample = rows[0]

    return {
      codebaseId,
      codebaseName: sample.codebase_name,
      language: displayLanguage(sample.programming_language),
      firstCommitTime: first,
      lastCommitTime: last,
      versionCount: commitTimes.length,
    }
  })
    .filter((span): span is ProjectTimeSpan => span !== null)
    .sort((a, b) => {
      const language =
        LANGUAGES.indexOf(a.language as (typeof LANGUAGES)[number]) -
        LANGUAGES.indexOf(b.language as (typeof LANGUAGES)[number])
      return language === 0
        ? a.firstCommitTime.getTime() - b.firstCommitTime.getTime()
        : language
    })
}

function formatNumber(value: number) {
  return new Intl.NumberFormat(undefined, {
    maximumFractionDigits: value >= 10 ? 1 : 3,
  }).format(value)
}

function formatDate(value: Date) {
  return new Intl.DateTimeFormat(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
  }).format(value)
}

function formatDuration(span: ProjectTimeSpan) {
  const days = Math.max(
    0,
    Math.round(
      (span.lastCommitTime.getTime() - span.firstCommitTime.getTime()) /
        (24 * 60 * 60 * 1000),
    ),
  )
  const years = days / 365.25

  if (years >= 1) {
    return `${formatNumber(years)} years`
  }

  return `${days.toLocaleString()} days`
}
