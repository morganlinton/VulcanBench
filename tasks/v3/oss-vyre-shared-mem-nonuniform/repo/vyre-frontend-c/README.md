# vyre-frontend-c

GPU-first C parser and compile-evidence driver for Vyre.

Status: beta / active development. This crate and the `vyrec` CLI are not the
`0.4.2` Vyre release proof, are not production-ready C compiler components, and
do not currently claim clang parity. They are the in-repo compiler-front-end
consumer used to drive GPU-resident parsing, preprocessing, semantic evidence,
and object-evidence design forward. The exact promotion criteria live in
[`MATURITY.md`](MATURITY.md).

`vyre-frontend-c` takes C source through the `vyre-libs` parsing pipeline,
emits parser/AST/semantic-readiness evidence, and emits Linux ET_REL `.o`
payloads for the supported compile-only surface as that surface comes online.
Those payloads embed the compiled Vyre program as a `VYRECOB2` v3 section. It
is the library behind the beta `vyrec` binary at `tools/vyrec/`.

```text
C source
  → lex → digraph rewrite → opt_conditional_mask
  → macro expansion (table passthrough)
  → bracket_match (paren + brace)
  → function shapes → call sites → ABI layout
  → ast_shunting_yard → AST/semantic evidence
  → supported compile-only payload → Linux ET_REL .o  (with .vyrecob2.* section)
```

The promotion gate lives in [`MATURITY.md`](MATURITY.md); archived execution
plans are evidence only.

## Invariants

1. **Single-TU entrypoint per run.** `pipeline::compile_unit` takes
   one translation unit and emits one object file. Multi-TU is the
   linker's job (driver-level, not library-level).
2. **Every stage is GPU-reachable.** Lex, bracket match, and
   statement-bounds extraction all run through vyre Programs that the
   backend dispatches; there is no host-only fallback for the hot
   stages. CPU-only host helpers exist strictly for bootstrap and
   debugging.
3. **Emit format is ELF ET_REL with a `.vyrecob2.*` payload section.**
   The payload is the wire-encoded vyre Program + metadata. Consumers
  link normally with `cc -nostdlib`, then a small `_start` entry shim
   surfaces the GPU entry point.
4. **Bytes are packed little-endian, 4-byte aligned.** Haystack
   packing (`pipeline::pack`) is deterministic and reversible; two
   packs of the same bytes produce byte-identical buffers.
5. **No ABI drift without a VYRECOB version bump.** The payload
   section name encodes the wire version; old tooling sees a new
   section name and refuses to load it rather than misinterpret.

## Boundaries

`vyre-frontend-c` owns:

- The C-source → vyre IR pipeline (`pipeline`).
- Haystack byte packing / statement-bounds extraction.
- Minimal ELF64 relocation generation (`elf_linux`).
- Translation-unit compilation and the in-process lex DFA cache.
- The `api` surface the `vyrec` CLI consumes.

`vyre-frontend-c` does NOT own:

- The C grammar itself: that lives in `vyre-libs/src/parsing/c/`
  and the grammar is shared with every C-consuming crate.
- GPU backend implementation details: those stay behind `vyre-driver`.
- Linking: the CLI (`tools/vyrec`) drives `cc -nostdlib`; the
  library emits the `.o` and stops.
- Runtime concerns (async I/O, pipeline-cache policy, megakernel
  orchestration): those are `vyre-runtime`.

## Parser contract topics

Beta parser evidence must prove more than "some C file compiled". The frontend
contract covers C syntax recognition, AST fidelity, actionable diagnostics,
byte-span/location preservation, preprocessor behavior for includes and macros,
GNU extension handling, and honest unsupported-feature diagnostics. Full C
compiler lowering and clang parity are not claimed for the Vyre `0.4.2`
platform release; silent acceptance of unsupported syntax is still a frontend
bug.

## Three worked examples

### 1. Compile a single TU to an object file

```rust
use vyre_frontend_c::pipeline;

fn compile_hello(src: &str, out_path: &std::path::Path) -> std::io::Result<()> {
    let object = pipeline::compile_unit(src)?;
    std::fs::write(out_path, object.bytes())?;
    Ok(())
}
```

### 2. Pack a C-source byte buffer for GPU dispatch

```rust
use vyre_frontend_c::pipeline::pack_haystack;

fn to_gpu_buffer(src: &[u8]) -> Vec<u8> {
    pack_haystack(src).bytes
}
```

### 3. Extract statement bounds from pre-lexed tokens

```rust
use vyre_frontend_c::pipeline::{compile_unit, statement_bounds};

fn stmt_ranges(src: &str) -> std::io::Result<Vec<std::ops::Range<usize>>> {
    let tu = compile_unit(src)?;
    Ok(statement_bounds(&tu))
}
```

## Extension guide: adding a compiler pass

1. Decide whether the pass is host-only (bootstrap/debug) or must
   run on GPU (hot path). Host-only passes live in `pipeline/` as
   ordinary Rust functions; GPU passes emit a vyre `Program` that
   `vyre-driver` dispatches.
2. For a GPU pass, wire it into `compile_unit`'s sequence in
   `pipeline::compile_unit`. Order matters: the lex pass MUST run
   before `bracket_match`, etc. Document the dependency in a comment
   on the pass function.
3. For a host pass, add a test under `tests/` that exercises it on
   a representative TU; for a GPU pass, add a conform fixture under
   `conform/vyre-conform-runner/fixtures` so the backend is diffed
   against the CPU reference.
4. Extend the `.vyrecob2.*` payload section only through a version
   bump. Old tooling MUST refuse to load a new-version payload
   rather than attempt a partial read.
5. Update `docs/COMPILER_E2E_PLAN.md` with the pass's phase number
   and preconditions; that doc is the source of truth for pipeline
   ordering, not individual file comments.

See `pipeline/compile_unit.rs` for the end-to-end driver and
`elf_linux.rs` for the ET_REL emission template.

## Beta evidence

Readiness for this crate is separate from the core Vyre platform release.
Claims here must map to concrete gate output, benchmark output, conformance
output, parser corpus output, or documentation proof files before `vyrec` can
graduate from beta.

Concrete release evidence anchors:

- `release/evidence/parser/distributed-parser-map.json`
- `release/evidence/parser/c-parser-linux-subsystem.json`
- `release/evidence/parser/c-parser-diagnostics-summary.json`
- `release/evidence/parser/c-parser-throughput.json`
