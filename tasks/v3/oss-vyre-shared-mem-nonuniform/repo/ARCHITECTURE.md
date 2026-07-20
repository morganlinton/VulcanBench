# Vyre Architecture

Vyre architecture is organized around one-way ownership boundaries. Domain
logic builds `Program` values, foundation crates preserve canonical identity,
lowering converts programs into backend-neutral descriptors, emitters produce
target artifacts, and drivers execute against concrete devices.

## Layer boundaries

- `vyre-core` exposes the public facade and stable composition contracts.
- `vyre-foundation` owns canonical IR storage, wire bytes, fingerprints,
  validation metadata, optimization facts, and scheduler contracts.
- `vyre-lower` owns backend-neutral lowering from `Program` to descriptors.
- `vyre-emit-*` crates own target artifact generation and target capability
  diagnostics.
- `vyre-driver` owns backend-neutral lifecycle, binding, specialization,
  residency, graph capture, validation, and evidence schemas.
- Backend crates own target-specific device acquisition, pipeline caching,
  dispatch, resident resources, synchronization, and metrics.
- `vyre-runtime` owns persistent megakernel protocol semantics; drivers adapt
  buffers and launches but do not duplicate runtime protocol decoding.

## Modularity rules

- One primitive, one schema, one parser, one planner, and one constant source.
- Public APIs are facades; implementation ownership lives in the narrowest
  crate that can serve every caller.
- Compatibility aliases must have canonical owners, canonical paths, and
  removal conditions in one registry.
- Cross-backend behavior must pass through shared ABI, binding, result
  compaction, validation, capability, and evidence schemas.

## Evidence rules

- Every user-visible claim needs a command, artifact, fixture, or byte-level
  assertion.
- Every backend optimization must preserve output bytes or emit a structured
  unsupported diagnostic before launch.
- Every performance claim needs source fingerprints, backend ids, device
  signatures, command provenance, active GPU time where available, transfer
  bytes, and output digests.
- Every research primitive needs a production consumer or an explicit feature
  boundary enforced by a gate.

## Failure rules

- Fail closed at boundaries.
- Error messages include the violated contract and the fix path.
- No hidden fallback can change backend choice, output bytes, resource
  ownership, or evidence status without being visible in the result schema.
