# vyre-frontend-rust maturity gate

Current maturity: **experimental nano-subset (crate v0.6.x)**.

This crate is the thin-on-semantics pipeline driver for a GPU-first Rust
compiler. The reusable lexer, parser, semantic analysis, and lowering live in
`vyre-libs::parsing::rust` (`lex`, `parse`, `sema`, `lower`); this crate only
orchestrates them and owns object emission, GPU dispatch, and the public API.

The implemented subset is **narrow but real and end-to-end verified against live
`rustc`** (accept/reject parity) and a reference interpreter (execution parity).
It is not a stub: `sema` (resolve + type inference + NLL borrow check) and
`lower` (to executable Vyre IR) are implemented and gated. The remaining
unwired stage is GPU-dispatched lexing, which fails loudly rather than silently
falling back.

## Scope boundary

`vyre-frontend-rust` owns Rust-source ingestion, pipeline orchestration,
GPU-reachable lexing evidence, and object/evidence emission. The language
algorithms (parsing, name resolution, type inference, borrow checking,
lowering) are substrate and live in `vyre-libs::parsing::rust`.

It does not own the Vyre platform release proof.

## Supported nano-subset

- Functions: `fn name(param: Type, ...) -> Type { body }`
- Statements: `let mut? name: Type = expr;`, `name = expr;` (assignment),
  `return expr?;`, `while cond { body }`, `for name in start..end { body }`,
  `if/else` (incl. `else if`), expression statements, nested block statements
- Types: `i32`, `bool`, `&T`, `&mut T`
- Expressions: integer/bool literals; arithmetic `+ - * / %`; comparisons
  `== != < > <= >=`; boolean `&& || !`; `if/else`; blocks; function calls
  (incl. forward references); `&expr` / `&mut expr`; `*expr`
- Integer literals: parsed as `u128`; literals exceeding `u128` are rejected
  (matching rustc's unconditional "integer literal is too large" hard error,
  which `--cap-lints allow` cannot suppress). Within-range over-`i32` literals
  are accepted and wrap, matching rustc's capped `overflowing_literals` lint.
- NO (out of subset, roadmap): generics, traits, impls, macros, modules,
  structs, enums, closures, async, const, static, named lifetimes, attributes,
  `loop`/`break`/`continue`, `match`, wider integer types
  (`u32`/`i64`/`u64`), float types.

## Verification status (gates)

| Gate | Evidence | Status |
|---|---|---|
| Lexer correctness | `tests/lexer_oracle.rs` — substrate lexer vs `rustc_lexer`, byte-level content agreement over a nano-subset corpus. | **Present** |
| Parser + verdict parity | `tests/rustc_differential.rs` — full pipeline (lex+parse+resolve+typeck+mutability+escape+conflicts) accept/reject verdict vs live `rustc --crate-type lib --edition 2021 --cap-lints allow`, over curated ACCEPT/REJECT, operator (`OPS_ACCEPT`/`OPS_REJECT`), and integer-literal (`LITERAL_CORPUS`) corpora. | **Present** |
| Semantic analysis (resolve + type inference) | `vyre-libs/tests/rust_sema_borrow_oracle.rs` + the differential above; `sema` is ~1k LOC implemented (NOT pending). | **Present** |
| Borrow check (NLL) | `vyre-libs/tests/rust_sema_borrow_oracle.rs` proves E0596/E0597/E0499/E0502 via CFG NLL dataflow match rustc accept/reject over generated straight-line, branch, reborrow, and coercion programs plus a curated corpus. `tests/{conflict,borrow,escape,differential_fuzz}.rs` exercise the same through the driver; `differential_fuzz.rs` is a 1024-case proptest vs live rustc. | **Present** |
| Lowering (→ Vyre IR) | `vyre-libs/tests/rust_lower_exec_oracle.rs` lowers the AST to a Vyre `Program`, runs it on the reference interpreter, and checks against two independent oracles incl. counted `while` loops and half-open `for start..end` range loops (u32 IR loop bounds with signed i32 source semantics). | **Present** |
| Reliability (hostile input) | `tests/adversarial_parse_depth.rs` — pathologically nested input (parens, `! * &mut`, nested `while`/`if` blocks, `&mut` type chains) fails closed with a typed `ParseError` instead of overflowing the native stack and aborting the process. `tests/proptest_robustness.rs` — 4096-case fuzz: pipeline never panics on arbitrary bytes/token soup. | **Present** |
| GPU lexing | `tests/gpu_lex.rs` must prove the GPU lexer path matches the CPU oracle byte-for-byte. GPU dispatch is unwired. | **Pending (roadmap)** |
| No silent fallback | The unwired GPU-lex stage fails loudly and actionably; locked by `tests/smoke.rs::compile_pipeline_rejects_unwired_gpu_lex_without_silent_cpu_path`. (sema and lowering are wired — only GPU lex is gated here.) | **Present** |

## Promotion / production criteria

TBD. This crate is not on the Vyre 0.4.2 release train. Promotion out of
experimental requires GPU-lex wiring with byte-for-byte CPU-oracle parity, plus
grammar widening (per `docs/RUST_COMPILER_BUILDOUT.md`, one construct per task,
each gated by an extension of the rustc differential).
