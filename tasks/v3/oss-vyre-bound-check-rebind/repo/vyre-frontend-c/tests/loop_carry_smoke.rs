//! Smoke test: dispatch a lex-shaped accumulating loop on the wgpu
//! backend and assert the carrier-driven `tok_idx` matches expectation.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre::DispatchConfig;

#[test]
fn carrier_indexed_store_writes_distinct_indices() {
    // Mimics the lex's store_token_and_advance pattern: a loop where
    // each iteration conditionally stores to an output buffer indexed
    // by a carrier counter, then increments the counter. Each store
    // must land at a distinct index.
    let prog = Program::wrapped(
        vec![
            BufferDecl::storage("flag", 0, BufferAccess::ReadOnly, DataType::U32).with_count(8),
            BufferDecl::storage("out_data", 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(8),
            BufferDecl::storage("out_count", 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("idx", Expr::u32(0)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::u32(8),
                    vec![
                        Node::let_bind("v", Expr::load("flag", Expr::var("i"))),
                        Node::if_then(
                            Expr::ne(Expr::var("v"), Expr::u32(0)),
                            vec![
                                Node::store("out_data", Expr::var("idx"), Expr::var("v")),
                                Node::assign("idx", Expr::add(Expr::var("idx"), Expr::u32(1))),
                            ],
                        ),
                    ],
                ),
                Node::store("out_count", Expr::u32(0), Expr::var("idx")),
            ],
        )],
    );
    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("preferred backend available");
    let cfg = DispatchConfig::default();
    let bytes_in: Vec<u8> = (10u32..=17).flat_map(|w| w.to_le_bytes()).collect();
    let data_init = vec![0u8; 32];
    let count_init = vec![0u8; 4];
    let outs = backend
        .dispatch(&prog, &[bytes_in, data_init, count_init], &cfg)
        .expect("dispatch");
    let count_buf = outs.last().expect("output buffer");
    let count = u32::from_le_bytes([count_buf[0], count_buf[1], count_buf[2], count_buf[3]]);
    assert_eq!(count, 8, "carrier should reach 8");
    let data_buf = &outs[outs.len() - 2];
    let data: Vec<u32> = data_buf
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    assert_eq!(
        data,
        vec![10, 11, 12, 13, 14, 15, 16, 17],
        "each iteration must write to a distinct carrier-indexed slot",
    );
}

