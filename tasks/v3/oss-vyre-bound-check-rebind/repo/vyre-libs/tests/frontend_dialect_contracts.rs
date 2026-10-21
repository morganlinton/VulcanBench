//! Frontend dialect contracts test suite.

const DIALECTS: &str = include_str!("../../docs/optimization/FRONTEND_DIALECT_CONTRACTS.toml");

#[test]
fn frontend_dialect_contracts_require_versions_and_fallbacks() {
    for required in [
        "language",
        "version",
        "extension_flags",
        "parser_feature_gates",
        "unsupported_syntax_diagnostics",
        "fallback_route",
    ] {
        assert!(
            DIALECTS.contains(required),
            "frontend dialect contract must expose {required}"
        );
    }

    assert!(DIALECTS.contains("rust-2021-default"));
    assert!(DIALECTS.contains("gnu11-c"));
    assert!(DIALECTS.contains("python-3-12"));
}
