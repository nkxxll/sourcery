import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/codebase/$id/stats')({
  component: RouteComponent,
})

function RouteComponent() {
  return <div>Hello "/codebase/$id/stats"!</div>
}