#[test]
fn region_body_let_bind_with_set_token_chain_writes_correct_type() {
    // Mirrors the lex's classify_at_pos shape: var `tok_type` is
    // let_bind'd fresh each loop iteration INSIDE a Region body.
    // Several `if_then(emit==0 AND cond, [assign emit=1, assign tok_type=N])`
    // sequences (the set_token pattern) update tok_type. After the
    // Region body, a Store writes `tok_type` to an output buffer.
    // For input flag=[1,2,3,...], the test asserts each iteration
    // wrote the matching tok_type (1→11, 2→22, 3→33).
    let prog = Program::wrapped(
        vec![
            BufferDecl::storage("flag", 0, BufferAccess::ReadOnly, DataType::U32).with_count(8),
            BufferDecl::storage("out_data", 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(8),
            BufferDecl::storage("out_count", 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("idx", Expr::u32(0)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::u32(8),
                    vec![
                        Node::let_bind("v", Expr::load("flag", Expr::var("i"))),
                        // Region wrapping the per-iter body, like lex's
                        // child_phase("classify_at_pos", ...).
                        Node::Region {
                            generator: vyre::ir::Ident::from("test.classify"),
                            source_region: None,
                            body: std::sync::Arc::new(vec![
                                Node::let_bind("emit", Expr::u32(0)),
                                Node::let_bind("tok_type", Expr::u32(0)),
                                // set_token #1
                                Node::if_then(
                                    Expr::and(
                                        Expr::eq(Expr::var("emit"), Expr::u32(0)),
                                        Expr::eq(Expr::var("v"), Expr::u32(1)),
                                    ),
                                    vec![
                                        Node::assign("emit", Expr::u32(1)),
                                        Node::assign("tok_type", Expr::u32(11)),
                                    ],
                                ),
                                // set_token #2
                                Node::if_then(
                                    Expr::and(
                                        Expr::eq(Expr::var("emit"), Expr::u32(0)),
                                        Expr::eq(Expr::var("v"), Expr::u32(2)),
                                    ),
                                    vec![
                                        Node::assign("emit", Expr::u32(1)),
                                        Node::assign("tok_type", Expr::u32(22)),
                                    ],
                                ),
                                // set_token #3
                                Node::if_then(
                                    Expr::and(
                                        Expr::eq(Expr::var("emit"), Expr::u32(0)),
                                        Expr::eq(Expr::var("v"), Expr::u32(3)),
                                    ),
                                    vec![
                                        Node::assign("emit", Expr::u32(1)),
                                        Node::assign("tok_type", Expr::u32(33)),
                                    ],
                                ),
                                // store_token: if emit==1, store tok_type
                                Node::if_then(
                                    Expr::eq(Expr::var("emit"), Expr::u32(1)),
                                    vec![
                                        Node::store(
                                            "out_data",
                                            Expr::var("idx"),
                                            Expr::var("tok_type"),
                                        ),
                                        Node::assign(
                                            "idx",
                                            Expr::add(Expr::var("idx"), Expr::u32(1)),
                                        ),
                                    ],
                                ),
                            ]),
                        },
                    ],
                ),
                Node::store("out_count", Expr::u32(0), Expr::var("idx")),
            ],
        )],
    );
    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("preferred backend available");
    let cfg = DispatchConfig::default();
    let bytes_in: Vec<u8> = [1u32, 2, 3, 1, 2, 3, 1, 2]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    let data_init = vec![0u8; 32];
    let count_init = vec![0u8; 4];
    let outs = backend
        .dispatch(&prog, &[bytes_in, data_init, count_init], &cfg)
        .expect("dispatch");
    let count_buf = outs.last().expect("output buffer");
    let count = u32::from_le_bytes([count_buf[0], count_buf[1], count_buf[2], count_buf[3]]);
    assert_eq!(count, 8, "all 8 inputs match a set_token");
    let data_buf = &outs[outs.len() - 2];
    let data: Vec<u32> = data_buf
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    assert_eq!(
        data,
        vec![11, 22, 33, 11, 22, 33, 11, 22],
        "Stores must read the per-iteration `tok_type` set by the matching set_token, \
         not the let_bind seed (0) or some stale value",
    );
}

