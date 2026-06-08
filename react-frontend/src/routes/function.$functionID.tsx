import { useQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'

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
          <Link
            to="/version/$versionId"
            params={{ versionId: func.version_id }}
            className="text-sm font-medium text-[#0f3f88] underline"
          >
            Back to version
          </Link>
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

function StatsPanel({
  title,
  metrics,
}: {
  title: string
  metrics: Record<string, unknown>
}) {
  const entries = Object.entries(metrics)
  if (entries.length === 0) {
    return (
      <section className="rounded border border-[#d0d7de] bg-white p-4">
        <h3 className="mb-3 text-base font-semibold text-[#0f3f88]">{title}</h3>
        <p className="text-sm text-[#6b6e73]">No stats available.</p>
      </section>
    )
  }

  return (
    <section className="rounded border border-[#d0d7de] bg-white p-4">
      <h3 className="mb-3 text-base font-semibold text-[#0f3f88]">{title}</h3>
      <dl className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {entries.map(([key, value]) => (
          <div key={key} className="rounded border border-[#d0d7de] p-3">
            <dt className="text-xs font-semibold uppercase tracking-wide text-[#6b6e73]">
              {key.replaceAll('_', ' ')}
            </dt>
            <dd className="mt-1 break-all font-mono text-sm text-[#24292f]">
              {formatValue(value)}
            </dd>
          </div>
        ))}
      </dl>
    </section>
  )
}

function formatValue(value: unknown) {
  if (value === null || value === undefined) {
    return 'None'
  }
  if (typeof value === 'object') {
    return JSON.stringify(value)
  }
  return String(value)
}
