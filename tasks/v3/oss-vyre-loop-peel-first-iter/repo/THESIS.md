# Vyre Thesis

Vyre is a substrate-neutral primitive-composition IR for turning typed,
auditable programs into CPU and GPU execution without leaking backend details
into domain logic.

## Core contract

- Core owns graph structure, value identity, memory regions, wire identity,
  and trait contracts.
- Backend crates own acquisition, compilation, dispatch, resident resources,
  timing, and target capability checks.
- Frontend and library crates compose public primitives into `Program`
  values; they do not own backend policy.
- Evidence gates are product surfaces: every capability claim must resolve to
  a command, artifact, fixture, or byte-level parity check.

## Architectural laws

- Open IR: hot-path node forms stay compact, while extension points keep the
  operation world open.
- Backend isolation: no WGSL, PTX, MSL, SPIR-V, CUDA, WGPU, or Metal policy
  crosses into core program semantics.
- Capability negotiation: unsupported target behavior is represented as a
  structured capability diagnostic with an owner and fix path.

## Execution thesis

Vyre wins by making composition cheap and evidence mandatory. A high-level
operation should lower into reusable Lego-block primitives, share facts and
buffers across passes, route to the fastest valid backend plan, and produce
auditable output bytes. Performance work is therefore correctness work:
extra copies, stale caches, hidden fallbacks, panics, duplicate planners, and
unbounded allocations are production defects.

## Research thesis

The research surface is not a separate playground. New primitives, GPU
algorithms, parser paths, dataflow solvers, and benchmark harnesses must prove
three properties before they become part of the production surface:

- A real Vyre consumer uses the primitive through the public composition path.
- The primitive has CPU reference, backend parity, adversarial, and scale
  evidence appropriate to its risk.
- The implementation consolidates an existing seam instead of creating a
  second owner for the same behavior.

## Source map

- `docs/THESIS.md` is the compatibility redirect for older links.
- `ARCHITECTURE.md` defines the engineering boundary rules.
- `docs/VISION.md` defines the long-range product direction.
- `docs/memory-model.md` defines memory behavior.
- `docs/targets.md` defines target tiers and backend registration.