#[test]
fn region_body_let_bind_with_inner_loop_increment_then_store() {
    // Reproduces the lex's PREPROC scan-loop shape:
    //   Region body { let_bind tok_len = 1; if_then(cond, [ loop_for { if (..) { tok_len = tok_len + 1 } } ]); store(tok_len); }
    // After the inner loop runs N times incrementing tok_len, the Store
    // at the END of the Region body must read the post-loop value, not
    // the pre-loop seed (1) and not 0.
    let prog = Program::wrapped(
        vec![
            BufferDecl::storage("flag", 0, BufferAccess::ReadOnly, DataType::U32).with_count(8),
            BufferDecl::storage("out_data", 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(8),
            BufferDecl::storage("out_count", 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("idx", Expr::u32(0)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::u32(8),
                    vec![
                        Node::let_bind("v", Expr::load("flag", Expr::var("i"))),
                        Node::Region {
                            generator: vyre::ir::Ident::from("test.classify"),
                            source_region: None,
                            body: std::sync::Arc::new(vec![
                                Node::let_bind("tok_len", Expr::u32(1)),
                                Node::if_then(
                                    Expr::ne(Expr::var("v"), Expr::u32(0)),
                                    vec![
                                        Node::let_bind("scan_done", Expr::u32(0)),
                                        Node::loop_for(
                                            "scan",
                                            Expr::u32(0),
                                            Expr::var("v"),
                                            vec![Node::if_then(
                                                Expr::eq(Expr::var("scan_done"), Expr::u32(0)),
                                                vec![Node::assign(
                                                    "tok_len",
                                                    Expr::add(Expr::var("tok_len"), Expr::u32(1)),
                                                )],
                                            )],
                                        ),
                                    ],
                                ),
                                Node::store("out_data", Expr::var("idx"), Expr::var("tok_len")),
                                Node::assign("idx", Expr::add(Expr::var("idx"), Expr::u32(1))),
                            ]),
                        },
                    ],
                ),
                Node::store("out_count", Expr::u32(0), Expr::var("idx")),
            ],
        )],
    );
    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("preferred backend available");
    let cfg = DispatchConfig::default();
    // For input v=N: scan loop runs N iters incrementing tok_len.
    // Expected tok_len at store: 1 + N.
    let bytes_in: Vec<u8> = [0u32, 1, 2, 3, 4, 5, 6, 7]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    let data_init = vec![0u8; 32];
    let count_init = vec![0u8; 4];
    let outs = backend
        .dispatch(&prog, &[bytes_in, data_init, count_init], &cfg)
        .expect("dispatch");
    let count_buf = outs.last().expect("output buffer");
    let count = u32::from_le_bytes([count_buf[0], count_buf[1], count_buf[2], count_buf[3]]);
    assert_eq!(
        count, 8,
        "all 8 inputs stored (idx incremented unconditionally)"
    );
    let data_buf = &outs[outs.len() - 2];
    let data: Vec<u32> = data_buf
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    // Iter 0 (v=0): scan loop SKIPPED (the if_then guard `v != 0` is false), tok_len stays at 1.
    // Iters 1..7: scan loop ran v times, each iter incremented tok_len. tok_len = 1 + v.
    assert_eq!(
        data,
        vec![1, 2, 3, 4, 5, 6, 7, 8],
        "Store after Region body must read post-loop tok_len, not the seed (1) and not 0",
    );
}

#[test]
fn nested_loop_reseeds_named_carrier_to_inner_scope_value() {
    // Two sibling inner loops in the same Region body, each writing
    // to the same source-level variable `acc` via `assign acc = acc + 1`.
    // Between them, an `assign acc = 100` resets acc.
    //
    // Bug shape (pre-fix): the SECOND inner loop's named-carrier seed
    // was skipped because the carrier-local already existed from the
    // first loop. So loop-2 read whatever loop-1 left in the local
    // (= 1 + iter_count_1), not the post-reset value (= 100).
    //
    // Expected (post-fix): the second inner loop sees acc=100, so its
    // post-loop value is 100 + iter_count_2.
    let prog = Program::wrapped(
        vec![
            BufferDecl::storage("flag", 0, BufferAccess::ReadOnly, DataType::U32).with_count(8),
            BufferDecl::storage("out_data", 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(8),
        ],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(8),
                vec![
                    Node::let_bind("v", Expr::load("flag", Expr::var("i"))),
                    Node::Region {
                        generator: vyre::ir::Ident::from("test.region"),
                        source_region: None,
                        body: std::sync::Arc::new(vec![
                            Node::let_bind("acc", Expr::u32(0)),
                            // Loop 1: increment acc by v
                            Node::loop_for(
                                "j",
                                Expr::u32(0),
                                Expr::var("v"),
                                vec![Node::assign(
                                    "acc",
                                    Expr::add(Expr::var("acc"), Expr::u32(1)),
                                )],
                            ),
                            // Reset acc.
                            Node::assign("acc", Expr::u32(100)),
                            // Loop 2: increment acc by v.
                            Node::loop_for(
                                "k",
                                Expr::u32(0),
                                Expr::var("v"),
                                vec![Node::assign(
                                    "acc",
                                    Expr::add(Expr::var("acc"), Expr::u32(1)),
                                )],
                            ),
                            Node::store("out_data", Expr::var("i"), Expr::var("acc")),
                        ]),
                    },
                ],
            )],
        )],
    );
    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("preferred backend available");
    let cfg = DispatchConfig::default();
    let bytes_in: Vec<u8> = [0u32, 1, 2, 3, 4, 5, 6, 7]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    let data_init = vec![0u8; 32];
    let outs = backend
        .dispatch(&prog, &[bytes_in, data_init], &cfg)
        .expect("dispatch");
    let data_buf = outs.last().expect("output buffer");
    let data: Vec<u32> = data_buf
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    // For input v: loop1 makes acc=v, reset to 100, loop2 makes acc=100+v.
    assert_eq!(
        data,
        vec![100, 101, 102, 103, 104, 105, 106, 107],
        "second inner loop must re-seed its named carrier from the post-reset scope value (100), \
         not inherit the first loop's leftover state",
    );
}

