# React Frontend Improvement Notes

This document summarizes improvement opportunities found in `react-frontend`. The app is small enough to keep changes incremental, but a few route files are already carrying data access, transformation logic, D3 rendering, and page markup at the same time.

## Current Shape

- Framework: TanStack Start, TanStack Router, React Query, React 19, Vite, Tailwind CSS.
- Main route directory: `src/routes`.
- Shared components: `src/components`.
- Shared helpers and models: `src/lib`.
- Existing reusable UI is limited to `Button`, table primitives, `AppLayout`, and `StatsPanel`.

## Highest Value Improvements

1. Extract API access and query options from route components.
2. Move shared domain types into `src/lib/models.ts` or feature-specific model files.
3. Split large visualization routes into page, data transform, and rendering modules.
4. Introduce reusable page primitives for loading, error, headers, details, and action links.
5. Replace imperative navigation with TanStack Router links or navigation helpers.
6. Standardize formatting, metric handling, and color tokens.
7. Add tests for data transformations before changing the visualizations.

## Refactor Opportunities

### API and React Query

Several route files repeat the same `fetch`, `res.ok`, and `throw new Error(...)` pattern. Examples include:

- `src/routes/index.tsx`
- `src/routes/codebase.$id.tsx`
- `src/routes/version.$versionId.tsx`
- `src/routes/file.$fileID.tsx`
- `src/routes/function.$functionID.tsx`
- `src/routes/treemap.$versionID.$kind.tsx`
- `src/routes/callgraph.$versionID.tsx`

Recommended extraction:

- Add `src/lib/api.ts` with a small typed request helper.
- Add feature query helpers such as `codebaseQueryOptions(id)`, `versionQueryOptions(versionId)`, and `fileQueryOptions(fileId)`.
- Keep React Query calls in route components, but move endpoint details out of JSX files.

Example target shape:

```ts
export async function apiGet<T>(path: string): Promise<T> {
  const res = await fetch(path)
  if (!res.ok) {
    throw new Error(`Request failed: ${path} (${res.status})`)
  }
  return res.json() as Promise<T>
}
```

This makes errors consistent and reduces copy-paste when adding endpoints.

### Shared Domain Models

`src/lib/models.ts` currently only contains `Codebase`, `CodebaseList`, and `CodebasesByLanguage`. Many route-local types are reusable domain models:

- `Version` appears in `version.$versionId.tsx`, `treemap.$versionID.$kind.tsx`, and `callgraph.$versionID.tsx`.
- `VersionFunction` appears in `function.$functionID.tsx` and `callgraph.$versionID.tsx` with related fields.
- File state and metric shapes are local to route files but are useful across detail pages and visualizations.
- `GithubIssueTimelineEvent`, `CodebaseMetricsVersion`, and metric records can be named and reused.

Recommended extraction:

- Keep generic backend response models in `src/lib/models.ts` if the file stays small.
- If it grows, split by domain: `src/lib/models/codebase.ts`, `src/lib/models/version.ts`, `src/lib/models/metrics.ts`.
- Avoid redefining the same `Version` shape per route.

### Page State Components

Loading and error UI is repeated in multiple files with slightly different wording and classes.

Recommended reusable components:

- `LoadingState` for `Loading version...`, `Loading file...`, and similar messages.
- `ErrorState` for unknown error handling and consistent visual treatment.
- `EmptyState` for `No metrics available yet`, `No functions available`, and similar states.

This also creates a single place to improve accessibility with `role="status"` and `role="alert"`.

### Header and Detail Layouts

The following files repeat a similar card header pattern:

- `src/routes/codebase.$id.tsx`
- `src/routes/version.$versionId.tsx`
- `src/routes/file.$fileID.tsx`
- `src/routes/function.$functionID.tsx`
- `src/routes/treemap.$versionID.$kind.tsx`
- `src/routes/callgraph.$versionID.tsx`

Recommended reusable components:

- `PageHeader` with eyebrow, title, actions, and children.
- `DetailGrid` for `dl` layout.
- `DetailItem` for label/value pairs.
- `PageActionLink` for consistent TanStack Router link styling.

