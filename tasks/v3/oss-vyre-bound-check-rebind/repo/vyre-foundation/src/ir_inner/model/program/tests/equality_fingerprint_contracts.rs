use super::*;

#[test]
fn partial_eq_ignores_buffer_declaration_order() {
    let left = Program::wrapped(
        vec![
            BufferDecl::output("out", 0, DataType::U32).with_count(1),
            BufferDecl::read("input", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        sample_body(),
    );
    let right = Program::wrapped(
        vec![
            BufferDecl::read("input", 1, DataType::U32).with_count(1),
            BufferDecl::output("out", 0, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        sample_body(),
    );

    assert_eq!(
        left, right,
        "Fix: Program equality must ignore buffer declaration order."
    );
    assert!(
        left.structural_eq(&right),
        "Fix: structural_eq must agree with PartialEq on reordered buffers."
    );
}

#[test]
fn structural_eq_rejects_semantic_entry_differences() {
    let left = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(7)), Node::Return],
    );
    let right = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(9)), Node::Return],
    );

    assert!(
        !left.structural_eq(&right),
        "Fix: structural_eq must reject programs whose observable writes differ."
    );
}

#[test]
fn canonical_fingerprint_normalizes_commutative_literal_order() {
    let left = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::add(Expr::u32(7), Expr::var("x")),
        )],
    );
    let right = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::add(Expr::var("x"), Expr::u32(7)),
        )],
    );

    assert_ne!(
        left.to_wire().expect("Fix: left fixture must encode"),
        right.to_wire().expect("Fix: right fixture must encode"),
        "Fix: this regression test must exercise distinct author wire forms."
    );
    assert_eq!(
        left.fingerprint(),
        right.fingerprint(),
        "Fix: canonical Program fingerprint must ignore commutative literal spelling."
    );
}

#[test]
fn canonical_fingerprint_normalizes_safe_commutative_nonliteral_order() {
    let left = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::bitxor(Expr::var("a"), Expr::var("b")),
        )],
    );
    let right = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::bitxor(Expr::var("b"), Expr::var("a")),
        )],
    );

    assert_eq!(
        left.fingerprint(),
        right.fingerprint(),
        "Fix: canonical Program fingerprint must sort safe commutative operands."
    );
}

#[test]
fn canonical_fingerprint_preserves_float_sensitive_nonliteral_order() {
    for (left_value, right_value) in [
        (
            Expr::add(Expr::var("a"), Expr::var("b")),
            Expr::add(Expr::var("b"), Expr::var("a")),
        ),
        (
            Expr::mul(Expr::var("a"), Expr::var("b")),
            Expr::mul(Expr::var("b"), Expr::var("a")),
        ),
        (
            Expr::min(Expr::var("a"), Expr::var("b")),
            Expr::min(Expr::var("b"), Expr::var("a")),
        ),
        (
            Expr::max(Expr::var("a"), Expr::var("b")),
            Expr::max(Expr::var("b"), Expr::var("a")),
        ),
    ] {
        let left = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), left_value)],
        );
        let right = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), right_value)],
        );

        assert_ne!(
            left.fingerprint(),
            right.fingerprint(),
            "Fix: canonical Program fingerprint must preserve order for float-sensitive ops."
        );
    }
}

#[test]
fn canonical_wire_hash_is_blake3_of_canonical_wire_bytes() {
    let program = Program::wrapped(
        vec![
            BufferDecl::read("input", 1, DataType::U32).with_count(1),
            BufferDecl::output("out", 0, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::bitxor(Expr::var("b"), Expr::var("a")),
        )],
    );
    let canonical_wire = program
        .canonical_wire_bytes()
        .expect("Fix: canonical fixture must encode");
    let expected = *blake3::hash(&canonical_wire).as_bytes();

    assert_eq!(program.fingerprint(), expected);
    assert_eq!(
        *program
            .canonical_wire_hash()
            .expect("Fix: canonical fixture must hash")
            .as_bytes(),
        expected
    );
}

#[test]
fn canonical_wire_hash_normalizes_float_payload_noise() {
    let nan_a = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::let_bind("x", Expr::f32(f32::from_bits(0x7FC1_2345)))],
    );
    let nan_b = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::let_bind("x", Expr::f32(f32::from_bits(0x7FA0_0001)))],
    );
    let subnormal = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::let_bind("x", Expr::f32(f32::from_bits(1)))],
    );
    let zero = Program::wrapped(
        Vec::new(),
        [1, 1, 1],
        vec![Node::let_bind("x", Expr::f32(0.0))],
    );

    assert_eq!(nan_a.fingerprint(), nan_b.fingerprint());
    assert_eq!(subnormal.fingerprint(), zero.fingerprint());
}

#[test]
fn canonical_fingerprint_flattens_binding_free_nested_blocks() {
    let nested = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::block(vec![Node::block(vec![Node::store(
            "out",
            Expr::u32(0),
            Expr::u32(1),
        )])])],
    );
    let flat = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
    );

    assert_eq!(
        nested.fingerprint(),
        flat.fingerprint(),
        "Fix: canonical Program fingerprint must flatten binding-free Block wrappers."
    );
}

#[test]
fn canonical_fingerprint_preserves_binding_block_scope() {
    let scoped = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::block(vec![Node::let_bind("x", Expr::u32(1))])],
    );
    let leaked = Program::wrapped(
        vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::let_bind("x", Expr::u32(1))],
    );

    assert_ne!(
        scoped.fingerprint(),
        leaked.fingerprint(),
        "Fix: canonicalization must not flatten Blocks that own local bindings."
    );
}

#[test]
fn canonical_fingerprint_distinguishes_subgroup_reduce_ops() {
    // The structural fingerprint folds the SubgroupReduceOp wire tag (meta.rs's
    // `Expr::SubgroupReduce` arm: `h.update(&[op.builtin_wire_tag()])`). If that
    // fold were dropped, CSE would merge e.g. `subgroup_max(x)` with
    // `subgroup_add(x)` into ONE reduction — a silent miscompile. Every distinct
    // reduce op over the SAME operand must therefore hash distinctly.
    let ops: [(&str, fn(Expr) -> Expr); 7] = [
        ("add", Expr::subgroup_add),
        ("mul", Expr::subgroup_mul),
        ("min", Expr::subgroup_min),
        ("max", Expr::subgroup_max),
        ("and", Expr::subgroup_and),
        ("or", Expr::subgroup_or),
        ("xor", Expr::subgroup_xor),
    ];
    let mut seen: Vec<(&str, [u8; 32])> = Vec::new();
    for (name, ctor) in ops {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), ctor(Expr::var("x")))],
        );
        let fingerprint = program.fingerprint();
        for (other_name, other_fingerprint) in &seen {
            assert_ne!(
                fingerprint, *other_fingerprint,
                "Fix: subgroup_{name} and subgroup_{other_name} must have distinct \
                 fingerprints — the structural hash must fold the reduce op, else CSE \
                 merges different reductions into one."
            );
        }
        seen.push((name, fingerprint));
    }
    assert_eq!(seen.len(), 7, "all seven subgroup reduce ops must be fingerprinted");
}