// Linking the wgpu driver so its `inventory::submit!` registrations are
// pulled into this test binary and `acquire_preferred_dispatch_backend`
// finds a live backend.
#[allow(unused_imports)]
use vyre_driver_wgpu as _2;
// Linking the wgpu driver so its `inventory::submit!` registrations are
// pulled into this test binary and `acquire_preferred_dispatch_backend`
// finds a live backend.
#[allow(unused_imports)]
use vyre_driver_wgpu as _;

#[test]
fn loop_carrier_accumulates_under_nested_if_then() {
    let prog = Program::wrapped(
        vec![
            BufferDecl::storage("haystack", 0, BufferAccess::ReadOnly, DataType::U32).with_count(8),
            BufferDecl::storage("out_count", 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("cursor", Expr::u32(0)),
                Node::let_bind("tok_idx", Expr::u32(0)),
                Node::loop_for(
                    "iter",
                    Expr::u32(0),
                    Expr::u32(8),
                    vec![Node::if_then(
                        Expr::lt(Expr::var("cursor"), Expr::u32(8)),
                        vec![
                            Node::let_bind("byte", Expr::load("haystack", Expr::var("cursor"))),
                            Node::let_bind("emit", Expr::u32(0)),
                            Node::if_then(
                                Expr::and(
                                    Expr::eq(Expr::var("emit"), Expr::u32(0)),
                                    Expr::ne(Expr::var("byte"), Expr::u32(0)),
                                ),
                                vec![Node::assign("emit", Expr::u32(1))],
                            ),
                            Node::if_then(
                                Expr::eq(Expr::var("emit"), Expr::u32(1)),
                                vec![Node::assign(
                                    "tok_idx",
                                    Expr::add(Expr::var("tok_idx"), Expr::u32(1)),
                                )],
                            ),
                            Node::assign("cursor", Expr::add(Expr::var("cursor"), Expr::u32(1))),
                        ],
                    )],
                ),
                Node::store("out_count", Expr::u32(0), Expr::var("tok_idx")),
            ],
        )],
    );
    let backend =
        vyre::backend::acquire_preferred_dispatch_backend().expect("preferred backend available");
    let cfg = DispatchConfig::default();
    let bytes_in: Vec<u8> = (1u32..=8).flat_map(|w| w.to_le_bytes()).collect();
    let count_init = vec![0u8; 4];
    let outs = backend
        .dispatch(&prog, &[bytes_in, count_init], &cfg)
        .expect("dispatch");
    let count_buf = outs.last().expect("output buffer");
    let count = u32::from_le_bytes([count_buf[0], count_buf[1], count_buf[2], count_buf[3]]);
    assert_eq!(
        count, 8,
        "tok_idx-style carrier must accumulate one increment per iteration where byte != 0; got {count}",
    );
}
