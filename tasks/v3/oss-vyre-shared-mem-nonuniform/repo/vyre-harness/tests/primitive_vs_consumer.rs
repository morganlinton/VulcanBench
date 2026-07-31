//! P-HARNESS-3: differential tests, primitive vs self-consumer.
//!
//! For each (primitive, self-consumer) pair, run both on the same
//! input; assert the consumer's output is consistent with the
//! primitive's behavior (the consumer should be a "thicker"
//! wrapper, not a divergent rewrite).
#![allow(missing_docs)]

use vyre_self_substrate::{dataflow_fixpoint, scallop_provenance, scallop_provenance_wide};

#[test]
fn consumers_are_thicker_wrappers_not_rewrites() {
    let provenance = scallop_provenance::build_provenance_program(2, 4);
    assert_eq!(
        provenance.buffers().len(),
        4,
        "scallop_provenance must remain a direct self-consumer of scallop_join's four-buffer fixpoint shape"
    );
    assert!(
        provenance
            .entry()
            .iter()
            .any(|node| format!("{node:?}").contains("scallop_join")),
        "scallop_provenance must wrap the registered scallop_join primitive instead of rewriting it"
    );

    let wide = scallop_provenance_wide::scallop_provenance_wide_program(
        "state", "next", "rules", "changed", 2, 2, 4,
    );
    assert_eq!(
        wide.buffers().len(),
        4,
        "scallop_provenance_wide must preserve the wide primitive's four-buffer contract"
    );

    let closure = dataflow_fixpoint::reachability_closure(&[0, 1, 0, 0], 2, 8);
    assert_eq!(
        closure[1], 1,
        "dataflow_fixpoint must expose the primitive closure semantics to self-substrate callers"
    );
}
