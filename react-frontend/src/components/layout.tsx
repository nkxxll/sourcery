import { Link, useRouterState } from '@tanstack/react-router'
import type { ReactNode } from 'react'

import { cn } from '#/lib/utils'

const navLinks = [
  { path: '/', label: 'Home' },
] as const

const normalizePath = (path: string) =>
  path === '/' ? '/' : path.replace(/\/+$/, '') || '/'

export function AppLayout({ children }: { children: ReactNode }) {
  const currentPath = useRouterState({
    select: (state) => normalizePath(state.location.pathname),
  })

  return (
    <div className="min-h-screen">
      <header className="sticky top-0 z-10 border-b border-[#d0d7de] bg-white px-4 py-3">
        <nav aria-label="Primary navigation">
          {navLinks.map((link) => (
            <Link
              key={link.path}
              to={link.path}
              className={cn(
                'mr-4 font-medium no-underline',
                currentPath === link.path ? 'text-[#0f3f88]' : 'text-[#4d4f53]',
              )}
            >
              {link.label}
            </Link>
          ))}
        </nav>
      </header>
      <main className="p-4">{children}</main>
    </div>
  )
}
