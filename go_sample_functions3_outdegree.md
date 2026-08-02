# Outdegree Report: `go_sample_functions3.go`

## Summary

`go_callnames.txt` contains one call-analysis section for each of the five
top-level definitions in the sample. The number in each section heading is a
**weighted outdegree**: the total number of call occurrences, including
repeated calls. The count of indented entries is a useful proxy for distinct
listed call targets, although the report keeps some syntactically different
expressions separate.

| Top-level definition | Call occurrences | Listed targets | Same-package calls | Main outgoing use |
|---|---:|---:|---:|---|
| `(*Terminal).Loop` | 856 | 264 | 361 (42%) | Event-loop orchestration; `req` (84), `len` (56), preview/UI methods |
| `(*Field).setupValuerAndSetter` | 417 | 74 | 65 (16%) | Reflection and value conversion; `field.ReflectValueOf` (93 and 88 report entries) |
| `ConvertGeminiResponseToOpenAIResponses` | 378 | 67 | 87 (23%) | JSON response transformation; `sjson.SetBytes` (121) |
| `TestHandler` | 339 | 39 | 22 (6%) | Integration-test setup and HTTP/assertion activity; `require.NoError` (72) |
| `ConvertClaudeResponseToOpenAIResponses` | 334 | 66 | 73 (22%) | JSON response transformation; `sjson.SetBytes` (94) |

The sample has **2,324 weighted call occurrences**. `Loop` contributes 36.8%
of them and is the clear outdegree hub. The other four functions are closer in
weighted size, but `setupValuerAndSetter` has the highest concentration in a
small number of reflection operations.

## How The Definitions Are Used

- `(*Terminal).Loop` is a large coordinator. Its calls are distributed across
  input handling, event dispatch, rendering, preview processes, timers,
  selection/navigation, and terminal lifecycle management. Local closures such
  as `req`, `doAction`, `refreshPreview`, and scrolling helpers account for
  substantial internal activity.
- `(*Field).setupValuerAndSetter` is an initialization/configuration routine.
  Its outdegree is dominated by reflection (`ReflectValueOf`, `reflect.Indirect`,
  `reflect.ValueOf`, and value inspection), plus serializer getter/setter setup.
- The Gemini and Claude conversion functions have nearly the same role and
  profile: parse streamed JSON, inspect fields with `gjson`, maintain conversion
  state, emit events, and construct output with `sjson`. Their similar
  outdegrees indicate parallel implementations rather than a call hierarchy.
- `TestHandler` is a test root. Its calls are mostly test scaffolding,
  HTTP requests, database inspection, and assertions; `require.NoError` and
  `assert.Equal` dominate repeated calls.

## Structural Finding

There are no calls between the five sampled top-level definitions in the
provided call-name report. They are independent graph roots selected from
different packages/contexts. The only notable same-package entry in the
sample’s test root is `StartHandler`; the conversion roots call shared helpers
such as `emitEvent` and `nextSeq`, while `Loop` relies heavily on methods and
local closures.

**Interpretation:** weighted outdegree measures implementation activity, not
necessarily API importance. `Loop` is broad and orchestration-heavy; the two
converters are narrower but serialization-heavy; `setupValuerAndSetter` is
reflection-heavy; and `TestHandler` is assertion/test-I/O-heavy.

Source data: `crates/sourcery-callgraph-dabble/go_callnames.txt`.
