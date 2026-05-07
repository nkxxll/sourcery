import { createFileRoute } from '@tanstack/react-router'

import { LanguageTable } from '#/components/language-table'

export const Route = createFileRoute('/')({ component: Home })

const rows = {
  golang: ['go-yaml', 'echo', 'bubbletea'],
  ocaml: ['ocaml-yaml', 'dream'],
  elixir: ['phoenix', 'jason'],
  c: ['cjson', 'raylib'],
} satisfies Record<string, string[]>

function Home() {
  return <LanguageTable rows={rows} />
}
