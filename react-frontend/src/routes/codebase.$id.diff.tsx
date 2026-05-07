import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/codebase/$id/diff')({
  component: CodebaseDiffTimelinePage,
})

function CodebaseDiffTimelinePage() {
  const { id } = Route.useParams()

  return <div id="container">Diff timeline for codebase {id}</div>
}
