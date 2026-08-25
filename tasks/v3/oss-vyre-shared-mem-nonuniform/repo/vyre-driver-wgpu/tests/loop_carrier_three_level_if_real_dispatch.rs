//! Real-GPU regression test for the c-parser scope-walker bug shape:
//! a loop body whose only outer-var Assign is nested 3-deep inside
//! `if_then(cond_a) { if_then(cond_b) { if_then_else(cond_c) { assign(out_var, ..) } } }`.
//!
//! `vyre-emit-naga/tests/carrier_scope_regression.rs` only checks WGSL
//! validation; it cannot catch behavioral divergence. This test
//! actually dispatches the program on the live wgpu backend and asserts
//! the carrier value escapes the loop with the expected post-iteration
//! value.

mod common;
use common::acquire_live_backend as live_backend;

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre::{DispatchConfig, VyreBackend};

const SENTINEL: u32 = u32::MAX;

/// Mirrors the c-parser `c11_annotate_typedef_names` scope walker shape:
/// outer `let scope_open = SENTINEL`, then a loop iterating `i = 0..N`,
/// where the assign to `scope_open` lives 3 levels deep:
/// `if (scope_open == SENTINEL) { if (kind == LBRACE) { if_then_else (depth == 0) { assign scope_open = i } { assign depth-=1 } } }`.
///
/// Inputs: a `kinds` buffer with one byte per token; we set kinds[0]=LBRACE so
/// iteration 0 should latch `scope_open` to 0 and leave it pinned for the rest.
#[test]
fn three_level_if_assign_in_loop_propagates_via_carrier() {
    const TOK_LBRACE: u32 = 1;
    const N: u32 = 4;

    let prog = Program::wrapped(
        vec![
            BufferDecl::storage("kinds", 0, BufferAccess::ReadOnly, DataType::U32).with_count(N),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("scope_open", Expr::u32(SENTINEL)),
                Node::let_bind("depth", Expr::u32(0)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::u32(N),
                    vec![
                        Node::let_bind("kind", Expr::load("kinds", Expr::var("i"))),
                        Node::if_then(
                            Expr::eq(Expr::var("scope_open"), Expr::u32(SENTINEL)),
                            vec![Node::if_then(
                                Expr::eq(Expr::var("kind"), Expr::u32(TOK_LBRACE)),
                                vec![Node::if_then_else(
                                    Expr::eq(Expr::var("depth"), Expr::u32(0)),
                                    vec![Node::assign("scope_open", Expr::var("i"))],
                                    vec![Node::assign(
                                        "depth",
                                        Expr::sub(Expr::var("depth"), Expr::u32(1)),
                                    )],
                                )],
                            )],
                        ),
                    ],
                ),
                Node::store("out", Expr::u32(0), Expr::var("scope_open")),
            ],
        )],
    );

    let backend = live_backend();
    // 4 tokens: kinds = [LBRACE, 0, 0, 0]
    let kinds_bytes: Vec<u8> = [TOK_LBRACE, 0, 0, 0]
        .iter()
        .flat_map(|w: &u32| w.to_le_bytes())
        .collect();
    let outputs = backend
        .dispatch(&prog, &[kinds_bytes], &DispatchConfig::default())
        .expect("dispatch succeeds");
    assert_eq!(outputs.len(), 1);
    let out_word = u32::from_le_bytes(outputs[0][0..4].try_into().unwrap());
    assert_eq!(
        out_word, 0,
        "scope_open must latch to 0 in iteration 0 and propagate through to post-loop store; got {out_word:#x}",
    );
}