This would remove duplicated Tailwind class strings and make pages easier to scan.

### Metrics Utilities

Metric-related helpers are spread across routes and components:

- `toMetricsRecord`, `toNumber`, and `withDerivedLocMetrics` in `codebase.$id.tsx`.
- `formatMetricLabel` in `stats-panel.tsx`.
- `flattenNumericMetrics`, `metricValue`, and `formatMetric` in `treemap.$versionID.$kind.tsx`.
- `formatHalsteadValue` and Halstead labels in `file.$fileID.tsx`.

Recommended extraction:

- Add `src/lib/metrics.ts` for numeric parsing, label formatting, flattening nested numeric metrics, and derived LOC metrics.
- Export Halstead metric labels from one place.
- Use the same number formatting rules across tables, detail cards, and visualizations.

This is a good first refactor because it is mostly pure logic and can be covered with Vitest.

### D3 Visualizations

The largest files are visualization-heavy:

- `src/routes/codebase.$id.tsx` is 700+ lines and includes metric chart logic plus table sorting.
- `src/routes/callgraph.$versionID.tsx` is 800+ lines and includes graph building, controls, search, and force rendering.
- `src/routes/treemap.$versionID.$kind.tsx` includes page logic, treemap transformations, metric normalization, and D3 rendering.

Recommended extraction:

- Move pure data builders into testable modules:
  - `buildCallgraph`, `filterGraph`, `searchNodes`.
  - `buildTree`, `mapFileItems`, `mapFunctionItems`, `getMetricOptions`.
  - issue timeline aggregation and metric series creation.
- Move D3 hooks into feature components:
  - `src/features/callgraph/use-force-graph.ts`.
  - `src/features/treemap/use-treemap.ts`.
  - `src/features/codebase-metrics/use-metrics-chart.ts`.
- Keep route files focused on params, queries, and page composition.

This will reduce route file size and make regressions easier to test.

### Navigation

Some visualizations use `window.location.href` for navigation:

- `CodebaseMetricsTable` in `src/routes/codebase.$id.tsx`.
- Treemap click handlers in `src/routes/treemap.$versionID.$kind.tsx`.

Recommended improvement:

- Prefer TanStack Router navigation with `useNavigate` or render clickable content as `Link` when possible.
- This preserves SPA navigation, preloading behavior, and route typing.

### Styling Standards

The UI uses many repeated hex colors such as `#0f3f88`, `#6b6e73`, `#d0d7de`, `#f6f8fa`, and `#24292f`.

Recommended improvement:

- Define these as Tailwind theme tokens or CSS variables.
- Use semantic names such as `border-subtle`, `text-muted`, `text-brand`, and `surface-muted`.
- Avoid one-off colors like the red table border in `LanguageTable` unless intentionally part of the design.

This improves consistency and makes future design changes cheaper.

### Table and List Patterns

`LanguageTable` currently uses TanStack Table for a single-row language grid. That adds complexity without much benefit.

Recommended options:

- Replace it with a simpler responsive CSS grid grouped by language.
- Keep TanStack Table for data that needs sorting, filtering, pagination, or column state.

The metrics table in `codebase.$id.tsx` does need sorting, so it is a better candidate for a reusable table component or TanStack Table.

## React Coding Practice Improvements

### Keep Route Components Thin

Route components should primarily handle:

- Reading route params.
- Calling query hooks.
- Handling loading, error, and empty states.
- Composing feature components.

Move pure transforms and lower-level UI details out once a route file starts hiding the page flow.

### Prefer Pure Helpers for Data Transformation

Functions such as `buildCallgraph`, `buildTree`, metric flattening, and issue aggregation are good pure-helper candidates. They should not depend on React state, refs, or DOM APIs.

Benefits:

- Easier unit tests.
- Smaller React components.
- Less risk when changing D3 rendering.

### Use `useMemo` Selectively

