use super::*;

pub(super) const VAST_LAST_CHILD_WORKGROUP_MAX_U32: u32 = 12_288;

#[must_use]
pub fn c11_build_vast_nodes_uses_global_last_child(num_tokens: u32) -> bool {
    num_tokens.max(1) > VAST_LAST_CHILD_WORKGROUP_MAX_U32
}

pub fn c11_build_vast_nodes(
    tok_types: &str,
    tok_starts: &str,
    tok_lens: &str,
    num_tokens: Expr,
    out_vast_nodes: &str,
    out_count: &str,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };

    let build_row = Expr::mul(Expr::var("build_i"), Expr::u32(VAST_NODE_STRIDE_U32));
    let parent_row = Expr::mul(Expr::var("parent_idx"), Expr::u32(VAST_NODE_STRIDE_U32));
    let previous_row = Expr::mul(
        Expr::var("previous_sibling"),
        Expr::u32(VAST_NODE_STRIDE_U32),
    );
    let stack_slot = Expr::var("stack_depth");
    let top_slot = Expr::select(
        Expr::gt(Expr::var("stack_depth"), Expr::u32(0)),
        Expr::sub(Expr::var("stack_depth"), Expr::u32(1)),
        Expr::u32(0),
    );

    let parallel_row_init = vec![
        Node::let_bind(
            "parallel_row",
            Expr::mul(t.clone(), Expr::u32(VAST_NODE_STRIDE_U32)),
        ),
        Node::store(
            out_vast_nodes,
            Expr::var("parallel_row"),
            Expr::load(tok_types, t.clone()),
        ),
        Node::store(
            out_vast_nodes,
            Expr::add(Expr::var("parallel_row"), Expr::u32(5)),
            Expr::load(tok_starts, t.clone()),
        ),
        Node::store(
            out_vast_nodes,
            Expr::add(Expr::var("parallel_row"), Expr::u32(6)),
            Expr::load(tok_lens, t.clone()),
        ),
        Node::store(
            out_vast_nodes,
            Expr::add(Expr::var("parallel_row"), Expr::u32(7)),
            Expr::u32(0),
        ),
        Node::store(
            out_vast_nodes,
            Expr::add(Expr::var("parallel_row"), Expr::u32(8)),
            Expr::u32(0),
        ),
    ];

    let build_loop = vec![
        Node::let_bind("row", build_row),
        Node::let_bind("tok", Expr::load(tok_types, Expr::var("build_i"))),
        Node::store(out_vast_nodes, Expr::var("row"), Expr::var("tok")),
        Node::store(
            out_vast_nodes,
            Expr::add(Expr::var("row"), Expr::u32(5)),
            Expr::load(tok_starts, Expr::var("build_i")),
        ),
        Node::store(
            out_vast_nodes,
            Expr::add(Expr::var("row"), Expr::u32(6)),
            Expr::load(tok_lens, Expr::var("build_i")),
        ),
        Node::store(
            out_vast_nodes,
            Expr::add(Expr::var("row"), Expr::u32(VAST_TYPEDEF_FLAGS_FIELD)),
            Expr::u32(0),
        ),
        Node::store(
            out_vast_nodes,
            Expr::add(Expr::var("row"), Expr::u32(VAST_TYPEDEF_SCOPE_FIELD)),
            Expr::u32(0),
        ),
        Node::store(
            out_vast_nodes,
            Expr::add(Expr::var("row"), Expr::u32(VAST_TYPEDEF_SYMBOL_FIELD)),
            Expr::u32(0),
        ),
        Node::let_bind(
            "parent_idx",
            Expr::select(
                Expr::gt(Expr::var("stack_depth"), Expr::u32(0)),
                Expr::load("__vast_stack", top_slot.clone()),
                Expr::u32(SENTINEL),
            ),
        ),
        Node::store(
            out_vast_nodes,
            Expr::add(Expr::var("row"), Expr::u32(1)),
            Expr::var("parent_idx"),
        ),
        Node::store(
            out_vast_nodes,
            Expr::add(Expr::var("row"), Expr::u32(2)),
            Expr::u32(SENTINEL),
        ),
        Node::store(
            out_vast_nodes,
            Expr::add(Expr::var("row"), Expr::u32(3)),
            Expr::u32(SENTINEL),
        ),
        Node::store(
            out_vast_nodes,
            Expr::add(Expr::var("row"), Expr::u32(4)),
            Expr::u32(SENTINEL),
        ),
        // Clamp parent_idx into a safe in-range slot (0) before the
        // speculative load. `Expr::select` evaluates both arms; on PTX the
        // load is unguarded, so feeding SENTINEL through `parent_row` reads
        // way out of bounds → CUDA_ERROR_ILLEGAL_ADDRESS. WGSL's bounds-check
        // policy clamps for us; PTX has no such policy. Clamping here keeps
        // both backends correct without requiring a backend-side bounds-check.
        Node::let_bind(
            "safe_parent_idx",
            Expr::select(
                Expr::lt(Expr::var("parent_idx"), num_tokens.clone()),
                Expr::var("parent_idx"),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(
            "safe_parent_row",
            Expr::mul(
                Expr::var("safe_parent_idx"),
                Expr::u32(VAST_NODE_STRIDE_U32),
            ),
        ),
        Node::let_bind(
            "previous_sibling",
            Expr::select(
                Expr::lt(Expr::var("parent_idx"), num_tokens.clone()),
                Expr::load("__vast_last_child", Expr::var("safe_parent_idx")),
                Expr::var("root_last_child"),
            ),
        ),
        Node::store(
            out_vast_nodes,
            Expr::add(Expr::var("row"), Expr::u32(VAST_PREVIOUS_SIBLING_FIELD)),
            Expr::var("previous_sibling"),
        ),
        Node::if_then_else(
            Expr::lt(Expr::var("previous_sibling"), num_tokens.clone()),
            vec![Node::store(
                out_vast_nodes,
                Expr::add(previous_row, Expr::u32(3)),
                Expr::var("build_i"),
            )],
            vec![Node::if_then(
                Expr::lt(Expr::var("parent_idx"), num_tokens.clone()),
                vec![Node::store(
                    out_vast_nodes,
                    Expr::add(parent_row.clone(), Expr::u32(2)),
                    Expr::var("build_i"),
                )],
            )],
        ),
        Node::if_then_else(
            Expr::lt(Expr::var("parent_idx"), num_tokens.clone()),
            vec![Node::store(
                "__vast_last_child",
                Expr::var("safe_parent_idx"),
                Expr::var("build_i"),
            )],
            vec![Node::assign("root_last_child", Expr::var("build_i"))],
        ),
        // `__attribute__` was previously checked here for the strict
        // GNU `__attribute__((...))` double-parenthesis form, trapping
        // the entire parse on mismatch. Real-world glibc headers
        // (and clang-extension code) use many forms the strict check
        // didn't recognize:
        //   - `__attribute__ ((__pure__))` with whitespace between the
        //     attribute and the paren pair (lexer filters this, fine).
        //   - `__attribute__((...)) __attribute__((...))` chained.
        //   - `__attribute_const__` / `__nonnull` etc. expanding to
        //     `__attribute__ ((...))` via macro  -  should be fine after
        //     preprocessor expansion.
        //   - GCC also accepts `__attribute__()` (empty) and silently
        //     discards it.
        //
        // The downstream VAST classifier (classify/nodes_04..09 and
        // ref_typedef/asm_attributes) knows how to skip or attach
        // attributes wherever they appear. Failing the entire TU on a
        // single misclassified attribute position is the wrong default;
        // real parsers (clang, gcc) recover and continue. Drop the
        // hard trap and let the downstream classifier handle it.
        Node::if_then(
            is_open_token(Expr::var("tok")),
            vec![
                Node::store("__vast_stack", stack_slot, Expr::var("build_i")),
                Node::assign(
                    "stack_depth",
                    Expr::add(Expr::var("stack_depth"), Expr::u32(1)),
                ),
            ],
        ),
        Node::let_bind(
            "top_idx",
            Expr::select(
                Expr::gt(Expr::var("stack_depth"), Expr::u32(0)),
                Expr::load("__vast_stack", top_slot),
                Expr::u32(SENTINEL),
            ),
        ),
        // Same OOB-on-PTX hazard as `safe_parent_idx`: top_idx is SENTINEL
        // when the stack is empty / hasn't been populated, and the speculative
        // tok_types load reads from u32::MAX on an unguarded backend.
        Node::let_bind(
            "safe_top_idx",
            Expr::select(
                Expr::lt(Expr::var("top_idx"), num_tokens.clone()),
                Expr::var("top_idx"),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(
            "top_kind",
            Expr::select(
                Expr::lt(Expr::var("top_idx"), num_tokens.clone()),
                Expr::load(
                    tok_types,
                    Expr::select(
                        Expr::lt(Expr::var("top_idx"), num_tokens.clone()),
                        Expr::var("safe_top_idx"),
                        Expr::u32(0),
                    ),
                ),
                Expr::u32(0),
            ),
        ),
        Node::if_then(
            Expr::and(
                Expr::gt(Expr::var("stack_depth"), Expr::u32(0)),
                is_matching_close(Expr::var("top_kind"), Expr::var("tok")),
            ),
            vec![Node::assign(
                "stack_depth",
                Expr::sub(Expr::var("stack_depth"), Expr::u32(1)),
            )],
        ),
    ];

    let body = vec![
        Node::if_then(Expr::lt(t.clone(), num_tokens.clone()), parallel_row_init),
        Node::if_then(
            Expr::eq(t.clone(), Expr::u32(0)),
            vec![
                Node::store(out_count, Expr::u32(0), num_tokens.clone()),
                Node::let_bind("stack_depth", Expr::u32(0)),
                Node::let_bind("root_last_child", Expr::u32(SENTINEL)),
                Node::loop_for(
                    "last_child_init",
                    Expr::u32(0),
                    num_tokens.clone(),
                    vec![
                        Node::store(
                            "__vast_last_child",
                            Expr::var("last_child_init"),
                            Expr::u32(SENTINEL),
                        ),
                        Node::store(
                            "__vast_stack",
                            Expr::var("last_child_init"),
                            Expr::u32(SENTINEL),
                        ),
                    ],
                ),
                Node::loop_for("build_i", Expr::u32(0), num_tokens.clone(), build_loop),
            ],
        ),
    ];

    let n = node_count(&num_tokens).max(1);
    let last_child_decl = if c11_build_vast_nodes_uses_global_last_child(n) {
        BufferDecl::storage(
            "__vast_last_child",
            5,
            BufferAccess::ReadWrite,
            DataType::U32,
        )
        .with_count(n)
        .with_output_byte_range(0..0)
    } else {
        BufferDecl::workgroup("__vast_last_child", n, DataType::U32)
    };
    let stack_decl = if c11_build_vast_nodes_uses_global_last_child(n) {
        BufferDecl::storage("__vast_stack", 6, BufferAccess::ReadWrite, DataType::U32)
            .with_count(n)
            .with_output_byte_range(0..0)
    } else {
        BufferDecl::workgroup("__vast_stack", n, DataType::U32)
    };
    Program::wrapped(
        vec![
            BufferDecl::storage(tok_types, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(tok_starts, 1, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(tok_lens, 2, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::output(out_vast_nodes, 3, DataType::U32)
                .with_count(n.saturating_mul(VAST_NODE_STRIDE_U32)),
            BufferDecl::storage(out_count, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1)
                .with_pipeline_live_out(true),
            last_child_decl,
            stack_decl,
        ],
        [1, 1, 1],
        vec![wrap_anonymous(
            BUILD_VAST_OP_ID,
            vec![child_phase(
                BUILD_VAST_OP_ID,
                vyre_primitives::parsing::ast_cse_structural_hash::OP_ID,
                body,
            )],
        )],
    )
    .with_entry_op_id(BUILD_VAST_OP_ID)
}
