# Outdegree Report: `ocaml_sample_functions4.ml`

## Scope and Interpretation

The sample contains ten top-level OCaml definitions. Outdegree is interpreted
as outgoing call activity: weighted outdegree counts repeated call occurrences,
while unique outdegree counts distinct called operations. Higher-order
arguments such as `~f` and callbacks are important because they pass behavior
through a wrapper, even when the wrapper itself has only one visible callee.

| Function | Main outgoing calls | Functionality | Outdegree profile |
|---|---|---|---|
| `count` | `fold`, `f`, monadic `>>|` | Counts elements for which an effectful predicate returns `true` | Higher-order fold; the visible calls are small, but predicate work is delegated and repeated per element |
| `set` | underlying `set`, `is_compressed`, `assert` | Performs a union-find update and checks path compression | Mutation-plus-invariant wrapper; calls are sequential and diagnostic validation follows the update |
| `to_p_type` | `Z.to_int` | Maps an integer program-header type to a variant, preserving unknown values as `PT_OTHER` | Minimal conversion function; pattern matching supplies most behavior without calls |
| `ofRootConncheck` | `withConncheck`, `findByRoot` | Finds a connection by root inside connection-checking context | Two-stage context adapter; the wrapper controls execution while the lookup supplies the result |
| `pp_typ` | `Type.string_of_typ`, `pp_print_string` | Prints a type using its canonical string representation | Formatting adapter; one call computes the representation and one emits it |
| `add_buffer` | `resize`, `Bigstring.blito` | Appends one big buffer to another, resizing when needed | Stateful buffer operation; conditional capacity management precedes bulk copying and position update |
| `rem_files` | `List.iter`, `rem_api` | Removes API files for every descriptor | Higher-order iterator wrapper; the callback is the repeated outgoing operation |
| `exists` | `existsi` | Tests for an element in an indexed table while hiding the index argument | Delegation wrapper; optional range arguments are forwarded unchanged |
| `reset` | `with_cache`, `get_date_cache`, `reset_cache` | Resets a date cache through the cache access protocol | Effect-management wrapper; several callbacks configure read/write behavior, with reset work delegated to `reset_cache` |
| `hprop` | `float` twice | Computes a weighted property from record fields and converts two integers to floats | Small arithmetic helper; two repeated calls to the same conversion function dominate its outgoing activity |

## Common Patterns

- **Higher-order delegation:** `count`, `rem_files`, and `exists` rely on
  library combinators (`fold`, `List.iter`, and `existsi`) and pass behavior as
  function arguments. Their direct unique outdegree is low, but their effective
  work can scale with the collection being traversed.
- **Thin wrappers:** `ofRootConncheck`, `pp_typ`, and `exists` primarily adapt
  argument shape or execution context. Their outdegree is a useful indicator
  of delegation, not of algorithmic complexity.
- **State and effect control:** `set`, `add_buffer`, and `reset` combine an
  operation with state management or invariant handling. `set` checks a
  postcondition, `add_buffer` ensures capacity before mutation, and `reset`
  supplies callbacks to a cache protocol.
- **Small deterministic transformations:** `to_p_type` and `hprop` do most of
  their work through pattern matching or arithmetic. Their low outdegree does
  not mean identical behavior: `to_p_type` is a total enum conversion, while
  `hprop` is numeric computation.
- **Repeated callees:** `count` invokes its predicate through the fold, and
  `rem_files` invokes `rem_api` per descriptor. `hprop` visibly calls `float`
  twice. These are the clearest cases where weighted outdegree differs from
  unique outdegree.

## Functional Grouping

The ten definitions can be grouped by functionality:

1. **Collection traversal and search:** `count`, `rem_files`, and `exists`.
   Each abstracts iteration/search over a collection or table. `count` and
   `rem_files` perform work for many elements, while `exists` can terminate on
   the first match and forwards optional bounds.
2. **Context and API adaptation:** `ofRootConncheck`, `pp_typ`, and `exists`.
   These expose a convenient local interface over a context manager, formatter,
   or indexed implementation. `exists` belongs in both this group and the
   traversal group because it is simultaneously a delegating adapter and a
   search operation.
3. **Mutable state and effect protocols:** `set`, `add_buffer`, and `reset`.
   They all coordinate an externally visible state change with supporting
   calls. Only `set` enforces an explicit assertion; `add_buffer` and `reset`
   enforce correctness through capacity and cache protocols.
4. **Value conversion and calculation:** `to_p_type` and `hprop`. Both are
   compact data-level functions with little call-graph fan-out, but one maps
   tagged numeric input to a variant and the other calculates a floating-point
   score.

## Overall Finding

OCaml outdegree in this sample is generally low because abstraction is carried
by higher-order arguments, pattern matching, and library combinators rather
than long call chains. The most useful distinction is between direct fan-out
and delegated repeated work: `exists` may have one direct callee while
`count` and `rem_files` perform collection-wide work through callbacks. The
functions can be grouped strongly by behavior, especially traversal,
adaptation, mutable/effectful state, and value-level computation; their
outdegree values should be read in the context of those roles rather than as a
standalone complexity ranking.

Source: `ocaml_sample_functions4.ml`.
