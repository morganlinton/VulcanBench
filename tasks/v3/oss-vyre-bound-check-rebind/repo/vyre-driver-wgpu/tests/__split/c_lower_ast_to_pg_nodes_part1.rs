use super::*;

#[test]
fn ast_to_pg_nodes_registration_is_witnessed() {
    let witness_inputs = (entry()
        .test_inputs
        .expect("Fix: test_inputs must be pinned"))();
    let witness_expected = (entry()
        .expected_output
        .expect("Fix: expected_output must be pinned"))();
    let program = (entry().build)();
    assert_reference_witnesses(&program, &witness_inputs, &witness_expected);
}

#[test]
fn ast_to_pg_nodes_parity_with_reference_on_witness() {
    let inputs = (entry()
        .test_inputs
        .expect("Fix: test_inputs must be pinned"))();
    let witness = &inputs[0];
    let expected = reference_ast_to_pg_nodes(&witness[0]);
    let program = c_lower_ast_to_pg_nodes(
        "vast_nodes",
        Expr::u32(node_count_from_vast(&witness[0])),
        "out_pg_nodes",
    );
    let output = run_reference_eval(&program, witness);
    assert_eq!(
        output,
        vec![expected],
        "Fix: program output must match CPU reference"
    );
}

#[test]
fn ast_to_pg_nodes_preserves_tree_links() {
    let vast = build_vast(&[
        vec![
            node_kind::FUNCTION_DECL,
            u32::MAX,
            1,
            u32::MAX,
            0,
            0,
            21,
            0,
            0,
            0,
        ],
        vec![node_kind::BASIC_BLOCK, 0, 2, u32::MAX, 0, 10, 11, 0, 0, 0],
        vec![node_kind::CALL, 1, u32::MAX, 3, 0, 12, 5, 0, 0, 0],
        vec![node_kind::LITERAL, 1, u32::MAX, u32::MAX, 0, 18, 1, 0, 0, 0],
    ]);
    let expected = reference_ast_to_pg_nodes(&vast);
    let program = c_lower_ast_to_pg_nodes("vast_nodes", Expr::u32(4), "out_pg_nodes");
    let actual = run_reference_eval(&program, std::slice::from_ref(&vast));
    assert_eq!(actual, vec![expected.clone()]);

    assert_eq!(word_at(&expected, 4), 1, "root first_child must survive");
    assert_eq!(
        word_at(&expected, 6 + 4),
        2,
        "block first_child must survive"
    );
    assert_eq!(
        word_at(&expected, 2 * 6 + 5),
        3,
        "call next_sibling must survive"
    );
}

