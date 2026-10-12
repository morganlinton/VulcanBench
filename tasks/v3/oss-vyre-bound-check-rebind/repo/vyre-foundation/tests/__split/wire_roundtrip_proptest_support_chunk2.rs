fn arb_node() -> BoxedStrategy<Node> {
    arb_node_with_depth(3)
}

fn arb_node_with_depth(depth: u32) -> BoxedStrategy<Node> {
    let leaf = prop_oneof![
        (arb_ident(), arb_expr()).prop_map(|(name, value)| Node::Let {
            name: name.into(),
            value,
        }),
        (arb_ident(), arb_expr()).prop_map(|(name, value)| Node::Assign {
            name: name.into(),
            value,
        }),
        (
            prop::sample::select(vec!["out", "rw", "bytes_out"]),
            arb_expr(),
            arb_expr(),
        )
            .prop_map(|(buffer, index, value)| Node::Store {
                buffer: buffer.into(),
                index,
                value,
            }),
        Just(Node::Return),
        Just(Node::barrier()),
    ];

    if depth == 0 {
        return leaf.boxed();
    }

    leaf.prop_recursive(3, 64, 3, move |inner| {
        prop_oneof![
            (
                arb_expr(),
                prop_vec(inner.clone(), 0..=3),
                prop_vec(inner.clone(), 0..=3),
            )
                .prop_map(|(cond, then, otherwise)| Node::If {
                    cond,
                    then,
                    otherwise,
                }),
            (
                arb_ident(),
                arb_expr(),
                arb_expr(),
                prop_vec(inner.clone(), 0..=3),
            )
                .prop_map(|(var, from, to, body)| Node::Loop {
                    var: var.into(),
                    from,
                    to,
                    body,
                }),
            prop_vec(inner, 0..=3).prop_map(Node::Block),
        ]
    })
    .boxed()
}

fn arb_program() -> BoxedStrategy<Program> {
    (
        arb_buffer_datatype(),
        arb_buffer_datatype(),
        prop_vec(arb_node(), 0..=6),
        prop_oneof![9 => Just(false), 1 => Just(true)],
    )
        .prop_map(|(extra_a, extra_b, entry, non_composable)| {
            Program::wrapped(
                vec![
                    BufferDecl::output("out", 0, DataType::U32)
                        .with_count(8)
                        .with_output_byte_range(0..16),
                    BufferDecl::read("input", 1, DataType::U32).with_count(8),
                    BufferDecl::read_write("rw", 2, DataType::U32).with_count(8),
                    BufferDecl::read("bytes_in", 3, DataType::Bytes).with_count(16),
                    BufferDecl::read_write("bytes_out", 4, DataType::Bytes).with_count(16),
                    BufferDecl::read("counts", 5, DataType::U32).with_count(8),
                    BufferDecl::workgroup("scratch", 4, DataType::U32),
                    BufferDecl::read("extra_a", 6, extra_a).with_count(1),
                    BufferDecl::read("extra_b", 7, extra_b).with_count(1),
                ],
                [1, 1, 1],
                entry,
            )
            .with_non_composable_with_self(non_composable)
        })
        .boxed()
}

fn first_replaced(bytes: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    assert_eq!(needle.len(), replacement.len());
    let mut mutated = bytes.to_vec();
    let offset = mutated[40..]
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|offset| offset + 40)
        .expect("Fix: expected to find the encoded extension id in the wire body");
    mutated[offset..offset + needle.len()].copy_from_slice(replacement);
    mutated
}

fn first_replaced_with_valid_digest(bytes: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    let mut mutated = first_replaced(bytes, needle, replacement);
    let digest = blake3::hash(&mutated[40..]);
    mutated[8..40].copy_from_slice(digest.as_bytes());
    mutated
}

fn first_let_expr(program: &Program) -> &Expr {
    match top_level_body(program).first() {
        Some(Node::Let { value, .. }) => value,
        other => panic!("Fix: expected first node to be a let binding, got {other:?}"),
    }
}

fn top_level_body(program: &Program) -> &[Node] {
    match program.entry().first() {
        Some(Node::Region { body, .. }) => body.as_slice(),
        _ => program.entry(),
    }
}

/// Mirror of `serial::wire::encode::put_expr::canonical_f32_bits` so
/// this crate's tests can compare against the wire's canonical form
/// without pulling the private encoder helper.
///
/// Wire canonicalization is more aggressive than
/// `vyre_reference::ieee754::canonical_f32`: BOTH subnormal signs
/// AND -0.0 flush to +0.0. NaN payloads collapse to the single
/// positive qNaN (0x7FC0_0000).
fn canonicalize_f32(value: f32) -> f32 {
    if value.is_nan() {
        return f32::from_bits(0x7FC0_0000);
    }
    if value.is_subnormal() {
        return 0.0_f32;
    }
    if value.to_bits() == (-0.0_f32).to_bits() {
        return 0.0_f32;
    }
    value
}

