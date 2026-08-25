//! Performance contracts for the C VAST structural builder.

#[test]
fn vast_builder_uses_single_stack_pass_not_per_token_reverse_scans() {
    let source = include_str!("../src/parsing/c/parse/vast/build/structural_builder.rs");
    let start = source
        .find("pub fn c11_build_vast_nodes")
        .expect("VAST builder must remain present");
    let end = source[start..]
        .find(".with_entry_op_id(BUILD_VAST_OP_ID)")
        .map(|offset| start + offset)
        .expect("VAST builder entry op marker must remain present");
    let builder = &source[start..end];

    assert!(
        builder.contains("\"stack_depth\"") && builder.contains("\"root_last_child\""),
        "VAST builder must keep structural parent/sibling recovery in one stack pass"
    );
    assert!(
        !builder.contains("close_scan") && !builder.contains("candidate_sibling"),
        "VAST builder must not regress to nested close/sibling scans"
    );
    assert!(
        builder.contains("[1, 1, 1]"),
        "VAST builder is a single linear GPU setup pass; do not relaunch one invocation per token"
    );
}
