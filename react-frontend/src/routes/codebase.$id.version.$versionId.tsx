import { useQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { Search } from 'lucide-react'
import type { ReactNode } from 'react'
import { useMemo, useState } from 'react'

export const Route = createFileRoute('/codebase/$id/version/$versionId')({
  component: VersionDashboardPage,
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

type FilenameSearchResult = {
  file_state_id: string
  file_id: string | null
  path: string
  status: string
  score: number
}

type FunctionSearchResult = {
  function_id: string
  file_id: string
  file_path: string
  file_language: string | null
  name: string
  start_line: number
  end_line: number
  score: number
}

function VersionDashboardPage() {
  const { id, versionId } = Route.useParams()
  const [filenameQuery, setFilenameQuery] = useState('')
  const [functionQuery, setFunctionQuery] = useState('')

  const trimmedFilenameQuery = filenameQuery.trim()
  const trimmedFunctionQuery = functionQuery.trim()

  const versionQuery = useQuery({
    queryKey: ['version', versionId],
    queryFn: async () => {
      const res = await fetch(`/api/version/${versionId}`)
      if (!res.ok) {
        throw new Error(`Failed to fetch version (${res.status})`)
      }
      return res.json() as Promise<Version>
    },
  })

  const filenameResultsQuery = useQuery({
    queryKey: ['version-filename-search', versionId, trimmedFilenameQuery],
    queryFn: async () => {
      const params = new URLSearchParams({
        q: trimmedFilenameQuery,
        limit: '25',
      })
      const res = await fetch(
        `/api/version/${versionId}/files/search?${params.toString()}`,
      )
      if (!res.ok) {
        throw new Error(`Failed to search filenames (${res.status})`)
      }
      return res.json() as Promise<FilenameSearchResult[]>
    },
    enabled: trimmedFilenameQuery.length > 0,
    initialData: [],
  })

  const functionResultsQuery = useQuery({
    queryKey: ['version-function-search', versionId, trimmedFunctionQuery],
    queryFn: async () => {
      const params = new URLSearchParams({
        q: trimmedFunctionQuery,
        limit: '25',
      })
      const res = await fetch(
        `/api/version/${versionId}/functions/search?${params.toString()}`,
      )
      if (!res.ok) {
        throw new Error(`Failed to search functions (${res.status})`)
      }
      return res.json() as Promise<FunctionSearchResult[]>
    },
    enabled: trimmedFunctionQuery.length > 0,
    initialData: [],
  })

  const committedAt = useMemo(() => {
    const value =
      versionQuery.data?.committed_at ?? versionQuery.data?.created_at ?? null
    if (!value) {
      return null
    }
    const date = new Date(value)
    if (Number.isNaN(date.valueOf())) {
      return value
    }
    return date.toLocaleString('en-US', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    })
  }, [versionQuery.data])

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
              Version
            </p>
            <h2 className="break-all font-mono text-xl font-semibold text-[#0f3f88]">
              {version.commit_hash}
            </h2>
          </div>
          <Link
            to="/codebase/$id"
            params={{ id }}
            className="text-sm font-medium text-[#0f3f88] underline"
          >
            Back to codebase
          </Link>
        </div>
        <dl className="grid gap-4 text-sm text-[#4d4f53] sm:grid-cols-2">
          <div>
            <dt className="font-medium text-[#0f3f88]">Committed</dt>
            <dd>{committedAt ?? 'Unknown'}</dd>
          </div>
          <div>
            <dt className="font-medium text-[#0f3f88]">Author</dt>
            <dd>
              {version.author_name || 'Unknown'}
              {version.author_email ? ` <${version.author_email}>` : ''}
            </dd>
          </div>
          <div className="sm:col-span-2">
            <dt className="font-medium text-[#0f3f88]">Message</dt>
            <dd>{version.message || 'No commit message'}</dd>
          </div>
        </dl>
      </header>

      <div className="grid gap-6 xl:grid-cols-2">
        <SearchPanel
          title="Filename Search"
          value={filenameQuery}
          onChange={setFilenameQuery}
          placeholder="Search filenames"
          isFetching={filenameResultsQuery.isFetching}
          error={filenameResultsQuery.error}
          hasQuery={trimmedFilenameQuery.length > 0}
        >
          <FilenameResults rows={filenameResultsQuery.data} />
        </SearchPanel>

        <SearchPanel
          title="Function Search"
          value={functionQuery}
          onChange={setFunctionQuery}
          placeholder="Search functions"
          isFetching={functionResultsQuery.isFetching}
          error={functionResultsQuery.error}
          hasQuery={trimmedFunctionQuery.length > 0}
        >
          <FunctionResults rows={functionResultsQuery.data} />
        </SearchPanel>
      </div>
    </div>
  )
}

function SearchPanel({
  title,
  value,
  onChange,
  placeholder,
  isFetching,
  error,
  hasQuery,
  children,
}: {
  title: string
  value: string
  onChange: (value: string) => void
  placeholder: string
  isFetching: boolean
  error: unknown
  hasQuery: boolean
  children: ReactNode
}) {
  return (
    <section className="flex min-h-[28rem] flex-col gap-4 rounded border border-[#d0d7de] bg-white p-4">
      <div className="flex flex-col gap-3">
        <h3 className="text-base font-semibold text-[#0f3f88]">{title}</h3>
        <label className="relative block">
          <span className="sr-only">{title}</span>
          <Search
            aria-hidden="true"
            className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-[#6b6e73]"
          />
          <input
            value={value}
            onChange={(event) => onChange(event.target.value)}
            placeholder={placeholder}
            className="h-10 w-full rounded border border-[#d0d7de] bg-white pl-9 pr-3 text-sm text-[#24292f] outline-none focus:border-[#0f3f88] focus:ring-2 focus:ring-[#0f3f88]/20"
            type="search"
          />
        </label>
      </div>

      {error ? (
        <p className="text-sm text-[#6b6e73]">
          Error... {error instanceof Error ? error.message : 'Unknown error'}
        </p>
      ) : isFetching ? (
        <p className="text-sm text-[#6b6e73]">Searching...</p>
      ) : hasQuery ? (
        children
      ) : (
        <p className="text-sm text-[#6b6e73]">Enter a search term.</p>
      )}
    </section>
  )
}

function FilenameResults({ rows }: { rows: FilenameSearchResult[] }) {
  if (rows.length === 0) {
    return <p className="text-sm text-[#6b6e73]">No filenames found.</p>
  }

  return (
    <div className="overflow-auto rounded border border-[#d0d7de]">
      <table className="w-full border-collapse text-sm">
        <thead className="bg-[#f6f8fa]">
          <tr className="border-b border-[#d0d7de]">
            <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-[#6b6e73]">
              Path
            </th>
            <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-[#6b6e73]">
              Status
            </th>
            <th className="px-3 py-2 text-right text-xs font-semibold uppercase tracking-wide text-[#6b6e73]">
              Score
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr
              key={row.file_state_id}
              className="border-b border-[#d0d7de] last:border-0"
            >
              <td className="max-w-[28rem] break-all px-3 py-2 font-mono text-xs text-[#24292f]">
                {row.path}
              </td>
              <td className="whitespace-nowrap px-3 py-2 text-[#4d4f53]">
                {row.status}
              </td>
              <td className="whitespace-nowrap px-3 py-2 text-right font-mono text-[#4d4f53]">
                {row.score.toFixed(3)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function FunctionResults({ rows }: { rows: FunctionSearchResult[] }) {
  if (rows.length === 0) {
    return <p className="text-sm text-[#6b6e73]">No functions found.</p>
  }

  return (
    <div className="overflow-auto rounded border border-[#d0d7de]">
      <table className="w-full border-collapse text-sm">
        <thead className="bg-[#f6f8fa]">
          <tr className="border-b border-[#d0d7de]">
            <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-[#6b6e73]">
              Function
            </th>
            <th className="px-3 py-2 text-left text-xs font-semibold uppercase tracking-wide text-[#6b6e73]">
              File
            </th>
            <th className="px-3 py-2 text-right text-xs font-semibold uppercase tracking-wide text-[#6b6e73]">
              Score
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr
              key={row.function_id}
              className="border-b border-[#d0d7de] last:border-0"
            >
              <td className="whitespace-nowrap px-3 py-2">
                <div className="font-mono text-xs font-semibold text-[#24292f]">
                  {row.name}
                </div>
                <div className="text-xs text-[#6b6e73]">
                  {row.start_line}-{row.end_line}
                </div>
              </td>
              <td className="max-w-[28rem] break-all px-3 py-2">
                <div className="font-mono text-xs text-[#24292f]">
                  {row.file_path}
                </div>
                {row.file_language && (
                  <div className="text-xs text-[#6b6e73]">
                    {row.file_language}
                  </div>
                )}
              </td>
              <td className="whitespace-nowrap px-3 py-2 text-right font-mono text-[#4d4f53]">
                {row.score.toFixed(3)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
