use super::*;

/// Reproducer that mirrors fnv1a64's structure exactly: triple-Region
/// wrap -> if-then(gid==0) -> let-bind state -> Loop bounded by
/// BufLen(input) -> body assigns to outer state -> after-loop Store.
/// fnv1a64's catalog form returns the unchanged FNV1A64_OFFSET on
/// GPU (loop body never runs), but the Q3 wgpu fix made arrayLength
/// correct for the simpler tests above.
fn fnv1a64_shaped_count_program() -> Program {
    // Pattern: outer state `n` initialised to 0, loop runs `buf_len(input)`
    // iterations, each iteration does `n = n + 1`. Final n stored to out[0].
    // For a 4-byte input, expect out[0] = 1.
    let inner = vec![Node::if_then(
        Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
        vec![
            Node::let_bind("n", Expr::u32(0)),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::buf_len("input"),
                vec![
                    // Mirror fnv1a64's pattern: read input[i], use it,
                    // assign back to outer state via Var.
                    Node::let_bind(
                        "byte",
                        Expr::bitand(Expr::load("input", Expr::var("i")), Expr::u32(0xFF)),
                    ),
                    Node::let_bind("next", Expr::add(Expr::var("n"), Expr::u32(1))),
                    Node::assign("n", Expr::var("next")),
                    // The byte let must survive even if unused by `n`.
                    Node::let_bind("_swallow", Expr::var("byte")),
                ],
            ),
            Node::store("out", Expr::u32(0), Expr::var("n")),
        ],
    )];
    let mid = Node::Region {
        generator: Ident::from("vyre-primitives::test::fnv_shape_inner"),
        source_region: None,
        body: Arc::new(inner),
    };
    let outer = Node::Region {
        generator: Ident::from("vyre-primitives::test::fnv_shape_mid"),
        source_region: Some(GeneratorRef {
            name: "vyre-libs::catalog::test::fnv_shape_outer".to_string(),
        }),
        body: Arc::new(vec![mid]),
    };
    let body = Node::Region {
        generator: Ident::from("vyre-libs::catalog::test::fnv_shape_outer"),
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

#[test]
fn fnv1a64_shaped_loop_runs_once_for_one_byte_input() {
    let program = fnv1a64_shaped_count_program();
    let observed = dispatch_and_read_first_word_lowered(&program, vec![0xAB, 0, 0, 0]);
    assert_eq!(
        observed, 1,
        "Q3: a fnv1a64-shaped loop (BufLen-bounded, with outer-state assign) must iterate once for a 4-byte input, got {observed}. \
         If this fails while the simpler buf_len tests pass, the bug is in how the loop body's outer-scope assigns interact with BufLen lowering."
    );
}

#[test]
fn fnv1a64_shaped_loop_runs_three_times_for_twelve_byte_input() {
    let program = fnv1a64_shaped_count_program();
    let observed =
        dispatch_and_read_first_word_lowered(&program, vec![1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0]);
    assert_eq!(
        observed, 3,
        "Q3: a fnv1a64-shaped loop must iterate three times for a 12-byte input, got {observed}."
    );
}
