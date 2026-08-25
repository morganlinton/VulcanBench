//! Failure-oriented tests for the routing registry public surface.
//!
//! Guarantees:
//! - `RoutingTable` reports `None` for unknown call sites
//! - `DialectRegistry` reports `None` for unknown ops / unsupported targets
//! - Duplicate-op errors are actionable and name both registrants

use std::borrow::Cow;

use vyre_driver::{DialectRegistry, OpDef, RoutingTable, SortBackend, Target};

#[test]
fn routing_table_distribution_returns_none_for_unknown_callsite() {
    let table = RoutingTable::default();
    assert!(
        table.distribution("no.such.callsite").is_none(),
        "Fix: RoutingTable::distribution must return None for unseen call sites"
    );
}

#[test]
fn routing_table_empty_input_distribution() {
    let table = RoutingTable::default();
    let backend = table
        .observe_sort_u32(Cow::Borrowed("empty.sort"), &[])
        .expect("Fix: empty input must not panic");
    assert_eq!(backend, SortBackend::InsertionSort);
    assert!(
        table.distribution("empty.sort").is_some(),
        "Fix: distribution must be recorded for empty input"
    );
}

#[test]
fn dialect_registry_lookup_miss_returns_none() {
    let guard = DialectRegistry::global();
    let unknown = guard.intern_op("definitely.not.a.real.op");
    assert!(
        guard.lookup(unknown).is_none(),
        "Fix: DialectRegistry::lookup must return None for unknown ops"
    );
}

#[test]
fn dialect_registry_get_lowering_miss_for_unsupported_target() {
    let guard = DialectRegistry::global();
    // Even if an op exists, extension targets return None until a concrete
    // backend has registered a lowering for that stable target id.
    let unknown = guard.intern_op("also.not.real");
    assert!(
        guard
            .get_lowering(unknown, Target::Extension("extension-text"))
            .is_none(),
        "Fix: get_lowering must return None when no lowering is registered for the target"
    );
    assert!(
        guard
            .get_lowering(unknown, Target::Extension("extension-binary"))
            .is_none(),
        "Fix: get_lowering must return None when no lowering is registered for the target"
    );
    assert!(
        guard
            .get_lowering(unknown, Target::Extension("extension-native"))
            .is_none(),
        "Fix: get_lowering must return None when no lowering is registered for the target"
    );
    assert!(
        guard
            .get_lowering(unknown, Target::Extension("extension-alt"))
            .is_none(),
        "Fix: get_lowering must return None when no lowering is registered for the target"
    );
}

#[test]
fn duplicate_op_id_error_is_actionable() {
    let defs = vec![
        OpDef {
            id: "dup.op",
            ..OpDef::default()
        },
        OpDef {
            id: "dup.op",
            ..OpDef::default()
        },
    ];
    let err = DialectRegistry::validate_no_duplicates(defs.iter())
        .expect_err("duplicate ids must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("Fix:"),
        "Fix: DuplicateOpIdError must be actionable; got: {msg}"
    );
    assert_eq!(err.op_id(), "dup.op");
    assert_eq!(err.first_registrant(), "<unknown dialect>");
    assert_eq!(err.second_registrant(), "<unknown dialect>");
}

#[test]
fn duplicate_op_id_error_with_dialect_names_is_actionable() {
    let defs = vec![
        OpDef {
            id: "dup.op",
            dialect: "dialect-a",
            ..OpDef::default()
        },
        OpDef {
            id: "dup.op",
            dialect: "dialect-b",
            ..OpDef::default()
        },
    ];
    let err = DialectRegistry::validate_no_duplicates(defs.iter())
        .expect_err("duplicate ids must be rejected");
    assert_eq!(err.first_registrant(), "dialect-a");
    assert_eq!(err.second_registrant(), "dialect-b");
    assert!(err.to_string().contains("Fix:"));
}
