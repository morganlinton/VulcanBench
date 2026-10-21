# vyre-aot

Ahead-of-time compilation for vyre Programs. Lowers a `Program` once,
emits target-native bytes plus a self-contained Rust launcher binary,
and removes the entire vyre runtime from the deployed image.

## What this crate is

vyre is a JIT-friendly substrate: at runtime, `vyre-driver` routes a
`Program` to a backend module loaded by the GPU driver. That works
when the consumer can ship the runtime alongside the artifact. For
embedded targets and code-budget-constrained submissions
(parameter-golf is the motivating case: `code_bytes +
compressed_model_bytes ≤ 16,000,000`) we need the compiler to disappear
at runtime. Only the target module and a thin launcher remain.

## Compile flow

```
Program ─► vyre-driver AOT emitter registry
        ─► target module bytes
        ─► [optional] brotli or lzma compression
        ─► launcher.rs (target glue, byte-pinned to the artifact)
        ─► single binary
```

The launcher carries no vyre dependency. It contains exactly the
register / launch / readback code path the artifact needs. Module
loading happens through the concrete driver crate that owns the target.
The launcher is byte-stable across machines.

## Feature flags

| Feature | Default | Purpose                                                           |
|---------|---------|-------------------------------------------------------------------|
| `secondary_text`   | off     | Enable tests for the primary text target when an emitter is linked. |

The crate itself does not depend on concrete drivers. Binaries that need a
target link the concrete driver crate that registers the matching AOT emitter.

## Configuration

Tier-A operational knobs live in `CONFIG.md` (target SM, optimization
profile, compression algorithm, cache root, verbosity). Tier-B
contracts live under `rules/aot/*.toml` for per-target shape budgets.
Embedding tools surface the same env vars verbatim.

## Architecture decisions

- **Thin launcher.** The launcher is generated, not linked. We
  control its size byte-for-byte so the parameter-golf budget is met.
- **Compression in the artifact, not the binary.** Target bytes are
  brotli- or lzma-compressed inside the launcher; decompression
  happens once at startup.
- **No JIT at deploy time.** vyre-aot artifacts do not depend on a
  runtime JIT path. The deployed binary contains only the target loader
  path and nothing else from the vyre stack.
- **Backend-owned emitters.** Concrete drivers register AOT emitters through
  `vyre-driver`; vyre-aot consumes the registry and never imports a concrete
  driver.

## Where to look

- `src/lib.rs`: public surface (`AotCompileOptions`, `compile`,
  artifact format).
- `src/launcher/`: generated launcher templates.
- `src/compress/`: brotli + lzma wrappers.
- `CONFIG.md`: Tier A/B configurability surface.
- `OWNERSHIP.md` (workspace root): dependency boundary.
- The `vyre-runtime` README: for what the artifact replaces.

## Conformance

Artifacts produced by vyre-aot must run byte-identical to the live-runtime
path on the conformance corpus. The conformance runner verifies parity
across `runtime` and every linked AOT target for every Cat-A op.
Drift is publish-blocking.

## Anti-goals

- vyre-aot is not a concrete-backend-specific tool. Targets are registered
  by concrete driver crates.
- vyre-aot is not a kernel-author surface. Use `vyre-libs` to author
  programs; `vyre-aot` only freezes them.
