//! Contract tests: the Rust borrow-fact delta registry carries relation provenance.

const DELTAS: &str = include_str!("../../docs/optimization/RUST_BORROW_FACT_DELTAS.toml");

#[test]
fn rust_borrow_fact_deltas_include_relation_provenance() {
    for required in [
        "borrow",
        "region",
        "loan",
        "alias",
        "escape",
        "source_span",
        "provenance",
        "diagnostic_parity",
    ] {
        assert!(
            DELTAS.contains(required),
            "Rust borrow fact delta registry must include {required}"
        );
    }

    assert!(DELTAS.contains("matches-rustc"));
    assert!(DELTAS.contains("matches-rustc-negative"));
}
