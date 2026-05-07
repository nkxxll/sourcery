import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/diff-graph')({
  component: DiffGraph,
})

function DiffGraph() {
  return <div id="container" />
}
