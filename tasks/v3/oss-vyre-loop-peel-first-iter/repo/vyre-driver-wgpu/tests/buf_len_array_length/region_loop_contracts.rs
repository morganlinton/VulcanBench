use super::*;

/// Wrap the buf_len writer body in three nested Region nodes to mirror
/// the shape `primitive_catalog::primitive_program` builds for
/// `catalog::hash::fnv1a64::consumer_a/b`. If `arrayLength` works on
/// the flat program but not on the deeply-wrapped one, the bug is in
/// region inlining or pre-lowering rather than in the wgpu binding.
fn deep_region_wrapped_buf_len_program() -> Program {
    let inner = vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![Node::store("out", Expr::u32(0), Expr::buf_len("input"))],
    )];
    let mid = Node::Region {
        generator: Ident::from("vyre-primitives::test::buf_len_inner"),
        source_region: None,
        body: Arc::new(inner),
    };
    let outer = Node::Region {
        generator: Ident::from("vyre-primitives::test::buf_len_mid"),
        source_region: Some(GeneratorRef {
            name: "vyre-libs::catalog::test::buf_len_outer".to_string(),
        }),
        body: Arc::new(vec![mid]),
    };
    let body = Node::Region {
        generator: Ident::from("vyre-libs::catalog::test::buf_len_outer"),
        source_region: None,
        body: Arc::new(vec![outer]),
    };
    Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![body],
    )
}

fn loop_counting_buf_len_program() -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("seen", Expr::u32(0)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::buf_len("input"),
                    vec![Node::assign(
                        "seen",
                        Expr::add(Expr::var("seen"), Expr::u32(1)),
                    )],
                ),
                Node::store("out", Expr::u32(0), Expr::var("seen")),
            ],
        )],
    )
}

#[test]
fn buf_len_through_three_region_wraps_for_one_element() {
    let program = deep_region_wrapped_buf_len_program();
    let observed = dispatch_and_read_first_word(&program, vec![0x99, 0, 0, 0]);
    assert_eq!(
        observed, 1,
        "Q3: arrayLength on a triple-Region-wrapped Program must report 1 for a 4-byte input, got {observed}. \
         If this fails while the flat-program tests pass, region inlining or pre-lowering is breaking the BufLen path \
         in catalog wrappers  -  see ROADMAP.md Q3."
    );
}

#[test]
fn buf_len_through_three_region_wraps_for_three_elements() {
    let program = deep_region_wrapped_buf_len_program();
    let observed = dispatch_and_read_first_word(&program, vec![1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0]);
    assert_eq!(
        observed, 3,
        "Q3: arrayLength on a triple-Region-wrapped Program must report 3 for a 12-byte input, got {observed}."
    );
}

#[test]
fn buf_len_through_three_region_wraps_through_pre_lowering_for_one_element() {
    // The cat_a_gpu_differential test path runs every program through
    // `vyre_foundation::optimizer::pre_lowering::optimize` before
    // dispatch. If buf_len works on the flat or shallow-wrapped path
    // but breaks here, the regression lives in the optimizer pipeline
    // (canonicalize → region_inline → const_fold → loop_unroll →
    // strength_reduce → normalize_atomics → CSE+DCE → ...).
    let program = deep_region_wrapped_buf_len_program();
    let observed = dispatch_and_read_first_word_lowered(&program, vec![0x99, 0, 0, 0]);
    assert_eq!(
        observed, 1,
        "Q3: arrayLength after pre_lowering::optimize on a triple-Region-wrapped Program must report 1 for a 4-byte input, got {observed}. \
         If this fails while the pre-lowering-skipping tests pass, an optimizer pass is folding `Expr::buf_len` to a constant  -  see ROADMAP.md Q3."
    );
}

#[test]
fn buf_len_through_three_region_wraps_through_pre_lowering_for_three_elements() {
    let program = deep_region_wrapped_buf_len_program();
    let observed =
        dispatch_and_read_first_word_lowered(&program, vec![1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0]);
    assert_eq!(
        observed, 3,
        "Q3: arrayLength after pre_lowering::optimize on a triple-Region-wrapped Program must report 3 for a 12-byte input, got {observed}."
    );
}

#[test]
fn buf_len_loop_bound_survives_pre_lowering() {
    let program = loop_counting_buf_len_program();
    let observed =
        dispatch_and_read_first_word_lowered(&program, vec![1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0]);
    assert_eq!(
        observed, 3,
        "Q3: a loop bounded by dynamic buf_len(input) must execute once per bound element after pre_lowering, got {observed}."
    );
}
