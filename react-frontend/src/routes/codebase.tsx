import { createFileRoute } from '@tanstack/react-router'

import { Button } from '#/components/ui/button'

export const Route = createFileRoute('/codebase')({
  component: CodeBase,
})

function CodeBase() {
  return (
    <div className="flex gap-2">
      <Button>Diff</Button>
      <Button>Version</Button>
    </div>
  )
}
