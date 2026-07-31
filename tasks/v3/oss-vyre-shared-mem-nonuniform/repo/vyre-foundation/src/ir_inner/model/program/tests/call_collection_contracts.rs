use super::*;

#[test]
fn collect_call_op_ids_preserves_first_appearance_order() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::let_bind("a", Expr::call("alpha.op", vec![Expr::u32(1)])),
            Node::let_bind("b", Expr::call("beta.op", vec![Expr::u32(2)])),
            Node::let_bind("c", Expr::call("gamma.op", vec![Expr::u32(3)])),
            Node::Return,
        ],
    );
    let ids: Vec<String> = collect_call_op_ids(&program)
        .into_iter()
        .map(|id| id.to_string())
        .collect();
    assert_eq!(
        ids,
        vec![
            "alpha.op".to_string(),
            "beta.op".to_string(),
            "gamma.op".to_string(),
        ]
    );
}

#[test]
fn collect_call_op_ids_shares_arc_for_duplicate_op_identifiers() {
    let shared = Ident::new(Arc::from("vyre.test.duplicate.call"));
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![
            Node::let_bind("a", Expr::call(shared.clone(), vec![Expr::u32(1)])),
            Node::let_bind("b", Expr::call(shared, vec![Expr::u32(2)])),
            Node::Return,
        ],
    );
    let ids = collect_call_op_ids(&program);
    assert_eq!(ids.len(), 2);
    assert!(Arc::ptr_eq(&ids[0], &ids[1]));
}

#[test]
fn fingerprint_matches_across_clone_when_canonical_wire_encode_rejects_workgroup() {
    let program = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [0, 0, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1)), Node::Return],
    );
    assert!(
        program.canonical_wire_hash().is_err(),
        "Fixture must exercise canonical wire rejection before fallback hashing."
    );
    let clone = program.clone();
    assert_eq!(program.fingerprint(), clone.fingerprint());
}

