# vyre-frontend-rust Design

GPU-first Rust compiler frontend for Vyre.

## Layer Position

- **`vyre-libs::parsing::rust`**: reusable substrate (Tier 3). Lexer (`lex`),
  parser (`parse`), semantic analysis (`sema`: resolution, type inference,
  borrow checking), and lowering (`lower`: AST -> Vyre IR). Mirrors
  `vyre-libs::parsing::c`.
- **This crate** (`vyre-frontend-rust`): thin-on-semantics driver. Owns
  pipeline orchestration, object/evidence emission, GPU backend selection, and
  the public API. It calls the substrate; it does not implement language
  semantics itself.

The substrate cannot depend on the driver. Keeping semantics and lowering in
the substrate (as the C frontend does) keeps typed Rust analysis reusable by
any consumer and lets `vyre-frontend-core` be extracted symmetrically later.

## Conventions (shared with `vyre-frontend-c`)

We follow the C frontend's conventions to prevent divergence:

| Convention | C frontend | Rust frontend |
|---|---|---|
| API module | `api/mod.rs` + `api/entrypoints.rs` | `api/mod.rs` + `api/entrypoints.rs` |
| Pipeline stages | One duty per file under `pipeline/` | `pipeline/lexer_dispatch.rs`, etc. |
| Pipeline orchestrator | `pipeline.rs` wires stages | `pipeline.rs` wires stages |
| Semantics + lowering | `vyre-libs::parsing::c::{sema,lower}` | `vyre-libs::parsing::rust::{sema,lower}` |
| Error messages | `"description. Fix: suggestion."` | matched in `RustFrontendError` |
| Object format | `VYRECOB2` sections | stub; must converge before v0.1.0 |
| Oracle/parity | `api/parity.rs` with `ParityFact` | `tests/oracle_support` + `tests/lexer_oracle.rs`; `rustc_lexer` is a dev-dependency |

## Extraction Points

When a 3rd language frontend (Go, Python, or a new one) reaches pipeline
parity, extract `vyre-frontend-core` containing:

1. **Pipeline orchestration** - `compile_unit()` stage sequencing
2. **Lexer dispatch framework** - GPU probe -> plan -> fallback
3. **Object writer** - VYRECOB2 section builder
4. **Parity infrastructure** - `ParityFact`, `ParityFinding`, release gating
5. **Backend selection** - CUDA vs WGPU dispatch

Marked in code with `// TODO(vyre-frontend-core): ...`.

## What Is Language-Specific (lives in `vyre-libs::parsing::rust`, never in core)

- Grammar / parser rules (`lex`, `parse`)
- Semantic analysis: name resolution, type system, borrow check (`sema`)
- Lowering to Vyre IR (`lower`)
- ABI layout rules

## Nano-Subset

The v0.1.0 nano-subset supports:

- Functions: `fn name(params) -> ret { body }`
- Let bindings: `let x: i32 = expr;`, `let mut x: i32 = expr;`
- If/else: `if cond { then } else { else }`
- Return: `return expr;`
- Types: `i32`, `bool`, `&T`, `&mut T`
- Expressions: literals, identifiers, binary ops (`+ - * / < ==`), unary (`! - * &`)

Anything outside this subset is rejected at parse time.

## Status

`sema` (resolve + typeck + borrow check) and `lower` are implemented for the
nano-subset. The full path lex -> parse -> resolve -> typeck -> borrow -> lower
produces an executable Vyre `Program`: `compile_unit` with `lower: true` runs on
the reference interpreter and matches both an independent AST interpreter and
real rustc compile+run (see `vyre-libs/tests/rust_lower_exec_oracle.rs` and
`tests/lower_exec.rs`). Constructs outside the wired subset return a loud,
actionable error rather than a fake success or a miscompiled Program.