There are valid `useMemo` uses around expensive graph and chart transformations. Avoid adding `useMemo` for trivial values unless profiling shows a need or the value is required for dependency stability.

Good current candidates:

- Graph construction.
- Treemap tree construction.
- Metric series creation.

Less valuable candidates:

- Simple string formatting.
- Small object or array creation that is not passed into expensive work.

### Avoid Side Effects in Render Flow

The D3 hooks already isolate DOM writes inside effects, which is good. Continue keeping imperative SVG code out of JSX render logic.

When possible, use cleanup functions to stop simulations, disconnect observers, remove listeners, and reset refs. `useForceGraph` already does this for the force simulation.

### Prefer Router Navigation Over Browser Navigation

Avoid `window.location.href` inside React event handlers. Use route links or TanStack Router navigation APIs so the app remains a client-side routed app.

### Standardize Accessibility

Add consistent accessibility treatment for shared components:

- Loading UI should use `role="status"` where appropriate.
- Error UI should use `role="alert"`.
- SVG charts should keep meaningful `role="img"` and `aria-label` values.
- Interactive SVG nodes should have keyboard-accessible alternatives if they remain primary navigation controls.
- Table headers that sort should use real buttons or `aria-sort`.

### Avoid Index Keys When Stable Keys Exist

`StatsPanel` uses array indexes for nested arrays. That is acceptable for static display, but stable identifiers are better when data can change or be reordered. If metric rows have stable keys, use them.

### Keep Generated Files Out of Manual Edits

`src/routeTree.gen.ts` is generated. Do not hand-edit it. Let the router plugin regenerate it after route changes.

## Suggested Folder Direction

The current structure is fine for a small app. If the visualizations continue growing, use feature folders without over-abstracting:

```txt
src/
  components/
    ui/
    page-header.tsx
    page-state.tsx
    detail-grid.tsx
  features/
    callgraph/
      callgraph-panel.tsx
      callgraph-model.ts
      use-force-graph.ts
    codebase-metrics/
      metrics-chart.tsx
      metrics-table.tsx
      metrics-series.ts
    treemap/
      treemap-panel.tsx
      treemap-model.ts
      use-treemap.ts
  lib/
    api.ts
    github.ts
    metrics.ts
    models.ts
    utils.ts
  routes/
```

Do not create this structure all at once. Extract files only when a route or helper is being actively changed.

## Testing Priorities

Add tests around pure logic first. These tests are cheap and catch the riskiest regressions.

Recommended initial tests:

- `buildCallgraph` resolves internal calls, external calls, duplicate calls, and degree counts.
- `filterGraph` hides external nodes and links correctly.
- `buildTree` creates expected nested paths and skips zero-value metrics.
- `flattenNumericMetrics` flattens nested numeric values and ignores non-numeric values.
- `withDerivedLocMetrics` derives bracket-inclusive LOC metrics correctly.
- `buildGithubPermalink` supports HTTPS and SSH repository URLs.

After that, add component tests for shared states and page primitives.

## Incremental Refactor Plan

1. Extract `apiGet` and replace repeated fetch blocks route by route.
2. Move duplicated `Version`, function, file, and metrics types into shared model files.
3. Extract `LoadingState`, `ErrorState`, `PageHeader`, and `DetailItem`.
4. Extract `src/lib/metrics.ts` and add Vitest coverage.
5. Extract callgraph pure model functions and test them.
6. Extract treemap pure model functions and test them.
7. Split `codebase.$id.tsx` into metrics chart and metrics table feature components.
8. Replace `window.location.href` navigation with router-aware navigation.
9. Introduce color tokens or CSS variables for repeated design values.
10. Revisit `LanguageTable` and replace it with a simpler responsive layout if sorting/table features are not needed.

## Things To Avoid

- Do not perform a large folder restructure before tests exist.
- Do not abstract every Tailwind class into a component; focus on repeated page structures first.
- Do not move route-specific behavior into global utilities unless at least two places need it.
- Do not hand-edit `routeTree.gen.ts`.
- Do not add backward-compatibility layers unless persisted data or external API consumers require them.
