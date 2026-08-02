# Outdegree Report: `ocaml_sample_functions3.ml`

## Summary

The source is a concatenation of five top-level OCaml definitions: `test`,
`to_ir`, `get_opcode`, `decode`, and `createToplevelWindow`. The counts below
use the corresponding sections in
`crates/sourcery-callgraph-dabble/ocaml_callnames.txt`. **Call occurrences**
are weighted outdegree; **listed targets** counts distinct call names in each
section.

| Definition | Call occurrences | Listed targets | Same-package calls | Role |
|---|---:|---:|---:|---|
| `test` | 1,065 | 67 | 1,013 (95.1%) | Test orchestration and filesystem synchronization |
| `to_ir` | 1,078 | 163 | 955 (88.6%) | x86 instruction to BIL/IR translation |
| `get_opcode` | 507 | 129 | 427 (84.2%) | Opcode parsing and operand decoding |
| `decode` | 362 | 103 | 334 (92.3%) | Byte-level x86 instruction decoding |
| `createToplevelWindow` | 869 | 338 | 610 (70.2%) | GTK UI construction and synchronization controls |

Combined, the sample has **3,881 weighted calls**. `createToplevelWindow` has
the broadest distinct call surface (338 targets), while `to_ir` has the highest
weighted outdegree (1,078).

## How The Definitions Are Used

- `test` is a top-level test driver. Its local helpers form the main call
  structure: `runtest` runs scenarios, `put` writes test filesystems, `sync`
  propagates changes, and `check`/`checkmissing` validate results. The most
  frequent calls are `put` (159), `sync` (83), and `check` (70).
- `test` also uses assertion helpers (`check_assert`, `assert_n_ris`,
  `assert_n_mov`) and move-test helpers (`sync_count_moves`, `testmoves`). It
  does not call the other sampled top-level definitions.
- `to_ir` is a recursive decoder/translator. It dispatches on x86 opcodes and
  builds BIL expressions using `op2e` (109), `Bil.Move` (80), `tmp` (66),
  `assn` (56), and `var` (56). Its recursive self-call appears once in the
  call report.
- `to_ir` relies on local helpers for operand conversion, flag computation,
  masking, extraction, and repeated vector operations. Its outgoing calls are
  spread across arithmetic, list processing, BIL constructors, and operand
  semantics rather than a small orchestration chain.
- `get_opcode` maps decoded opcode forms to IR constructors. Its largest
  repeated targets are `parse_modrm_vec` (36), `parse_modrm_addr` (27), and
  `disfailwith` (41), showing a parser dominated by operand-shape handling and
  invalid-opcode reporting.
- `decode` is a byte-stream dispatcher. It recursively handles prefixes and
  delegates instruction families to helpers such as `return` (26), `to_reg`
  (13), `decode` (11), `get_imm` (7), and `push`/`pop` (6 each).
- `createToplevelWindow` is a UI assembly and callback-registration root. It
  uses local helpers such as `grAdd` (39), `grSet` (19), `loadProfile` (3),
  `buildActionMenu` (2), and `buildExpertMenu` (1), alongside GTK widgets and
  synchronization/profile modules.

## Interpretation

Weighted outdegree measures implementation activity, not importance. `test`
and `createToplevelWindow` are orchestration roots, `get_opcode` and `decode`
are decoder roots, and `to_ir` is the largest translation root by repeated
call volume.

Source data: `ocaml_sample_functions3.ml` and
`crates/sourcery-callgraph-dabble/ocaml_callnames.txt`.
