import { Link, useRouterState } from '@tanstack/react-router'

const normalizePath = (path: string) =>
  path === '/' ? '/' : path.replace(/\/+$/, '') || '/'

export function NotFoundPage() {
  const currentPath = useRouterState({
    select: (state) => normalizePath(state.location.pathname),
  })

  return (
    <section className="grid max-w-[40rem] gap-3">
      <h1 className="text-2xl font-semibold">Page not found</h1>
      <p>
        No route exists for <code>{currentPath}</code>.
      </p>
      <Link to="/" className="w-fit text-[#0f3f88] underline">
        Go to home
      </Link>
    </section>
  )
}
