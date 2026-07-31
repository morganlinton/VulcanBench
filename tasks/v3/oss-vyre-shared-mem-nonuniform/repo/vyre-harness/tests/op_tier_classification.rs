//! Op-id tier classification contracts for harness routing.

use vyre_harness::{classify_op_id, OpTier};

#[test]
fn classify_known_vyre_namespaces_without_consumer_coupling() {
    assert_eq!(
        classify_op_id("vyre-intrinsics::hardware::popcount_u32"),
        OpTier::Intrinsic
    );
    assert_eq!(
        classify_op_id("vyre-primitives::graph::toposort"),
        OpTier::Primitive
    );
    assert_eq!(classify_op_id("vyre-libs::scan::literal_set"), OpTier::Libs);
    assert_eq!(classify_op_id("core.dispatch"), OpTier::Runtime);
}

#[test]
fn classify_external_crate_namespaces_generically() {
    assert_eq!(
        classify_op_id("external_frontend::analysis::dataflow"),
        OpTier::External
    );
    assert_eq!(
        classify_op_id("community_pack::scan::signature"),
        OpTier::External
    );
}

#[test]
fn classify_unknown_ids_without_namespace_as_unknown() {
    assert_eq!(classify_op_id("not_a_namespace"), OpTier::Unknown);
    assert_eq!(classify_op_id(""), OpTier::Unknown);
}
