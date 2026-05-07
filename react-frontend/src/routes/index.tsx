import { createFileRoute } from '@tanstack/react-router'

import { LanguageTable } from '#/components/language-table'
import { useQuery } from '@tanstack/react-query'

export const Route = createFileRoute('/')({ component: Home })

function Home() {
  const { isPending, error, data } = useQuery({
    queryKey: ['repoData'],
    queryFn: () =>
      fetch('/api/codebases').then((res) => {
        res.json()
      }),
  })

  if (isPending) return <>Loading...</>
  if (error) return <>Error...{error}</>
  return (
    <>
      {JSON.stringify(data)}
      <LanguageTable rows={data} />
    </>
  )
}
