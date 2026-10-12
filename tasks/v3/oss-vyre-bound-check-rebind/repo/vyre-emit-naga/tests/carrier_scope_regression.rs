//! Regression tests for the Q7 carrier mechanism. Every shape here
//! caused a "no definition in scope for identifier" naga validation
//! failure at some point during the loop-carry-SSA work; each test
//! pins the fix so a future refactor that re-introduces a scope leak
//! fails loudly here instead of in the integration pipeline.
//!
//! Each test:
//! 1. Builds a vyre `Program` with a specific control-flow shape.
//! 2. Runs `vyre::optimize` → `vyre_lower::lower_for_emit`.
//! 3. Calls `vyre_emit_naga::emit` to produce a `naga::Module`.
//! 4. Calls `naga::valid::Validator` with all flags + capabilities.
//! 5. Asserts validation succeeds (no NotInScope, no DanglingResultRef,
//!    no InvalidStoreTypes, etc.).
//!
//! If you change carrier emission, run this whole file. A failure
//! identifies the exact shape that regressed.

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

fn assert_emits_clean(prog: Program, label: &str) {
    // lower_for_emit calls prepare_program_for_emit internally,
    // which runs the optimizer. Pass the original Program.
    let lk = vyre_lower::lower_for_emit(&prog)
        .unwrap_or_else(|e| panic!("{label}: lower_for_emit failed: {e}"));
    let module = vyre_emit_naga::emit(&lk.descriptor)
        .unwrap_or_else(|e| panic!("{label}: emit failed: {e}"));
    let res = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module);
    if let Err(err) = res {
        panic!("{label}: naga validation failed: {err:?}");
    }
}

fn out_count_buf() -> BufferDecl {
    BufferDecl::storage("out", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1)
}

fn flag_buf() -> BufferDecl {
    BufferDecl::storage("flag", 0, BufferAccess::ReadOnly, DataType::U32).with_count(8)
}

#[test]
fn loop_carrier_with_assign_inside_if_then() {
    // The smoke-test shape that originally broke: var assigned inside
    // a nested if-then within a loop body. Carrier mechanism must
    // route the in-loop reads through a function-local.
    let prog = Program::wrapped(
        vec![flag_buf(), out_count_buf()],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("counter", Expr::u32(0)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::u32(8),
                    vec![
                        Node::let_bind("v", Expr::load("flag", Expr::var("i"))),
                        Node::if_then(
                            Expr::eq(Expr::var("v"), Expr::u32(1)),
                            vec![Node::assign(
                                "counter",
                                Expr::add(Expr::var("counter"), Expr::u32(1)),
                            )],
                        ),
                    ],
                ),
                Node::store("out", Expr::u32(0), Expr::var("counter")),
            ],
        )],
    );
    assert_emits_clean(prog, "loop_carrier_with_assign_inside_if_then");
}

#[test]
fn if_then_merge_select_visible_to_subsequent_if_in_same_body() {
    // After my merge_if_then_scope fires for `if_then(..., assign x)`,
    // the resulting Select binds `x`. A subsequent `if_then(..., read x)`
    // in the SAME parent body must see that Select's value. Originally
    // the Select's operand was an SSA produced inside the first if's
    // child block, and naga rejected the second if's body reading it.
    let prog = Program::wrapped(
        vec![flag_buf(), out_count_buf()],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("byte", Expr::load("flag", Expr::u32(0))),
                Node::let_bind("emit", Expr::u32(0)),
                Node::if_then(
                    Expr::ne(Expr::var("byte"), Expr::u32(0)),
                    vec![Node::assign("emit", Expr::u32(1))],
                ),
                Node::if_then(
                    Expr::eq(Expr::var("emit"), Expr::u32(1)),
                    vec![Node::store("out", Expr::u32(0), Expr::u32(42))],
                ),
            ],
        )],
    );
    assert_emits_clean(prog, "if_then_merge_select_visible_to_subsequent_if");
}

#[test]
fn loop_with_assign_inside_two_sequential_if_thens() {
    // The lex-shape that failed at n_tokens=29: a loop body with two
    // sequential if-thens, both assigning the same var. The merge
    // Selects from each if must thread through the loop carrier so
    // post-loop reads see the accumulated value.
    let prog = Program::wrapped(
        vec![flag_buf(), out_count_buf()],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("acc", Expr::u32(0)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::u32(8),
                    vec![
                        Node::let_bind("v", Expr::load("flag", Expr::var("i"))),
                        Node::if_then(
                            Expr::eq(Expr::var("v"), Expr::u32(1)),
                            vec![Node::assign(
                                "acc",
                                Expr::add(Expr::var("acc"), Expr::u32(1)),
                            )],
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var("v"), Expr::u32(2)),
                            vec![Node::assign(
                                "acc",
                                Expr::add(Expr::var("acc"), Expr::u32(10)),
                            )],
                        ),
                    ],
                ),
                Node::store("out", Expr::u32(0), Expr::var("acc")),
            ],
        )],
    );
    assert_emits_clean(prog, "loop_with_assign_inside_two_sequential_if_thens");
}

