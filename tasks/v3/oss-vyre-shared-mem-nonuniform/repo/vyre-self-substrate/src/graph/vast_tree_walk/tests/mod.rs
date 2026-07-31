use super::*;
use vyre_primitives::graph::vast_tree_walk::{
    try_ast_walk_postorder, try_ast_walk_preorder, POSTORDER_OP_ID, PREORDER_OP_ID,
};

#[test]
fn checked_plan_builds_preorder_and_postorder_programs() {
    let plan = build_vast_tree_walk_plan("vast_nodes", "pre", "post", 4, 4)
        .expect("Fix: valid VAST tree walk dimensions must build.");

    assert_eq!(plan.preorder.workgroup_size(), [1, 1, 1]);
    assert_eq!(plan.postorder.workgroup_size(), [1, 1, 1]);
    assert_eq!(plan.preorder.buffers().len(), 2);
    assert_eq!(plan.postorder.buffers().len(), 2);
    assert!(!plan.preorder.entry().is_empty());
    assert!(!plan.postorder.entry().is_empty());
}

#[test]
fn checked_plan_rejects_zero_traversal_capacity() {
    let error = build_vast_tree_walk_plan("vast_nodes", "pre", "post", 4, 0)
        .expect_err("Fix: zero output capacity must stay rejected.");

    assert!(
        error.contains("out_cap > 0"),
        "Fix: primitive diagnostic must explain output capacity failure, got: {error}"
    );
}

#[test]
fn checked_wrappers_matches_primitive_directly_for_valid_layout() {
    let wrapper = build_checked_preorder_walk("vast_nodes", "pre", 4, 4)
        .expect("Fix: wrapper preorder builder must accept valid dimensions.");
    let primitive = try_ast_walk_preorder("vast_nodes", "pre", 4, 4)
        .expect("Fix: primitive preorder builder must accept valid dimensions.");

    assert_eq!(wrapper.workgroup_size(), primitive.workgroup_size());
    assert_eq!(wrapper.buffers().len(), primitive.buffers().len());
    assert_eq!(wrapper.entry().len(), primitive.entry().len());
}

#[test]
fn generated_shape_matrix_equals_primitive_for_both_orders() {
    for (node_count, traversal_capacity) in [(1, 1), (2, 1), (2, 2), (8, 3), (8, 8), (64, 17)] {
        let preorder =
            build_checked_preorder_walk("vast_nodes", "pre", node_count, traversal_capacity)
                .expect("Fix: wrapper preorder builder must accept generated valid dimensions.");
        let primitive_preorder =
            try_ast_walk_preorder("vast_nodes", "pre", node_count, traversal_capacity)
                .expect("Fix: primitive preorder builder must accept generated valid dimensions.");
        assert_eq!(
            preorder.workgroup_size(),
            primitive_preorder.workgroup_size()
        );
        assert_eq!(preorder.buffers().len(), primitive_preorder.buffers().len());
        assert_eq!(preorder.entry().len(), primitive_preorder.entry().len());

        let postorder =
            build_checked_postorder_walk("vast_nodes", "post", node_count, traversal_capacity)
                .expect("Fix: wrapper postorder builder must accept generated valid dimensions.");
        let primitive_postorder =
            try_ast_walk_postorder("vast_nodes", "post", node_count, traversal_capacity)
                .expect("Fix: primitive postorder builder must accept generated valid dimensions.");
        assert_eq!(
            postorder.workgroup_size(),
            primitive_postorder.workgroup_size()
        );
        assert_eq!(
            postorder.buffers().len(),
            primitive_postorder.buffers().len()
        );
        assert_eq!(postorder.entry().len(), primitive_postorder.entry().len());
    }
}

#[test]
fn trusted_builders_still_consume_primitive_op_ids() {
    let preorder = build_trusted_preorder_walk("vast_nodes", "pre", 3, 3);
    let postorder = build_trusted_postorder_walk("vast_nodes", "post", 3, 3);
    let ids = primitive_op_ids();

    assert_eq!(ids, [PREORDER_OP_ID, POSTORDER_OP_ID]);
    assert!(!preorder.entry().is_empty());
    assert!(!postorder.entry().is_empty());
}

#[test]
fn trusted_builders_fail_fast_instead_of_returning_inert_programs() {
    let preorder = std::panic::catch_unwind(|| {
        let _ = build_trusted_preorder_walk("vast_nodes", "pre", 3, 0);
    })
    .expect_err("Fix: invalid trusted preorder shape must fail fast.");
    let postorder = std::panic::catch_unwind(|| {
        let _ = build_trusted_postorder_walk("vast_nodes", "post", u32::MAX, 1);
    })
    .expect_err("Fix: invalid trusted postorder shape must fail fast.");

    let preorder_message = panic_message(preorder);
    let postorder_message = panic_message(postorder);
    assert!(
        preorder_message.contains("trusted VAST preorder walk shape was not prevalidated"),
        "Fix: trusted preorder panic must name the violated prevalidation contract, got: {preorder_message}"
    );
    assert!(
        postorder_message.contains("trusted VAST postorder walk shape was not prevalidated"),
        "Fix: trusted postorder panic must name the violated prevalidation contract, got: {postorder_message}"
    );
}

#[test]
fn production_facade_does_not_import_primitive_builders_directly() {
    let facade = include_str!("../mod.rs");
    assert!(!facade.contains("try_ast_walk_preorder"));
    assert!(!facade.contains("try_ast_walk_postorder"));
    assert!(!facade.contains("ast_walk_preorder"));
    assert!(!facade.contains("ast_walk_postorder"));
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&'static str>() {
        (*message).to_string()
    } else {
        "<non-string panic>".to_string()
    }
}
