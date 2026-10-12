//! Query-only adversarial tests for vyre_foundation::extern_registry.
//!
//! These tests do NOT attempt to construct `#[non_exhaustive]` structs; they
//! query the existing registry state and document contract violations.
//!
//! This test binary seeds its own synthetic registration so the query checks
//! are never vacuous.

use std::collections::HashSet;
use vyre_foundation::extern_registry::{
    all_ops, dialects, ops_in_dialect, ExternDialect, ExternOp,
};

inventory::submit! {
    ExternDialect::new(
        "vyre-libs-query-probe",
        "0.1.0",
        "https://example.invalid/query-probe",
    )
}

inventory::submit! {
    ExternOp::new("vyre-libs-query-probe", "vyre-libs-query-probe::probe")
}

#[test]
fn synthetic_probe_registration_is_visible() {
    let probe_ops = ops_in_dialect("vyre-libs-query-probe");
    assert_eq!(
        probe_ops.len(),
        1,
        "synthetic probe dialect must expose exactly one op. \
         Fix: confirm this integration test binary links its own inventory::submit! seed."
    );
    assert_eq!(probe_ops[0].op_id, "vyre-libs-query-probe::probe");
}

#[test]
fn ops_in_dialect_nonexistent_returns_empty_no_panic_extended_corpus() {
    // Extend the existing corpus with adversarial / injection-style inputs.
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
    assert!(
        ops_in_dialect("test-pack-a\nline").is_empty(),
        "newline injection must not match"
    );
    assert!(
        ops_in_dialect("test-pack-a\tcol").is_empty(),
        "tab injection must not match"
    );
}

#[test]
fn dialect_names_are_sanitized_when_present() {
    // Vacuously true when registry is empty, but enforces the contract
    // for every dialect that IS registered.
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

#[test]
fn no_duplicate_dialect_names_when_present() {
    // Vacuously true when registry is empty, but enforces the uniqueness
    // contract for every dialect that IS registered.
    let all = dialects();
    let names: Vec<_> = all.iter().map(|d| d.name).collect();
    let unique: HashSet<_> = names.iter().cloned().collect();

    assert_eq!(
        names.len(),
        unique.len(),
        "CRITICAL FINDING: Duplicate dialect names detected: {:?}. \
         A verifier must reject duplicate ExternDialect::name entries. \
         Fix: add verify_no_duplicate_dialects().",
        names
    );
}

#[test]
fn ops_belong_to_registered_dialects_when_present() {
    // Every op's dialect field must match a registered dialect name.
    let known: HashSet<&str> = dialects().iter().map(|d| d.name).collect();
    for op in all_ops() {
        assert!(
            known.contains(op.dialect),
            "CRITICAL FINDING: Orphan op '{}' refers to unregistered dialect '{}'. \
             A verifier must reject ops whose dialect does not appear in the dialect list. \
             Fix: add verify_ops_have_registered_dialect().",
            op.op_id,
            op.dialect
        );
    }
}

#[test]
fn no_empty_op_ids_when_present() {
    // Every op must have a non-empty op_id.
    for op in all_ops() {
        assert_ne!(
            op.op_id.len(),
            0,
            "CRITICAL FINDING: ExternOp with empty op_id in dialect '{}'. \
             Empty op_id breaks downstream lookup and must be rejected. \
             Fix: add verify_op_ids_non_empty().",
            op.dialect
        );
    }
}