#[test]
fn if_then_else_merge_select_no_scope_leak() {
    // if-then-else variant of the merge phi. The Select after the
    // if-then-else has operands produced in BOTH branches. Both must
    // be visible at the parent scope.
    let prog = Program::wrapped(
        vec![flag_buf(), out_count_buf()],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("byte", Expr::load("flag", Expr::u32(0))),
                Node::let_bind("acc", Expr::u32(0)),
                Node::if_then_else(
                    Expr::eq(Expr::var("byte"), Expr::u32(1)),
                    vec![Node::assign("acc", Expr::u32(7))],
                    vec![Node::assign("acc", Expr::u32(13))],
                ),
                Node::store("out", Expr::u32(0), Expr::var("acc")),
            ],
        )],
    );
    assert_emits_clean(prog, "if_then_else_merge_select");
}

#[test]
fn structured_block_carries_assigns_to_sibling_op() {
    // A var assigned inside a `Node::Block` body and read after the
    // Block (which lowers to `KernelOpKind::StructuredBlock` and emits
    // as a naga `Statement::Block`). Without the StructuredBlock
    // carrier extension, the post-block Select referenced an SSA in
    // the sibling block.
    let prog = Program::wrapped(
        vec![flag_buf(), out_count_buf()],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("byte", Expr::load("flag", Expr::u32(0))),
                Node::let_bind("acc", Expr::u32(0)),
                Node::Block(vec![Node::if_then(
                    Expr::ne(Expr::var("byte"), Expr::u32(0)),
                    vec![Node::assign("acc", Expr::u32(99))],
                )]),
                Node::if_then(
                    Expr::eq(Expr::var("acc"), Expr::u32(99)),
                    vec![Node::store("out", Expr::u32(0), Expr::var("acc"))],
                ),
            ],
        )],
    );
    assert_emits_clean(prog, "structured_block_carries_assigns_to_sibling_op");
}

#[test]
fn nested_loop_inner_assign_outer_read() {
    // Outer loop has var `count`, inner loop assigns it, post-outer-loop
    // reads it. Both loop carriers must compose correctly.
    let prog = Program::wrapped(
        vec![flag_buf(), out_count_buf()],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("count", Expr::u32(0)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::u32(4),
                    vec![Node::loop_for(
                        "j",
                        Expr::u32(0),
                        Expr::u32(2),
                        vec![Node::assign(
                            "count",
                            Expr::add(Expr::var("count"), Expr::u32(1)),
                        )],
                    )],
                ),
                Node::store("out", Expr::u32(0), Expr::var("count")),
            ],
        )],
    );
    assert_emits_clean(prog, "nested_loop_inner_assign_outer_read");
}

#[test]
fn if_then_else_branches_dont_leak_lets_to_sibling_branch() {
    // Pattern triggered by lex's nested if-else chains: an if-else
    // inside a loop body where BOTH branches assign the same var.
    // The carrier mechanism must rebind values BETWEEN branches so
    // the else block doesn't read a let bound in the then block.
    let prog = Program::wrapped(
        vec![flag_buf(), out_count_buf()],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("acc", Expr::u32(0)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::u32(8),
                    vec![
                        Node::let_bind("v", Expr::load("flag", Expr::var("i"))),
                        Node::if_then_else(
                            Expr::eq(Expr::var("v"), Expr::u32(1)),
                            vec![Node::assign(
                                "acc",
                                Expr::add(Expr::var("acc"), Expr::u32(2)),
                            )],
                            vec![Node::assign(
                                "acc",
                                Expr::add(Expr::var("acc"), Expr::u32(3)),
                            )],
                        ),
                    ],
                ),
                Node::store("out", Expr::u32(0), Expr::var("acc")),
            ],
        )],
    );
    assert_emits_clean(
        prog,
        "if_then_else_branches_dont_leak_lets_to_sibling_branch",
    );
}

