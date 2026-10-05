# Public Jane Street sources and design rationale

Research inspected October 1, 2026. The task designs below are VulcanBench
inferences from public interfaces, not claims about internal workflows or
endorsement. The earlier [Jane Street research](../ocaml-v1/JANE_STREET_RESEARCH.md)
provides broader educational and engineering context.

## Async delivery lifecycle

The pinned public Async monitor interface documents exception capture with
try_with, scheduling choices, and extraction of underlying exceptions.
[Async monitor v0.17.0](https://raw.githubusercontent.com/janestreet/async_kernel/v0.17.0/src/monitor.mli).
The public pipe interface exposes closure and downstream-flush semantics.
[Async pipe v0.17.0](https://raw.githubusercontent.com/janestreet/async_kernel/v0.17.0/src/pipe.mli).

The original durable-delivery task uses these public Async primitives to test
an integration boundary: a successful worker result must still survive ordered
journal acknowledgement, while timeouts, cancellations and shutdown can happen
before scheduler callbacks run. Its deterministic clock and journal protocol
are explicitly supplied by VulcanBench. They are not copies of an upstream
service. Generation ownership, retry ordering, and callback reentrancy make
the task harder through stated interactions rather than a shorter solve budget.

## Typed serialization compatibility

The pinned public bin_prot reader and writer expose fixed-width int64_bits
primitives and position-based decoding.
[Reader](https://raw.githubusercontent.com/janestreet/bin_prot/v0.17.0/src/read.ml),
[writer](https://raw.githubusercontent.com/janestreet/bin_prot/v0.17.0/src/write.ml),
[reader interface](https://raw.githubusercontent.com/janestreet/bin_prot/v0.17.0/src/read_intf.ml).

The original schema task uses that fixed-width primitive inside a completely
specified record format. Generic GADT decoding, heterogeneous field tuples,
legacy defaults, nested boundaries and rollback require both type mastery and
protocol engineering. Thirty legacy vectors are generated independently with
Python little-endian struct packing, including signed extrema and opaque bytes.
The unknown-field allocation limit rejects a plausible implementation that
copies discarded data. This custom record format is not advertised as bin_prot's
record representation or an upstream compatibility defect.

## Full Base repository work

Base's pinned hash-table signature describes mutation guards and key hash /
comparison assumptions. Its AVL interface exposes the mutable search-tree
operations used by Hashtbl.
[Hash-table signature](https://raw.githubusercontent.com/janestreet/base/v0.17.3/src/hashtbl_intf.ml),
[AVL interface](https://raw.githubusercontent.com/janestreet/base/v0.17.3/src/avltree.mli),
[implementation](https://raw.githubusercontent.com/janestreet/base/v0.17.3/src/hashtbl.ml).
Its package project and build declarations provide the release-family context.
[Project](https://raw.githubusercontent.com/janestreet/base/v0.17.3/dune-project),
[library build](https://raw.githubusercontent.com/janestreet/base/v0.17.3/src/dune).

The feature request extends the actual Base v0.17.3 repository at commit
f8d1d3ad1894590cd19d11b9a89fe799caee5e57. All 495 tracked source files and
upstream MIT license are supplied. The added API prototype and three client
modules are original. There is no public reference patch being reproduced,
but the underlying source remains publicly retrievable; decontaminated=false.

Nested savepoints must cover direct AVL updates and existing helper mutations
while preserving original key and data identities. Explicit sparse-work bounds
reject an eager full-table snapshot that otherwise restores contents correctly.
This evaluates navigation and invariant preservation in an actual library,
without requiring unstated finance knowledge or an OxCaml extension toolchain.
