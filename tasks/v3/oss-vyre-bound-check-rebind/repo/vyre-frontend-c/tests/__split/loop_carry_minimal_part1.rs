use super::*;

#[test]
fn minimal_loop_carrier_accumulates_eight() {
    let prog = Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("acc", Expr::u32(0)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::u32(8),
                    vec![Node::assign("acc", Expr::add(Expr::var("acc"), Expr::u32(1)))],
                ),
                Node::store("out", Expr::u32(0), Expr::var("acc")),
            ],
        )],
    );
    let backend = vyre::backend::acquire_preferred_dispatch_backend()
        .expect("preferred backend available");
    let cfg = DispatchConfig::default();
    let outs = backend
        .dispatch(&prog, &[vec![0u8; 4]], &cfg)
        .expect("dispatch");
    let buf = outs.last().expect("output");
    let val = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    assert_eq!(val, 8, "loop must accumulate 1 each of 8 iterations");
}

#[test]
fn carrier_assign_inside_if_then_unconditional_true() {
    // Same as minimal but assign is wrapped in if_then(true).
    // If THIS fails the merge_if_then_scope is breaking carrier accumulation.
    let prog = Program::wrapped(
        vec![BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32).with_count(1)],
        [1, 1, 1],
        vec![Node::if_then(
            Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
            vec![
                Node::let_bind("acc", Expr::u32(0)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::u32(8),
                    vec![Node::if_then(
                        Expr::eq(Expr::u32(1), Expr::u32(1)),
                        vec![Node::assign("acc", Expr::add(Expr::var("acc"), Expr::u32(1)))],
                    )],
                ),
                Node::store("out", Expr::u32(0), Expr::var("acc")),
            ],
        )],
    );
    let backend = vyre::backend::acquire_preferred_dispatch_backend()
        .expect("preferred backend available");
    let cfg = DispatchConfig::default();
    let outs = backend
        .dispatch(&prog, &[vec![0u8; 4]], &cfg)
        .expect("dispatch");
    let buf = outs.last().expect("output");
    let val = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    assert_eq!(val, 8, "if_then(true)+assign must accumulate just like raw assign");
}

#[test]
fn carrier_assign_inside_if_then_with_load_cond() {
    // Loop body: let v = load(flag,i); if v != 0 { idx += 1 }
    let prog = Program::wrapped(
        vec![
            BufferDecl::storage("flag", 0, BufferAccess::ReadOnly, DataType::U32).with_count(8),
            BufferDecl::storage("out", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
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
                            vec![Node::assign(
                                "idx",
                                Expr::add(Expr::var("idx"), Expr::u32(1)),
                            )],
                        ),
                    ],
                ),
                Node::store("out", Expr::u32(0), Expr::var("idx")),
            ],
        )],
    );
    let backend = vyre::backend::acquire_preferred_dispatch_backend()
        .expect("preferred backend available");
    let cfg = DispatchConfig::default();
    let bytes_in: Vec<u8> = (10u32..=17).flat_map(|w| w.to_le_bytes()).collect();
    let outs = backend
        .dispatch(&prog, &[bytes_in, vec![0u8; 4]], &cfg)
        .expect("dispatch");
    let buf = outs.last().expect("output");
    let val = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    assert_eq!(val, 8, "carrier must increment 8 times when v != 0 each iter");
}

#[test]
fn dump_off_by_one_test_wgsl() {
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
                                                Expr::eq(
                                                    Expr::var("scan_done"),
                                                    Expr::u32(0),
                                                ),
                                                vec![Node::assign(
                                                    "tok_len",
                                                    Expr::add(
                                                        Expr::var("tok_len"),
                                                        Expr::u32(1),
                                                    ),
                                                )],
                                            )],
                                        ),
                                    ],
                                ),
                                Node::store("out_data", Expr::var("idx"), Expr::var("tok_len")),
                                Node::assign(
                                    "idx",
                                    Expr::add(Expr::var("idx"), Expr::u32(1)),
                                ),
                            ]),
                        },
                    ],
                ),
                Node::store("out_count", Expr::u32(0), Expr::var("idx")),
            ],
        )],
    );
    let dump_d = vyre_debug::dump_descriptor(
        &vyre_lower::lower_for_emit(&prog).unwrap().descriptor,
        &Default::default(),
    );
    eprintln!("=== OFF_BY_ONE DESC ===\n{}", dump_d.text);
    let dump_w = vyre_debug::dump_wgsl(&prog).unwrap();
    eprintln!("=== OFF_BY_ONE WGSL ===\n{}", dump_w.text);
}