#[test]
fn carrier_visible_after_nested_if_within_outer_if() {
    // Two-level nesting: outer if contains inner if, both with assigns.
    // The merge Selects must thread up through both levels without any
    // let leaking to the other half of the outer if.
    let prog = Program::wrapped(
        vec![flag_buf(), out_count_buf()],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("acc", Expr::u32(0)),
                Node::let_bind("byte", Expr::load("flag", Expr::u32(0))),
                Node::if_then_else(
                    Expr::ne(Expr::var("byte"), Expr::u32(0)),
                    vec![
                        Node::if_then(
                            Expr::lt(Expr::var("byte"), Expr::u32(5)),
                            vec![Node::assign("acc", Expr::u32(11))],
                        ),
                        Node::assign("acc", Expr::add(Expr::var("acc"), Expr::u32(1))),
                    ],
                    vec![Node::assign("acc", Expr::u32(99))],
                ),
                Node::store("out", Expr::u32(0), Expr::var("acc")),
            ],
        )],
    );
    assert_emits_clean(prog, "carrier_visible_after_nested_if_within_outer_if");
}

#[test]
fn three_vars_assigned_in_inner_if_read_after_outer_if() {
    // The lex's deep failing shape: a loop body with an outer if, an
    // inner if inside it that assigns multiple vars, then post-inner-if
    // ops at the OUTER-if body level read all three vars. Each var's
    // values[id] must be rebound after the inner if so the outer-if's
    // subsequent ops see parent-scope Loads.
    let prog = Program::wrapped(
        vec![flag_buf(), out_count_buf()],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("a", Expr::u32(0)),
                Node::let_bind("b", Expr::u32(0)),
                Node::let_bind("c", Expr::u32(0)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::u32(8),
                    vec![
                        Node::let_bind("v", Expr::load("flag", Expr::var("i"))),
                        Node::if_then(
                            Expr::ne(Expr::var("v"), Expr::u32(0)),
                            vec![
                                Node::if_then(
                                    Expr::lt(Expr::var("v"), Expr::u32(5)),
                                    vec![
                                        Node::assign("a", Expr::add(Expr::var("a"), Expr::u32(1))),
                                        Node::assign("b", Expr::add(Expr::var("b"), Expr::u32(2))),
                                        Node::assign("c", Expr::add(Expr::var("c"), Expr::u32(3))),
                                    ],
                                ),
                                // post-inner-if reads a, b, c at outer-if scope
                                Node::assign("a", Expr::add(Expr::var("a"), Expr::var("b"))),
                                Node::assign("c", Expr::add(Expr::var("c"), Expr::var("a"))),
                            ],
                        ),
                    ],
                ),
                Node::store(
                    "out",
                    Expr::u32(0),
                    Expr::add(Expr::var("a"), Expr::add(Expr::var("b"), Expr::var("c"))),
                ),
            ],
        )],
    );
    assert_emits_clean(prog, "three_vars_assigned_in_inner_if_read_after_outer_if");
}

#[test]
fn loop_carrier_with_let_bind_shadowing_in_body() {
    // Pattern from the lex's classify_at_pos: `let_bind emit = 0` at
    // the top of a Region body inside the loop body. `emit` is freshly
    // bound each iteration, so it is NOT a loop carrier. But subsequent
    // assigns to `emit` inside conditional branches still need merging
    // within the iteration.
    let prog = Program::wrapped(
        vec![flag_buf(), out_count_buf()],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("hits", Expr::u32(0)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::u32(8),
                    vec![
                        Node::let_bind("byte", Expr::load("flag", Expr::var("i"))),
                        Node::let_bind("emit", Expr::u32(0)),
                        Node::if_then(
                            Expr::eq(Expr::var("byte"), Expr::u32(1)),
                            vec![Node::assign("emit", Expr::u32(1))],
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var("emit"), Expr::u32(1)),
                            vec![Node::assign(
                                "hits",
                                Expr::add(Expr::var("hits"), Expr::u32(1)),
                            )],
                        ),
                    ],
                ),
                Node::store("out", Expr::u32(0), Expr::var("hits")),
            ],
        )],
    );
    assert_emits_clean(prog, "loop_carrier_with_let_bind_shadowing_in_body");
}

#[test]
fn dump_c11_lexer_naga() {
    use vyre_libs::parsing::c::lex::lexer::c11_lexer;
    let prog = c11_lexer(
        "haystack",
        "out_tok_types",
        "out_tok_starts",
        "out_tok_lens",
        "out_counts",
        256,
    );
    let lk = vyre_lower::lower_for_emit(&prog).expect("c11_lexer lower_for_emit must succeed");
    let module = vyre_emit_naga::emit(&lk.descriptor).expect("c11_lexer emit must succeed");
    let wgsl = naga::back::wgsl::write_string(
        &module,
        &naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("validation should pass for dump"),
        naga::back::wgsl::WriterFlags::empty(),
    )
    .expect("wgsl write must succeed");
    println!("{}", wgsl);
}
