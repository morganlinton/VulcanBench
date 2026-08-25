# vyre-grammar-gen

[![status: alpha](https://img.shields.io/badge/status-alpha-orange.svg)](Cargo.toml)

## What it does

`vyre-grammar-gen` is the host-side (CPU) tool that compiles the C11 lexer
grammar into binary table blobs for the vyre GPU C parser pipeline.

It produces:

| Output | Default path | Consumed by |
|--------|-------------|-------------|
| Lexer DFA | `c11_lexer_dfa.bin` | `vyre-libs::parsing::c::lex` (GPU loads as ReadOnly storage buffer) |
| LR tables | `c11_lr_tables.bin` (only when `--lr-json` is passed) | LR consumers with concrete grammar tables |

Every blob uses the `SGGC` magic header (version 1, little-endian u32 arrays)
and carries a BLAKE3-128 integrity tag. See `src/wire.rs` for the full layout.

The library also exposes:

- `preprocess_c_host` - conservative C preprocessor (line splicing, comment
  stripping, `#if 0` removal, object-like macro expansion).
- `lex_c11_max_munch_kinds` - max-munch DFA lexer that emits C11 token kinds.
- `kinds_blake3` - BLAKE3 hash of a token-kind sequence (corpus goldens).
- `count_chunked_valid_tokens` - SIMD-friendly chunked token counter.
- `DfaBuilder`, `LrBuilder`, `validate_lr_table` - table construction helpers.
- `decode_dfa_from_bytes`, `decode_lr_from_bytes` - wire format decoders.

## Quick start

```bash
# Emit the full C11 lexer DFA blob
cargo run -p vyre-grammar-gen -- emit --out-dir ./rules/c11/

# Emit DFA + an explicit LR table from JSON
cargo run -p vyre-grammar-gen -- emit --out-dir ./rules/c11/ --lr-json ./lr-table.json

# Emit with JSON sidecar metadata
cargo run -p vyre-grammar-gen -- emit --out-dir /tmp/ --format json

# Emit a tiny smoke-test DFA (fast, no real C11 patterns)
cargo run -p vyre-grammar-gen -- emit --out-dir /tmp/ --smoke-lexer

# Hex-dump the lexer blob to stdout
cargo run -p vyre-grammar-gen -- dump-lexer
cargo run -p vyre-grammar-gen -- dump-lexer --smoke-lexer

# Hex-dump an LR blob
cargo run -p vyre-grammar-gen -- dump-lr --lr-json ./lr-table.json
```

Use the library directly in tests or in the build pipeline:

```rust
use vyre_grammar_gen::{build_c11_lexer_dfa, wire::PackedBlob};

let dfa = build_c11_lexer_dfa();
let blob = PackedBlob::from_dfa(&dfa);
std::fs::write("c11_lexer_dfa.bin", &blob.bytes)?;
```

## When to use / When not

**Use when:**

- You need to regenerate the binary DFA or LR table blobs that `vyre-libs`
  uploads to the GPU.
- You are writing tests that require a known-good C11 lexer on the host.
- You need a host-side C preprocessor pass before feeding source to the
  GPU parser pipeline.

**Do not use when:**

- You need a full C11 parser tree - use `vyre-libs::parsing::c::parse`
  (C11 syntax-tree construction is handled there, not here).
- You need function-like macro expansion - `preprocess_c_host` expands only
  object-like macros; function-like macros are a documented non-feature.
- You need to generate grammar tables at runtime on the GPU - this crate is
  host-only and requires `std`.

## Compared to alternatives

| Alternative | Key difference |
|-------------|---------------|
| `lalrpop` / `pest` | General-purpose parser generators for host use; do not emit GPU-compatible binary blobs in the `SGGC` wire format. |
| `logos` | Fast host Rust lexer generator; not designed to produce portable binary transition tables for upload to GPU ReadOnly storage. |
| Hand-written DFA | Requires manual maintenance on every C11 change; `vyre-grammar-gen` drives the DFA from the `C11_PATTERNS` table so all patterns live in one place. |

## How it fits in Santh

`vyre-grammar-gen` is a build-time / test-time tool in the `libs/shared/`
layer. Its outputs feed `vyre-libs::parsing`:

```
vyre-grammar-gen (CPU, build time)
    |-- emits c11_lexer_dfa.bin
    |-- emits c11_lr_tables.bin (optional)
         |
         v
vyre-libs::parsing::c::{lex, parse}  (CPU host path + GPU vyre Program)
```

It depends only on `regex-automata`, `blake3`, `serde`, and `serde_json` -
no Santh-internal crates - keeping the dependency cone narrow.

See also:

- `../docs/parsing-and-frontends.md` - C parser frontend architecture and the
  "frontends on CPU, vyre Programs on GPU" partition.
- `../vyre-libs/src/parsing/` - consumer implementation (feature `c-parser`).

## Contributing

Follow the project-wide coding standards in `CLAUDE.md`. All public symbols
must be documented and tested. To add a new token kind:

1. Add the `(token_id, pattern)` pair to `C11_PATTERNS` in `src/c11_lexer.rs`.
2. Export the new constant from `c11_lexer` (and re-export from `lib.rs` if
   needed).
3. Pin the token id in `tests/gap.rs` to prevent silent renumbering.
4. Run `cargo +nightly test -p vyre-grammar-gen` and ensure all tests pass.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE)
at your option.
