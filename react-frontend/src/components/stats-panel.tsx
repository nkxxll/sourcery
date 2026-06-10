import type { ReactNode } from 'react'

type JsonObject = Record<string, unknown>

export function StatsPanel({
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
              {formatMetricLabel(key)}
            </dt>
            <dd className="mt-2 text-sm text-[#24292f]">
              <MetricValue value={value} />
            </dd>
          </div>
        ))}
      </dl>
    </section>
  )
}

function MetricValue({ value }: { value: unknown }) {
  if (value === null || value === undefined) {
    return <span className="font-mono text-[#6b6e73]">None</span>
  }

  if (Array.isArray(value)) {
    return <MetricArray value={value} />
  }

  if (isPlainObject(value)) {
    return <MetricObject value={value} />
  }

  return <span className="break-all font-mono">{String(value)}</span>
}

function MetricArray({ value }: { value: unknown[] }) {
  if (value.length === 0) {
    return <span className="font-mono text-[#6b6e73]">None</span>
  }

  if (value.every(isPlainObject)) {
    const keys = [...new Set(value.flatMap((item) => Object.keys(item)))]
    return (
      <div className="overflow-auto rounded border border-[#d0d7de]">
        <table className="w-full border-collapse text-xs">
          <thead className="bg-[#f6f8fa]">
            <tr className="border-b border-[#d0d7de]">
              {keys.map((key) => (
                <th
                  key={key}
                  className="px-2 py-1.5 text-left font-semibold uppercase tracking-wide text-[#6b6e73]"
                >
                  {formatMetricLabel(key)}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {value.map((item, index) => (
              <tr
                key={index}
                className="border-b border-[#d0d7de] last:border-0"
              >
                {keys.map((key) => (
                  <td key={key} className="px-2 py-1.5 align-top">
                    <MetricValue value={item[key]} />
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    )
  }

  return (
    <ol className="flex list-decimal flex-col gap-1 pl-5">
      {value.map((item, index) => (
        <li key={index}>
          <MetricValue value={item} />
        </li>
      ))}
    </ol>
  )
}

function MetricObject({ value }: { value: JsonObject }) {
  const entries = Object.entries(value)
  if (entries.length === 0) {
    return <span className="font-mono text-[#6b6e73]">None</span>
  }

  return (
    <dl className="flex flex-col gap-2 rounded bg-[#f6f8fa] p-2">
      {entries.map(([key, nestedValue]) => (
        <div key={key} className="min-w-0">
          <dt className="text-[0.7rem] font-semibold uppercase tracking-wide text-[#6b6e73]">
            {formatMetricLabel(key)}
          </dt>
          <dd className="mt-0.5">
            <MetricValue value={nestedValue} />
          </dd>
        </div>
      ))}
    </dl>
  )
}

function isPlainObject(value: unknown): value is JsonObject {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function formatMetricLabel(key: string) {
  return key
    .replaceAll('_', ' ')
    .replace(/\b\w/g, (letter) => letter.toUpperCase())
}
