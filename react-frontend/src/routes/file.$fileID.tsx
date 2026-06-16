import { useQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'

import { StatsPanel } from '#/components/stats-panel'
import { buildGithubPermalink } from '#/lib/github'

export const Route = createFileRoute('/file/$fileID')({
  component: VersionFilePage,
})

type FileState = {
  id: string
  codebase_id: string
  version_id: string
  path: string
  file_id: string | null
  status: string
  exists: boolean
  source_path: string | null
  metrics: Record<string, unknown>
  created_at: string
  total_functions: number
  codebase_url: string
  commit_hash: string
}

type HalsteadMetrics = {
  unique_operators?: number
  unique_operands?: number
  operators?: number
  operands?: number
  length?: number
  vocabulary?: number
  calculated_length?: number
  volume?: number
  difficulty?: number
  effort?: number
  time_seconds?: number
  bugs?: number
}

type MaintainabilityIndex = {
  three_property?: number
  four_property?: number
  visual_studio?: number
  comment_percentage?: number
}

const halsteadMetricLabels: Record<keyof HalsteadMetrics, string> = {
  unique_operators: 'Unique Operators',
  unique_operands: 'Unique Operands',
  operators: 'Operators',
  operands: 'Operands',
  length: 'Length',
  vocabulary: 'Vocabulary',
  calculated_length: 'Calculated Length',
  volume: 'Volume',
  difficulty: 'Difficulty',
  effort: 'Effort',
  time_seconds: 'Time Seconds',
  bugs: 'Bugs',
}

const maintainabilityIndexLabels: Record<keyof MaintainabilityIndex, string> = {
  visual_studio: 'Visual Studio Score',
  three_property: 'Three-property Score',
  four_property: 'Four-property Score',
  comment_percentage: 'Comment Percentage',
}

function VersionFilePage() {
  const { fileID } = Route.useParams()
  const fileQuery = useQuery({
    queryKey: ['file', fileID],
    queryFn: async () => {
      const res = await fetch(`/api/file/${fileID}`)
      if (!res.ok) {
        throw new Error(`Failed to fetch file (${res.status})`)
      }
      return res.json() as Promise<FileState>
    },
  })

  if (fileQuery.isPending) {
    return <p className="text-sm text-[#6b6e73]">Loading file...</p>
  }

  if (fileQuery.error) {
    return (
      <p className="text-sm text-[#6b6e73]">
        Error...{' '}
        {fileQuery.error instanceof Error
          ? fileQuery.error.message
          : 'Unknown error'}
      </p>
    )
  }

  const file = fileQuery.data
  const githubPermalink = buildGithubPermalink({
    codebaseUrl: file.codebase_url,
    commitHash: file.commit_hash,
    filePath: file.path,
  })
  const totalHalstead = getTotalHalsteadMetrics(file.metrics)
  const maintainabilityIndex = getMaintainabilityIndex(file.metrics)

  return (
    <div className="flex flex-col gap-6">
      <header className="rounded border border-[#d0d7de] bg-white p-4">
        <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
          <div>
            <p className="text-xs font-semibold uppercase tracking-wide text-[#6b6e73]">
              File
            </p>
            <h2 className="break-all font-mono text-xl font-semibold text-[#0f3f88]">
              {file.path}
            </h2>
          </div>
          <div className="flex flex-col gap-2 text-sm font-medium sm:items-end">
            {githubPermalink ? (
              <a
                href={githubPermalink}
                target="_blank"
                rel="noreferrer"
                className="text-[#0f3f88] underline"
              >
                View on GitHub
              </a>
            ) : null}
            <Link
              to="/version/$versionId"
              params={{ versionId: file.version_id }}
              className="text-[#0f3f88] underline"
            >
              Back to version
            </Link>
          </div>
        </div>
        <dl className="grid gap-4 text-sm text-[#4d4f53] sm:grid-cols-2">
          <Detail label="Status" value={file.status} />
          <Detail label="Exists" value={file.exists ? 'Yes' : 'No'} />
          <Detail label="Functions" value={file.total_functions.toString()} />
          <Detail label="Source Path" value={file.source_path ?? 'None'} />
          <Detail label="File ID" value={file.file_id ?? 'None'} />
        </dl>
        {totalHalstead ? (
          <div className="mt-4 border-t border-[#d0d7de] pt-4">
            <h3 className="mb-3 text-xs font-semibold uppercase tracking-wide text-[#6b6e73]">
              Total Halstead
            </h3>
            <dl className="grid gap-4 text-sm text-[#4d4f53] sm:grid-cols-2 lg:grid-cols-3">
              {Object.entries(halsteadMetricLabels).map(([key, label]) => (
                <Detail
                  key={key}
                  label={label}
                  value={formatHalsteadValue(
                    totalHalstead[key as keyof HalsteadMetrics],
                  )}
                />
              ))}
            </dl>
          </div>
        ) : null}
        {maintainabilityIndex ? (
          <div className="mt-4 border-t border-[#d0d7de] pt-4">
            <h3 className="mb-3 text-xs font-semibold uppercase tracking-wide text-[#6b6e73]">
              Maintainability Index
            </h3>
            <dl className="grid gap-4 text-sm text-[#4d4f53] sm:grid-cols-2 lg:grid-cols-4">
              {Object.entries(maintainabilityIndexLabels).map(([key, label]) => (
                <Detail
                  key={key}
                  label={label}
                  value={formatMaintainabilityValue(
                    maintainabilityIndex[key as keyof MaintainabilityIndex],
                    key === 'comment_percentage',
                  )}
                />
              ))}
            </dl>
          </div>
        ) : null}
      </header>

      <StatsPanel title="File Stats" metrics={file.metrics} />
    </div>
  )
}

function getTotalHalsteadMetrics(
  metrics: Record<string, unknown>,
): HalsteadMetrics | null {
  const value = metrics.total_halstead
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    return null
  }

  return value
}

function getMaintainabilityIndex(
  metrics: Record<string, unknown>,
): MaintainabilityIndex | null {
  const value = metrics.maintainability_index
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    return null
  }

  return value
}

function formatHalsteadValue(value: unknown) {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return 'None'
  }
  return Number.isInteger(value) ? value.toString() : value.toFixed(2)
}

function formatMaintainabilityValue(value: unknown, isPercentage: boolean) {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return 'None'
  }

  const formatted = value.toFixed(2)
  return isPercentage ? `${formatted}%` : formatted
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="font-medium text-[#0f3f88]">{label}</dt>
      <dd className="break-all font-mono text-xs">{value}</dd>
    </div>
  )
}
