import { createFileRoute } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { LanguageTable } from '#/components/language-table'
import type { CodebasesByLanguage } from '#/lib/models'

export const Route = createFileRoute('/')({ component: Home })

function Home() {
  const { isPending, error, data } = useQuery({
    queryKey: ['repoData'],
    queryFn: async () => {
      const res = await fetch('/api/codebases')
      if (!res.ok) {
        throw new Error(`Failed to fetch codebases (${res.status})`)
      }
      return res.json() as Promise<CodebasesByLanguage>
    },
  })

  if (isPending) return <>Loading...</>
  if (error) {
    const message = error instanceof Error ? error.message : 'Unknown error'
    return <>Error... {message}</>
  }
  return <LanguageTable rows={data} />
}
