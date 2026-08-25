# tests/SKILL.md  -  vyre-libs

Read `../../.internals/skills/testing/SKILL.md` first.

## Purpose

`vyre-libs` is Category-A library composition over vyre-ops
hardware-intrinsic primitives. Tests prove every public function
produces a valid Program that (a) validates, (b) round-trips through
wire format, (c) dispatches successfully on every linked dispatch-capable
backend, and (d) matches the CPU reference output byte-for-byte.

## Critical invariants

- Every public function returns a `Program` that passes `validate`.
- Every Program wraps its body in exactly one `Node::Region` with a
  generator name matching the fully-qualified module path.
- Round-trip through wire format: `from_wire(to_wire(program)) == program`.
- CPU reference output == every dispatch-capable backend output for
  every deterministic op.

## Adversarial surface

- **`tests/adversarial.rs`**  -  named binary for the skills contract; also
  run `f32_adversarial`, `op_boundaries`, `overflow_guards` (see module
  docs in `adversarial.rs`).
- Zero-sized buffers  -  validator must reject or produce zero-work
  `Program`, not panic.
- Extreme dimensions (matmul with `m = u32::MAX`)  -  structured error.
- Empty needle in `substring_search`  -  every position can match; output
  length = `haystack_len` (guarded; see `substring_search` source).

## Gap list (open improvements  -  see `findings.toml` for P0)

- `relu` is u32-oriented; i32/f32 element variants are gaps.
- FNV-1a in `hash::fnv1a32` is serial; parallel tree-reduce is a perf gap.
- Workgroup-level softmax / norm / attention scaling  -  see
  `FINDING-PRIM-1` in `findings.toml` (workgroup scan primitive).

## Cross-crate contracts

- Consumes `vyre::ir::*` (Program, Node, Expr, BufferDecl, DataType)
- Consumes `vyre_foundation::ir_inner::model::expr::GeneratorRef`
- Backend execution tests use `vyre-driver` registry capabilities only;
  concrete driver parity belongs to the owning driver crate.

## Bench targets

- `vyre_libs::math::matmul` throughput across 64/256/1024-dim
- `vyre_libs::crypto::fnv1a32` bytes/sec across 1 KB / 64 KB / 1 MB
- `vyre_libs::matching::substring_search` GB/s across needle lengths

## Fuzz targets

- Program generators for every public function  -  arbitrary valid
  inputs must produce a Program that validates + dispatches.

## Running

```bash
./cargo_full test -p vyre-libs
./cargo_full test -p vyre-libs --test integration
./cargo_full test -p vyre-libs --test property
./cargo_full test -p vyre-libs --test gap
./cargo_full test -p vyre-libs --test adversarial
./cargo_full test -p vyre-libs --test f32_adversarial --test op_boundaries --test overflow_guards
```

## Decision tables  -  picking a matching primitive

This is the lego-block reuse map for `vyre_libs::matching`. Pick the
top-most row whose constraints fit the workload  -  every later row is
strictly more capable but carries dispatch overhead the earlier
options avoid.

### Matching engine

| Engine                        | Pattern shape                | Behind feature flag | When to pick                                                |
|-------------------------------|------------------------------|---------------------|-------------------------------------------------------------|
| `substring_search`            | one literal needle           | `matching-substring`| <1 KB inputs or single-pattern hot path; no DFA build cost. |
| `aho_corasick`                | many literals (no regex)     | `matching-dfa`      | Many literals, simple shared-prefix DFA, classic AC walk.   |
| `cooperative_dfa_scan`        | many literals (subgroup-coop)| `matching-dfa`      | GPU dispatch where each subgroup advances one byte stream.  |
| `GpuLiteralSet`               | many literals + GPU + cache  | always-on           | The default secret-scanning path: precompiled DFA, wire-format cache.|
| `RulePipeline` / `mega_scan`  | regex (NFA, ≤1024 states)    | `matching-nfa`      | When literals don't suffice; supports anchors, classes.     |
| `compile_regex_set`           | regex set → `RulePipeline`   | `matching-regex`    | Caller has regex source strings rather than a literal set.  |

### Dispatch helpers

| Helper                          | Returns                  | Use when                                                    |
|---------------------------------|--------------------------|-------------------------------------------------------------|
| `pack_haystack_u32`             | `Vec<u8>`                | Packing a `&[u8]` haystack for any matcher's input buffer.  |
| `pack_u32_slice`                | `Vec<u8>`                | Packing an arbitrary `&[u32]` for storage upload.           |
| `haystack_len_u32`              | `Result<u32, _>`         | Plain `u32` cap check (max IR limit).                       |
| `scan_guard`                    | `Result<u32, _>`         | Both the `u32` cap **and** a configurable byte ceiling.     |
| `byte_scan_dispatch_config`     | `DispatchConfig`         | One workgroup per `workgroup_size[0]` haystack bytes.       |
| `candidate_start_dispatch_config` | `DispatchConfig`       | One workgroup per candidate start offset (NFA pipelines).   |
| `unpack_match_triples`          | `Vec<Match>`             | Decode the `(pid, start, end)` triple buffer back to typed. |

### Cache + persistence

| Helper                          | Use when                                                                |
|---------------------------------|-------------------------------------------------------------------------|
| `cached_load_or_compile`        | Persisting any `MatchEngineCache` to disk; one-line cache wiring.       |
| `engine_cache_path`             | Compute the on-disk path for a given engine + cache key.                |
| `MatchEngineCache::WIRE_MAGIC`  | Magic bytes per engine; defines the wire-format envelope.               |
| `Program::content_hash()`       | Deterministic 32-byte BLAKE3 of canonical wire bytes for cache keying.  |

### Region / span dedup

| Helper                          | Use when                                                                |
|---------------------------------|-------------------------------------------------------------------------|
| `dedup_regions_cpu`             | Owned `Vec<RegionTriple>`; you want a fresh deduped vector returned.    |
| `dedup_regions_inplace`         | You already own the `&mut Vec` and want zero-alloc compaction.          |
| `RegionTriple::new(pid,s,e)`    | Constructing the canonical span tuple from raw u32s.                    |

### Test fixtures (behind `feature = "test-fixtures"`)

| Fixture                            | Use when                                              |
|------------------------------------|-------------------------------------------------------|
| `AKIA_LITERAL` / `GHP_PREFIX`      | Need the canonical literal pair every test reuses.    |
| `MIXED_HAYSTACK`                   | Mixed-credential haystack at predictable offsets.     |
| `long_repeating_haystack()`        | 32× repetition of the mixed pattern; ~830 bytes.      |
| `canonical_literal_pair()`         | Pre-bundled `(patterns, haystack)` tuple.             |
| `overlapping_literal_pair()`       | NFA-vs-DFA overlap-policy stress fixture.             |
| `canonical_regex_set()`            | Regex frontend smoke fixture.                         |
| `realistic_detector_pattern_corpus()` | 200 production-shaped pattern bytestrings (no dups). |

The full list of public exports is enumerated in
`vyre_libs::matching::API_INDEX`; the `tests/api_index.rs` test
verifies every entry resolves to a real symbol.