/// EXACT match for c11_annotate_typedef_names_impl shape: parallel
/// per-invocation work (no outer loop), each invocation has its own
/// `scope_open` / `scope_depth` let-bindings, then runs the scope_scan walker.
/// No `if gid==0` gate; multiple invocations execute concurrently.
#[test]
fn parallel_per_row_scope_walker_via_invocation_id() {
    const TOK_LBRACE: u32 = 1;
    const TOK_RBRACE: u32 = 2;
    const N: u32 = 4;

    let t = Expr::InvocationId { axis: 0 };
    let prog = Program::wrapped(
        vec![
            BufferDecl::storage("kinds", 0, BufferAccess::ReadOnly, DataType::U32).with_count(N),
            BufferDecl::output("out", 1, DataType::U32).with_count(N),
        ],
        [N, 1, 1],
        vec![Node::if_then(
            Expr::lt(t.clone(), Expr::u32(N)),
            vec![
                Node::let_bind("scope_open", Expr::u32(SENTINEL)),
                Node::let_bind("scope_depth", Expr::u32(0)),
                Node::loop_for(
                    "scope_scan",
                    Expr::u32(0),
                    t.clone(),
                    vec![
                        Node::let_bind(
                            "scope_rev",
                            Expr::sub(Expr::sub(t.clone(), Expr::u32(1)), Expr::var("scope_scan")),
                        ),
                        Node::let_bind("scope_kind", Expr::load("kinds", Expr::var("scope_rev"))),
                        Node::if_then(
                            Expr::and(
                                Expr::eq(Expr::var("scope_open"), Expr::u32(SENTINEL)),
                                Expr::eq(Expr::var("scope_kind"), Expr::u32(TOK_RBRACE)),
                            ),
                            vec![Node::assign(
                                "scope_depth",
                                Expr::add(Expr::var("scope_depth"), Expr::u32(1)),
                            )],
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var("scope_open"), Expr::u32(SENTINEL)),
                            vec![Node::if_then(
                                Expr::eq(Expr::var("scope_kind"), Expr::u32(TOK_LBRACE)),
                                vec![Node::if_then_else(
                                    Expr::eq(Expr::var("scope_depth"), Expr::u32(0)),
                                    vec![Node::assign("scope_open", Expr::var("scope_rev"))],
                                    vec![Node::assign(
                                        "scope_depth",
                                        Expr::sub(Expr::var("scope_depth"), Expr::u32(1)),
                                    )],
                                )],
                            )],
                        ),
                    ],
                ),
                Node::store("out", t.clone(), Expr::var("scope_open")),
            ],
        )],
    );

    let backend = live_backend();
    let kinds_bytes: Vec<u8> = [TOK_LBRACE, TOK_LBRACE, 0, 0]
        .iter()
        .flat_map(|w: &u32| w.to_le_bytes())
        .collect();
    let outputs = backend
        .dispatch(&prog, &[kinds_bytes], &DispatchConfig::default())
        .expect("dispatch succeeds");
    assert_eq!(outputs.len(), 1);
    let words: Vec<u32> = outputs[0]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    assert_eq!(
        words,
        vec![SENTINEL, 0, 1, 1],
        "parallel per-invocation scope walker must produce one scope per row; got {words:?}",
    );
}

/// Real repro: the scope walker `scope_scan` loop lives INSIDE an outer per-row
/// `for t in 0..N` loop. Each outer iteration starts with a fresh `let
/// scope_open = SENTINEL`, runs the inner walker, then writes to a different
/// row of the output. The inner walker's carrier mechanism must NOT bleed
/// state across outer iterations  -  every outer iter starts clean.
#[test]
fn nested_outer_loop_with_inner_scope_walker_per_row() {
    const TOK_LBRACE: u32 = 1;
    const TOK_RBRACE: u32 = 2;
    const N: u32 = 4;

    let prog = Program::wrapped(
        vec![
            BufferDecl::storage("kinds", 0, BufferAccess::ReadOnly, DataType::U32).with_count(N),
            BufferDecl::output("out", 1, DataType::U32).with_count(N),
        ],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![Node::loop_for(
                "t",
                Expr::u32(0),
                Expr::u32(N),
                vec![
                    Node::let_bind("scope_open", Expr::u32(SENTINEL)),
                    Node::let_bind("scope_depth", Expr::u32(0)),
                    Node::loop_for(
                        "scope_scan",
                        Expr::u32(0),
                        Expr::var("t"),
                        vec![
                            Node::let_bind(
                                "scope_rev",
                                Expr::sub(
                                    Expr::sub(Expr::var("t"), Expr::u32(1)),
                                    Expr::var("scope_scan"),
                                ),
                            ),
                            Node::let_bind(
                                "scope_kind",
                                Expr::load("kinds", Expr::var("scope_rev")),
                            ),
                            Node::if_then(
                                Expr::and(
                                    Expr::eq(Expr::var("scope_open"), Expr::u32(SENTINEL)),
                                    Expr::eq(Expr::var("scope_kind"), Expr::u32(TOK_RBRACE)),
                                ),
                                vec![Node::assign(
                                    "scope_depth",
                                    Expr::add(Expr::var("scope_depth"), Expr::u32(1)),
                                )],
                            ),
                            Node::if_then(
                                Expr::eq(Expr::var("scope_open"), Expr::u32(SENTINEL)),
                                vec![Node::if_then(
                                    Expr::eq(Expr::var("scope_kind"), Expr::u32(TOK_LBRACE)),
                                    vec![Node::if_then_else(
                                        Expr::eq(Expr::var("scope_depth"), Expr::u32(0)),
                                        vec![Node::assign("scope_open", Expr::var("scope_rev"))],
                                        vec![Node::assign(
                                            "scope_depth",
                                            Expr::sub(Expr::var("scope_depth"), Expr::u32(1)),
                                        )],
                                    )],
                                )],
                            ),
                        ],
                    ),
                    Node::store("out", Expr::var("t"), Expr::var("scope_open")),
                ],
            )],
        )],
    );

    let backend = live_backend();
    // kinds = [LBRACE, LBRACE, anything, anything]
    // For t=0: walker has 0 iters → scope_open stays SENTINEL → store SENTINEL
    // For t=1: walker iterates scope_scan=0 (rev=0): kind=LBRACE, depth=0 → scope_open=0 → store 0
    // For t=2: walker iterates scope_scan=0 (rev=1, kind=LBRACE, depth=0 → scope_open=1), scan=1 skipped → store 1
    // For t=3: walker scope_scan=0..3, rev=2,1,0. rev=2 kind=anything, rev=1 LBRACE depth=0 → scope_open=1
    let kinds_bytes: Vec<u8> = [TOK_LBRACE, TOK_LBRACE, 0, 0]
        .iter()
        .flat_map(|w: &u32| w.to_le_bytes())
        .collect();
    let outputs = backend
        .dispatch(&prog, &[kinds_bytes], &DispatchConfig::default())
        .expect("dispatch succeeds");
    assert_eq!(outputs.len(), 1);
    let words: Vec<u32> = outputs[0]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    assert_eq!(
        words,
        vec![SENTINEL, 0, 1, 1],
        "outer-per-row scope walker must produce one scope per row; got {words:?}",
    );
}

