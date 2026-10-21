# vyre-conform-runner

CLI + library for emitting and verifying vyre conformance certificates.

A conformance certificate pins the exact backend + Program + witness tuple
matrix that vyre claims to run identically across every target. It is
signed, content-addressed, and checked on every release; if the claim
fails, the certificate never emits.

## Position in the conform subsystem

| Crate                    | Role                                              |
| ------------------------ | ------------------------------------------------- |
| `vyre-conform-spec`      | The declarative witness inventory (data only).    |
| `vyre-conform-generate`  | Produces witness corpora from the spec.           |
| `vyre-conform-enforce`   | Backend-agnostic assertions a runner calls into.  |
| `vyre-conform-runner`    | **This crate.** CLI + certificate builder.        |
| `vyre-test-harness`      | Dev-loop wrapper for per-PR integration runs.     |

## CLI

```
vyre-conform run --backend wgpu --ops all
```

Flags:
- `--backend {wgpu|reference|all}`: backend to run against.
- `--ops {all|<op-id> [<op-id>...]}`: scope the matrix.
- `--bundle-cert <path>`: emit a signed `BundleCertificate` after a
  clean run. The bundle content-addresses every witness and the reference
  output, so downstream callers can verify the whole matrix from a single
  blake3 digest.
- `--verify <path>`: verify an existing certificate against the current
  backend without regenerating witnesses.

## Library surface

`run_matrix(backend, witnesses)` returns `Result<BundleCertificate>`.
`verify_certificate(cert, backend)` returns `Result<()>` and is the sole
blessed path for production consumers asking "does this backend conform
to the claimed contract?". Both functions are `#[must_use]`: dropping
the result without checking it is a compile warning.

## Laws

- No runner step may silently skip a witness. A missing backend
  capability (e.g. no subgroup ops on a software adapter) is a loud
  "skipped: <reason>" entry in the certificate, never a clean pass.
- Certificate bytes are stable under serialization: producing the same
  certificate on two hosts returns byte-identical output (witnesses are
  LE-encoded via the `vyre_foundation::opaque_payload` helpers where
  relevant; see F-IR-32 in the IR soundness audit).
- Every failure carries a `Fix:` hint naming the op, witness tuple, and
  backend so CI can route the triage automatically.

## When to extend

When a new op ships a spec entry in `vyre-conform-spec`, this runner
picks it up automatically through the shared inventory. No code change
here unless a new backend needs wiring.
