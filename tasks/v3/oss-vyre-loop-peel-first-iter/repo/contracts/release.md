# vyre 0.4.1 release-gap contracts

Each gap has a failing test committed to the repo. An agent closes
the gap by implementing the engine so the test goes green  -  WITHOUT
weakening the assertion. Weakening the test is a LAW 9 fireable
offense; if the test is wrong, write a BETTER one, not a smaller one.

## Acceptance gate (every gap)

```bash
# The test file must fail today for the stated reason
./cargo_full test -p <crate> --test <gap-test-file>
# After the engine change it must pass
./cargo_full test -p <crate> --test <gap-test-file>
# And every other test still passes
./cargo_full test --workspace
# And clippy stays clean
./cargo_full clippy --workspace -- -D warnings
```

No `#[ignore]`, no `#[should_panic]` escape hatches except where the
gap explicitly documents one.

## The 12 gaps

| # | Item | Test path | Pass criteria |
|---|------|-----------|---------------|
| 1 | Reference completeness (deterministic transcendentals) | `vyre-reference/tests/gap_transcendentals_parity.rs` + `vyre-driver-wgpu/tests/gap_transcendentals_parity.rs` | Reference oracle bytes == `ieee754::canonical_*` on 1000 proptest inputs; GPU bytes == CPU reference bytes for sin/cos/sqrt/exp/log (bitwise test requires `--features parity-testing`) |
| 3 | Device-loss recovery | `vyre-driver-wgpu/tests/gap_device_lost_recovery.rs` | After simulated `device_lost() == true`, `try_recover()` returns `Ok(())` AND a subsequent `dispatch` succeeds |
| 4 | Pre-emption / deadline cancellation | `vyre-driver-wgpu/tests/gap_dispatch_preemption.rs` | Dispatch with `timeout = 100ms` on a 2-second program returns a cancellation error within 250ms, NOT 2s+100ms |
| 5 | Determinism contract | `vyre-driver-wgpu/tests/gap_determinism_contract.rs` | `dispatch(p, inputs)` called twice on the same backend returns byte-identical outputs, 1000 proptest runs |
| 6 | Public-API snapshot | `scripts/check_public_api.sh` | `cargo-public-api diff` against `PUBLIC_API.md` is empty |
| 8 | Doctest coverage | `scripts/check_doctest_coverage.sh` | Every `pub fn` / `pub struct` / `pub trait` in vyre-core, vyre-foundation, vyre-driver, vyre-driver-wgpu has a doctest |
| 9 | Error-code catalog | `vyre-driver/tests/gap_error_code_catalog.rs` | Every `ErrorCode` variant has a stable integer + entry in `docs/error-codes.md`; test verifies both |
| 11 | Unified performance meta-harness | `docs/VYRE_BENCH_META_HARNESS_PRD.md` | `vyre-bench` owns one registry, one result schema, one budget model, and one candidate-evaluation boundary |
| 12 | Parity cert artifact | `conform/vyre-conform-runner/tests/gap_cert_artifact.rs` | `./cargo_full run -p vyre-conform-runner -- prove --out <path>` produces a signed JSON cert with `wire_format_version`, `program_hash`, `backend_id`, `signature` |
| 13 | Dialect duplicate-id gate | `vyre-driver/tests/gap_duplicate_op_id.rs` | Registering two ops with the same id at compile time panics at registry init with `Fix: duplicate op id <name>` |
| 14 | CI matrix | `.github/workflows/ci.yml` + `scripts/check_ci_matrix.sh` | CI declares Linux+macOS+Windows × stable+nightly + with/without GPU feature |
| 15 | Release engineering | `scripts/check_release_ready.sh` | `cargo install --path vyre-core --root /tmp/vyre-install` succeeds AND produces a `vyre` binary that runs `--version` + a minimal demo |

## Split

Codex-A owns: **1, 3, 5, 8, 12, 14** (reference/GPU/docs/cert/CI)
Codex-B owns: **4, 6, 9, 11, 13, 15** (pre-emption/surface/catalog/meta-harness/gate/install)

Each codex:
1. Reads this file.
2. Works through its 6 items in order.
3. Writes the failing test at the named path (if not already stubbed).
4. Iterates engine changes until the test goes green.
5. Commits each closed gap as its own focused commit.

## Hard rules (same as the rest of 0.6)

- No `todo!()`, no `unimplemented!()`, no stubs.
- No weakening tests. If a test is wrong, rewrite it strictly.
- No `#[ignore]` without a `FINDING-XXXX` comment AND explicit approval.
- Every error carries `Fix: ` prose.
- Doctests on every public item closed this round.
- Workspace build green, clippy `-D warnings` green.
