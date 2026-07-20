//! Dialect duplicate-id gate.
//!
//! See `contracts/release.md`. Today two `inventory::submit!` ops
//! with the same id silently win-or-lose based on link order. This
//! is a class of bug that survives until a customer hits it in
//! prod. Closing the gap means detecting the duplicate at registry
//! init + panicking with `Fix: duplicate op id <name>` before any
//! dispatch can observe the wrong op.

#[test]
fn duplicate_op_id_is_rejected_at_registry_init() {
    let defs = vec![
        vyre_driver::OpDef {
            id: "registry.duplicate",
            ..vyre_driver::OpDef::default()
        },
        vyre_driver::OpDef {
            id: "registry.duplicate",
            ..vyre_driver::OpDef::default()
        },
    ];

    let err = vyre_driver::DialectRegistry::validate_no_duplicates(defs.iter()).expect_err(
        "duplicate-op-id gate: duplicate op ids must be rejected before registry freeze",
    );

    assert_eq!(err.op_id(), "registry.duplicate");
    assert_eq!(err.first_registrant(), "<unknown dialect>");
    assert_eq!(err.second_registrant(), "<unknown dialect>");
    assert!(
        err.to_string().contains(
            "first registrant `<unknown dialect>`, second registrant `<unknown dialect>`"
        ),
        "duplicate-op-id gate: duplicate error must name both registrants, got {err}"
    );
    assert!(
        err.to_string()
            .contains("Fix: keep one owner for this stable id"),
        "duplicate-op-id gate: duplicate error must be actionable, got {err}"
    );
}
