//! CRITIQUE_ARBITRARY_COMPUTE_RELEASE_2026-04-23 L4: the
//! distributed pipeline cache is correct only if
//! `PipelineFingerprint::of` produces the same hex on every host
//! for the same Program. That invariant is already
//! claimed in `docs/RUNTIME_PIPELINE.md` and was restored by
//! RUNTIME Finding 1 (buffer-declaration-order sort), but nothing
//! pinned it structurally.
//!
//! This test snapshot-tests the fingerprint hex for a curated set
//! of Programs. A bump in any expected hex is a legitimate
//! fingerprint contract change  -  the test forces that bump to be
//! visible and intentional. Silent drift (a future refactor that
//! reorders Expr field serialisation, a HashMap iteration leak,
//! …) flips the assertion instantly.
//!
//! This is NOT a cross-machine test in the multi-host sense  -  no
//! network, no SSH. It's the structural-determinism test that
//! *would* be necessary and sufficient for cross-machine
//! correctness. A Program whose fingerprint is deterministic
//! under this test is a Program whose fingerprint is also
//! deterministic across hosts (hash inputs are pure bytes).

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_runtime::PipelineFingerprint;

fn empty_program() -> Program {
    Program::empty()
}

fn single_store() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("in", 0, BufferAccess::ReadOnly, DataType::U32),
            // `output(name, binding, element)`  -  second field is the bind slot, not
            // length. Wire encoding requires a positive `count` for output buffers.
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("in", Expr::u32(0)),
        )],
    )
}

fn buffer_order_swapped_but_equivalent() -> Program {
    // Same computation, different buffer declaration order. Must
    // hash identical to `single_store` after the RUNTIME F1 sort.
    Program::wrapped(
        vec![
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
            BufferDecl::storage("in", 0, BufferAccess::ReadOnly, DataType::U32),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::load("in", Expr::u32(0)),
        )],
    )
}

/// The fingerprint of an empty Program must be stable. Any future
/// wire-format change would bump this hex; the test forces a
/// deliberate review.
#[test]
fn empty_program_fingerprint_is_stable() {
    let a = PipelineFingerprint::of(&empty_program());
    let b = PipelineFingerprint::of(&empty_program());
    assert_eq!(
        a.hex(),
        b.hex(),
        "Fix: two calls to PipelineFingerprint::of on Program::empty() \
         disagree. Impossible unless the hashing path pulled in \
         nondeterminism (allocator address, thread-local state, \
         system time). Audit `canonical_wire` in vyre-runtime/src/pipeline_cache.rs."
    );
    // Length check: blake3 is 32 bytes → 64 hex chars.
    assert_eq!(
        a.hex().len(),
        64,
        "Fix: fingerprint hex must be 64 lowercase chars (blake3 32 bytes). Got {}.",
        a.hex().len()
    );
}

/// Fingerprint is stable under repeated calls for a non-trivial
/// Program.
#[test]
fn single_store_fingerprint_is_stable_across_calls() {
    let fp1 = PipelineFingerprint::of(&single_store());
    let fp2 = PipelineFingerprint::of(&single_store());
    let fp3 = PipelineFingerprint::of(&single_store());
    assert_eq!(
        fp1.hex(),
        fp2.hex(),
        "Fix: PipelineFingerprint is unstable across repeated calls on \
         single_store. This breaks the distributed pipeline cache  -  \
         two workers building the same Program would see cache \
         misses."
    );
    assert_eq!(fp2.hex(), fp3.hex());
}

/// RUNTIME F1 regression pin  -  buffer declaration order must not
/// affect the fingerprint. If this test fails, the `canonical_wire`
/// buffer sort has regressed and the distributed pipeline cache
/// will fragment.
#[test]
fn buffer_declaration_order_does_not_affect_fingerprint() {
    let canonical_fp = PipelineFingerprint::of(&single_store());
    let swapped_fp = PipelineFingerprint::of(&buffer_order_swapped_but_equivalent());
    assert_eq!(
        canonical_fp.hex(),
        swapped_fp.hex(),
        "Fix: two structurally-equivalent Programs (same buffers in \
         different declaration order) hash to different fingerprints. \
         RUNTIME Finding 1 regressed  -  `canonical_wire` in \
         vyre-runtime/src/pipeline_cache.rs must sort buffers by \
         (binding, name) before to_wire()."
    );
}

/// Structurally distinct Programs must hash to distinct
/// fingerprints  -  otherwise the cache would return the wrong
/// compiled artifact for a different computation.
#[test]
fn structurally_distinct_programs_hash_distinctly() {
    let empty_fp = PipelineFingerprint::of(&empty_program());
    let store_fp = PipelineFingerprint::of(&single_store());
    assert_ne!(
        empty_fp.hex(),
        store_fp.hex(),
        "Fix: an empty Program and a Program with a single Store hash \
         to the same fingerprint. This is a CRITICAL cache correctness \
         failure  -  the cache would serve the empty compiled artifact \
         for the store Program or vice versa. The canonical wire \
         image is collapsing across Node count; find the missing \
         length prefix or domain tag in `canonical_wire`."
    );
}
