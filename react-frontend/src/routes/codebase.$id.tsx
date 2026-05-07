import { Link, createFileRoute } from '@tanstack/react-router'

import { Button } from '#/components/ui/button'

export const Route = createFileRoute('/codebase/$id')({
  component: CodebasePage,
})

function CodebasePage() {
  const { id } = Route.useParams()

  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm text-[#4d4f53]">Codebase {id}</p>
      <div className="flex gap-2">
        <Link to="/codebase/$id/diff" params={{ id }}>
          <Button>Diff timeline</Button>
        </Link>
        <Link to="/codebase/$id/stats" params={{ id }}>
          <Button>Codebase stats</Button>
        </Link>
      </div>
    </div>
  )
}
