# vyre-aot  -  architecture

Ahead-of-time compilation of vyre `Program`s into deployable
artifacts (binaries, shader blobs, manifest-bundled dispatch
plans) so a downstream consumer can ship a frozen kernel without
the full source compiler toolchain at runtime.

## Modules

### `artifact.rs`
Wire format for a single AOT artifact: serialized program bytes,
backend identifier, target adapter caps, conformance certificate
hash, optional debug info. Frozen  -  the artifact wire is part of
the public contract.

### `bundle.rs`
Multi-artifact container. One bundle ships every (program, backend,
target) combination an application needs. Internal layout: framing
header + sequential `Artifact` records with offset table.

### `compile.rs`
Orchestrates the AOT compile pipeline. Takes a `Program` plus a
target spec (backend, adapter limits, optimisation profile) and
produces an `Artifact`. Reuses the per-backend lowering passes
from `vyre-driver-*` so AOT and JIT outputs are byte-identical.

### `launcher.rs`
Runtime side: loads a Bundle, picks the artifact matching the live
adapter, hands it to the backend's persistent-pipeline cache.

### `manifest.rs`
TOML manifest describing a Bundle's contents (which target each
artifact serves, which capabilities are required, where the
conformance cert lives). Generated at compile time, consumed by
the Launcher.

## Public types

- **`Artifact`**  -  single (program, backend, target) artifact. Wire-
  serialised, content-addressable via the conformance-cert hash.
- **`Bundle`**  -  multi-artifact container with manifest.
- **`CompileTarget`**  -  enumerates backend + adapter caps the
  AOT pipeline lowers against.
- **`Manifest`**  -  TOML-driven bundle metadata.
- **`Launcher`**  -  runtime loader; matches a live adapter to an
  artifact and hands it to the backend.

## Integration points

- Reads `vyre::ir::Program` from `vyre-foundation`.
- Calls into `vyre-driver-{wgpu,spirv,cuda}` lowering for each
  target.
- Writes a conformance certificate compatible with downstream proof
  bundles (so an AOT-shipped rule still carries the same
  bit-identical reproducibility hash chain as the JIT path).
