# Wire primitive — 10-tier contract directory

Per the per-rule directory contract: every shipped primitive (here, the
`vyre_primitives::wire` family) has every tier covered by a real test
that locks the truth of a specific claim. This directory is the index.

| Tier                     | Status | Where                                                                |
| ------------------------ | ------ | -------------------------------------------------------------------- |
| 1. Positive truth        | ok     | `tests/wire_pack_into_contracts.rs` (19 contracts)                   |
| 2. Negative precision    | ok     | `tests/wire_pack_into_contracts.rs::*_rejects_*` (truncation, empty) |
| 3. Adversarial / evasion | ok     | `tests/proptest_wire_roundtrip.rs` NaN/Inf/subnormal/overflow corpus |
| 4. Cross-file            | ok     | `vyre-libs/tests/wire_cross_crate_compat.rs`                         |
| 5. CVE replay            | n/a    | wire is foundational substrate, not a vuln-detection rule           |
| 6. Property (proptest)   | ok     | `tests/proptest_wire_roundtrip.rs` (8 properties × 10 000 cases)     |
| 7. Differential          | ok     | `tests/wire_differential_std_io.rs` vs `std::io::Cursor` LE reader   |
| 8. Performance           | ok     | `benches/wire_throughput.rs` (criterion, 3 input sizes × 6 paths)    |
| 9. Scale                 | open   | `tests/wire_contracts/scale.rs` (TODO: 1B-byte buffer roundtrip)     |
| 10. End-to-end CLI       | open   | drives the vyre binary once one ships                                |

The two "open" tiers stay open and unfinished — they are real work the
codebase owes against the wire claim, not deferred work.
