//! Set domain selector test suite.

const SELECTOR: &str = include_str!("../../docs/optimization/SET_DOMAIN_SELECTOR.toml");

#[test]
fn set_domain_selector_covers_representations_and_operations() {
    for required in [
        "bitset",
        "roaring",
        "dense_vector",
        "sparse_matrix",
        "density",
        "churn",
        "intersection",
        "union",
        "difference",
    ] {
        assert!(
            SELECTOR.contains(required),
            "set-domain selector must include {required}"
        );
    }
}
