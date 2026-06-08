import { useQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'

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

function CallgraphPage() {
  const { versionID } = Route.useParams()

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

  if (versionQuery.isPending) {
    return <p className="text-sm text-[#6b6e73]">Loading version...</p>
  }

  if (versionQuery.error) {
    const message =
      versionQuery.error instanceof Error
        ? versionQuery.error.message
        : 'Unknown error'
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
        <p className="text-sm text-[#6b6e73]">
          Callgraph visualization coming soon.
        </p>
      </section>
    </div>
  )
}