/// Closer to the actual c-parser scope walker: TWO sequential conditionals in
/// the loop body, both writing outer-scope vars. The first writes `depth` only;
/// the second is the 3-level nest that writes `scope_open` OR `depth`. Both
/// conditional gates read `scope_open`, so the merge between the two
/// conditionals is the chokepoint.
#[test]
fn two_sequential_conditionals_with_shared_carrier_propagate() {
    const TOK_LBRACE: u32 = 1;
    const TOK_RBRACE: u32 = 2;

    let prog = Program::wrapped(
        vec![
            BufferDecl::storage("kinds", 0, BufferAccess::ReadOnly, DataType::U32).with_count(4),
            BufferDecl::output("out", 1, DataType::U32).with_count(2),
        ],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("scope_open", Expr::u32(SENTINEL)),
                Node::let_bind("depth", Expr::u32(0)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::u32(4),
                    vec![
                        Node::let_bind("kind", Expr::load("kinds", Expr::var("i"))),
                        Node::if_then(
                            Expr::and(
                                Expr::eq(Expr::var("scope_open"), Expr::u32(SENTINEL)),
                                Expr::eq(Expr::var("kind"), Expr::u32(TOK_RBRACE)),
                            ),
                            vec![Node::assign(
                                "depth",
                                Expr::add(Expr::var("depth"), Expr::u32(1)),
                            )],
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var("scope_open"), Expr::u32(SENTINEL)),
                            vec![Node::if_then(
                                Expr::eq(Expr::var("kind"), Expr::u32(TOK_LBRACE)),
                                vec![Node::if_then_else(
                                    Expr::eq(Expr::var("depth"), Expr::u32(0)),
                                    vec![Node::assign("scope_open", Expr::var("i"))],
                                    vec![Node::assign(
                                        "depth",
                                        Expr::sub(Expr::var("depth"), Expr::u32(1)),
                                    )],
                                )],
                            )],
                        ),
                    ],
                ),
                Node::store("out", Expr::u32(0), Expr::var("scope_open")),
                Node::store("out", Expr::u32(1), Expr::var("depth")),
            ],
        )],
    );

    let backend = live_backend();
    // Test fixture mirrors c-parser scope_open_before(idx=2) where tokens
    // are [RBRACE, LBRACE, LBRACE, ...]. Walking i=0..3:
    //   i=0 (RBRACE): cond1 fires → depth=1
    //   i=1 (LBRACE): cond2 fires → kind==LBRACE → depth(1)==0 FALSE → depth=0
    //   i=2 (LBRACE): cond2 fires → kind==LBRACE → depth(0)==0 TRUE → scope_open=2
    //   i=3 (?): scope_open != SENTINEL → both skip
    // Expected: scope_open=2, depth=0
    let kinds_bytes: Vec<u8> = [TOK_RBRACE, TOK_LBRACE, TOK_LBRACE, 0]
        .iter()
        .flat_map(|w: &u32| w.to_le_bytes())
        .collect();
    let outputs = backend
        .dispatch(&prog, &[kinds_bytes], &DispatchConfig::default())
        .expect("dispatch succeeds");
    assert_eq!(outputs.len(), 1);
    let scope_open = u32::from_le_bytes(outputs[0][0..4].try_into().unwrap());
    let depth = u32::from_le_bytes(outputs[0][4..8].try_into().unwrap());
    assert_eq!(
        (scope_open, depth),
        (2, 0),
        "two sequential conditionals must propagate carrier values across iterations; got scope_open={scope_open:#x} depth={depth}",
    );
}
