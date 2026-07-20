//! Adversarial break-it tests for extern_registry registration integrity.
//!
//! Every test in this file describes what SHOULD be true. If current code
//! doesn't meet the contract, the test fails  -  that's a documented FINDING
//! requiring an implementation fix.
//!
//! Expected FINDINGS (tests that will fail until fixed):
//! - Duplicate dialect names are silently accepted (no verifier rejects them).
//! - Malformed dialect names (not starting with `vyre-libs-`) are silently accepted.
//! - Orphan ops (dialect not in dialects list) are silently accepted.
//! - Empty op_id is silently accepted.
//!
//! Quality bar: break-it level. Assumes 10,000 community dialects will
//! register simultaneously one day  -  every failure mode must surface NOW.

use std::collections::HashSet;
use vyre_foundation::extern_registry::{
    all_ops, dialects, ops_in_dialect, verify, ExternDialect, ExternOp, ExternVerifyError,
};

// ═══════════════════════════════════════════════════════════════════════════════
// Adversarial registrations
// ═══════════════════════════════════════════════════════════════════════════════

// ── Angle 1: duplicate dialect name ────────────────────────────────────────────
// Two inventory::submit! invocations with the same ExternDialect.name.
// The library must NOT silently dedupe; a verifier fn must reject this.
inventory::submit! {
    ExternDialect::new("vyre-libs-dup", "0.1.0", "https://example.invalid/dup")
}
inventory::submit! {
    ExternDialect::new("vyre-libs-dup", "0.2.0", "https://example.invalid/dup2")
}

// ── Angle 2: op_id collision across dialects ───────────────────────────────────
// Same op_id string registered by two different dialects.
// Must be distinguishable via ops_in_dialect.
inventory::submit! {
    ExternOp::new("vyre-libs-pack-a", "foo")
}
inventory::submit! {
    ExternOp::new("vyre-libs-pack-b", "foo")
}

// ── Angle 3: malformed dialect name ────────────────────────────────────────────
// Name does NOT start with `vyre-libs-`. The library should emit a diagnostic.
inventory::submit! {
    ExternDialect::new("not-vyre-libs-bad", "0.1.0", "https://example.invalid/bad")
}

// ── Angle 4: orphan op (dialect not registered) ────────────────────────────────
inventory::submit! {
    ExternOp::new("vyre-libs-orphan", "vyre-libs-orphan::lonely")
}

// ── Angle 5: empty op_id ───────────────────────────────────────────────────────
inventory::submit! {
    ExternOp::new("vyre-libs-empty", "")
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn duplicate_dialect_names_must_be_rejected_by_verifier() {
    // Both submissions land (inventory::submit! is append-only)  -
    // the verifier catches the duplicate.
    let errors = verify().expect_err("duplicate dialect must surface at verify() time");
    assert!(
        errors.iter().any(|e| matches!(
            e,
            ExternVerifyError::DuplicateDialect { name, .. } if *name == "vyre-libs-dup"
        )),
        "verify() must report ExternVerifyError::DuplicateDialect for 'vyre-libs-dup'; got {errors:?}"
    );
}

#[test]
fn op_id_collision_across_dialects_is_distinguishable() {
    let a_ops = ops_in_dialect("vyre-libs-pack-a");
    let b_ops = ops_in_dialect("vyre-libs-pack-b");

    assert_eq!(a_ops.len(), 1, "pack-a must have exactly 1 op");
    assert_eq!(b_ops.len(), 1, "pack-b must have exactly 1 op");

    assert_eq!(a_ops[0].dialect, "vyre-libs-pack-a");
    assert_eq!(a_ops[0].op_id, "foo");
    assert_eq!(b_ops[0].dialect, "vyre-libs-pack-b");
    assert_eq!(b_ops[0].op_id, "foo");

    // Global view must contain BOTH, not dedupe by op_id.
    let global = all_ops();
    let colliding: Vec<_> = global.iter().filter(|o| o.op_id == "foo").collect();
    assert_eq!(
        colliding.len(),
        2,
        "all_ops() must contain both colliding ops from different dialects; found {}. \
         Global deduplication by op_id is a CRITICAL bug.",
        colliding.len()
    );
}

#[test]
fn malformed_dialect_name_must_emit_diagnostic() {
    // Registration is always allowed (inventory is append-only); the
    // verifier surfaces the name-prefix violation as an error.
    let errors = verify().expect_err("malformed name must surface at verify() time");
    assert!(
        errors.iter().any(|e| matches!(
            e,
            ExternVerifyError::MalformedDialectName { name } if *name == "not-vyre-libs-bad"
        )),
        "verify() must report ExternVerifyError::MalformedDialectName for 'not-vyre-libs-bad'; got {errors:?}"
    );
}

#[test]
fn orphan_ops_must_be_rejected_by_verifier() {
    let known_names: HashSet<&str> = dialects().iter().map(|d| d.name).collect();
    assert!(
        !known_names.contains("vyre-libs-orphan"),
        "test invariant: 'vyre-libs-orphan' must not be a registered dialect"
    );

    // `ops_in_dialect` is a filter, not a validator. The filter by
    // itself is allowed to return the registered op; the verifier
    // surfaces the orphan as an error.
    let errors = verify().expect_err("orphan op must surface at verify() time");
    assert!(
        errors.iter().any(|e| matches!(
            e,
            ExternVerifyError::OrphanOp { dialect, .. } if *dialect == "vyre-libs-orphan"
        )),
        "verify() must report ExternVerifyError::OrphanOp for 'vyre-libs-orphan'; got {errors:?}"
    );
}

#[test]
fn empty_op_id_must_be_rejected_by_verifier() {
    let errors = verify().expect_err("empty op_id must surface at verify() time");
    assert!(
        errors.iter().any(|e| matches!(
            e,
            ExternVerifyError::EmptyOpId { dialect } if *dialect == "vyre-libs-empty"
        )),
        "verify() must report ExternVerifyError::EmptyOpId for 'vyre-libs-empty'; got {errors:?}"
    );
}

#[test]
fn ops_in_dialect_nonexistent_returns_empty_no_panic() {
    // Baseline plus adversarial / injection-style inputs.
    assert!(
        ops_in_dialect("").is_empty(),
        "empty string dialect must not panic"
    );
    assert!(
        ops_in_dialect("vyre-libs-definitely-not-real-99999").is_empty(),
        "missing dialect must return empty vec"
    );
    assert!(
        ops_in_dialect("test-pack-a\x00injected").is_empty(),
        "null-byte injection attempt must not panic or match"
    );
    assert!(
        ops_in_dialect("VYRE-LIBS-UPPERCASE").is_empty(),
        "case sensitivity must be enforced"
    );
    assert!(
        ops_in_dialect("vyre-libs-").is_empty(),
        "prefix-only name must not match real dialects"
    );
    assert!(
        ops_in_dialect(" vyre-libs-pack-a").is_empty(),
        "leading whitespace must not match"
    );
    assert!(
        ops_in_dialect("vyre-libs-pack-a ").is_empty(),
        "trailing whitespace must not match"
    );
}

#[test]
fn dialect_names_are_sanitized() {
    for d in dialects() {
        assert!(!d.name.is_empty(), "empty dialect name is a CRITICAL bug");
        assert!(
            !d.name.contains(' '),
            "whitespace in dialect name '{}' is a CRITICAL bug",
            d.name
        );
        assert!(
            !d.name.contains('\t'),
            "tab in dialect name '{}' is a CRITICAL bug",
            d.name
        );
        assert!(
            !d.name.contains('\n'),
            "newline in dialect name '{}' is a CRITICAL bug",
            d.name
        );
    }
}
