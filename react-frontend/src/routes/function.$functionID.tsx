import { useQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'

import { StatsPanel } from '#/components/stats-panel'
import { buildGithubPermalink } from '#/lib/github'

export const Route = createFileRoute('/function/$functionID')({
  component: VersionFunctionPage,
})

type VersionFunction = {
  function_id: string
  file_id: string
  version_id: string
  file_path: string
  file_language: string | null
  name: string
  start_line: number
  end_line: number
  metrics: Record<string, unknown>
  created_at: string
  codebase_url: string
  commit_hash: string
}

function VersionFunctionPage() {
  const { functionID } = Route.useParams()
  const functionQuery = useQuery({
    queryKey: ['function', functionID],
    queryFn: async () => {
      const res = await fetch(`/api/function/${functionID}`)
      if (!res.ok) {
        throw new Error(`Failed to fetch function (${res.status})`)
      }
      return res.json() as Promise<VersionFunction>
    },
  })

  if (functionQuery.isPending) {
    return <p className="text-sm text-[#6b6e73]">Loading function...</p>
  }

  if (functionQuery.error) {
    return (
      <p className="text-sm text-[#6b6e73]">
        Error...{' '}
        {functionQuery.error instanceof Error
          ? functionQuery.error.message
          : 'Unknown error'}
      </p>
    )
  }

  const func = functionQuery.data
  const githubPermalink = buildGithubPermalink({
    codebaseUrl: func.codebase_url,
    commitHash: func.commit_hash,
    filePath: func.file_path,
    startLine: func.start_line,
    endLine: func.end_line,
  })

  return (
    <div className="flex flex-col gap-6">
      <header className="rounded border border-[#d0d7de] bg-white p-4">
        <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
          <div>
            <p className="text-xs font-semibold uppercase tracking-wide text-[#6b6e73]">
              Function
            </p>
            <h2 className="break-all font-mono text-xl font-semibold text-[#0f3f88]">
              {func.name}
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
              params={{ versionId: func.version_id }}
              className="text-[#0f3f88] underline"
            >
              Back to version
            </Link>
          </div>
        </div>
        <dl className="grid gap-4 text-sm text-[#4d4f53] sm:grid-cols-2">
          <Detail label="File" value={func.file_path} />
          <Detail label="Language" value={func.file_language ?? 'Unknown'} />
          <Detail label="Lines" value={`${func.start_line}-${func.end_line}`} />
          <Detail label="File ID" value={func.file_id} />
        </dl>
      </header>

      <StatsPanel title="Function Stats" metrics={func.metrics} />
    </div>
  )
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="font-medium text-[#0f3f88]">{label}</dt>
      <dd className="break-all font-mono text-xs">{value}</dd>
    </div>
  )
}
