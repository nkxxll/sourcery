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
      </header>

      <StatsPanel title="File Stats" metrics={file.metrics} />
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