#[test]
fn outer_loop_carrier_assign_inside_region_runs_eight_times() {
    // Outer loop has carrier `idx`; the assign(idx, idx+1) lives inside a
    // Region body. Should accumulate to 8 over 8 iters.
    let prog = Program::wrapped(
        vec![
            BufferDecl::storage("flag", 0, BufferAccess::ReadOnly, DataType::U32).with_count(8),
            BufferDecl::storage("out", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
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
                            generator: vyre::ir::Ident::from("test.unconditional_assign"),
                            source_region: None,
                            body: std::sync::Arc::new(vec![Node::assign(
                                "idx",
                                Expr::add(Expr::var("idx"), Expr::u32(1)),
                            )]),
                        },
                    ],
                ),
                Node::store("out", Expr::u32(0), Expr::var("idx")),
            ],
        )],
    );
    let backend = vyre::backend::acquire_preferred_dispatch_backend()
        .expect("preferred backend available");
    let cfg = DispatchConfig::default();
    let bytes_in: Vec<u8> = (1u32..=8).flat_map(|w| w.to_le_bytes()).collect();
    let outs = backend
        .dispatch(&prog, &[bytes_in, vec![0u8; 4]], &cfg)
        .expect("dispatch");
    let count = u32::from_le_bytes([
        outs.last().unwrap()[0],
        outs.last().unwrap()[1],
        outs.last().unwrap()[2],
        outs.last().unwrap()[3],
    ]);
    eprintln!("UNCOND_ASSIGN count={}", count);
    assert_eq!(count, 8);
}

#[test]
fn set_token_inside_region_inside_cursor_guard_with_advance() {
    // Closer to c11_lexer: outer loop wraps cursor advance and the
    // classify-Region in `if (cursor < haystack_len)`. cursor is a carrier
    // that gets advanced inside the if(emit==1) branch.
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
                Node::let_bind("cursor", Expr::u32(0)),
                Node::let_bind("idx", Expr::u32(0)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::u32(8),
                    vec![Node::if_then(
                        Expr::lt(Expr::var("cursor"), Expr::u32(8)),
                        vec![
                            Node::let_bind("v", Expr::load("flag", Expr::var("cursor"))),
                            Node::Region {
                                generator: vyre::ir::Ident::from("test.classify"),
                                source_region: None,
                                body: std::sync::Arc::new(vec![
                                    Node::let_bind("emit", Expr::u32(0)),
                                    Node::let_bind("tok_type", Expr::u32(0)),
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
                                            Node::assign(
                                                "cursor",
                                                Expr::add(Expr::var("cursor"), Expr::u32(1)),
                                            ),
                                        ],
                                    ),
                                ]),
                            },
                        ],
                    )],
                ),
                Node::store("out_count", Expr::u32(0), Expr::var("idx")),
            ],
        )],
    );
    let backend = vyre::backend::acquire_preferred_dispatch_backend()
        .expect("preferred backend available");
    let cfg = DispatchConfig::default();
    let bytes_in: Vec<u8> = [1u32, 2, 1, 2, 1, 2, 1, 2]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    let outs = backend
        .dispatch(&prog, &[bytes_in, vec![0u8; 32], vec![0u8; 4]], &cfg)
        .expect("dispatch");
    let count = u32::from_le_bytes([
        outs.last().unwrap()[0],
        outs.last().unwrap()[1],
        outs.last().unwrap()[2],
        outs.last().unwrap()[3],
    ]);
    let data: Vec<u32> = outs[outs.len() - 2]
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    eprintln!("LEX_LIKE count={} data={:?}", count, data);
    assert_eq!(count, 8, "must classify all 8 inputs");
    assert_eq!(data, vec![11, 22, 11, 22, 11, 22, 11, 22]);
}

