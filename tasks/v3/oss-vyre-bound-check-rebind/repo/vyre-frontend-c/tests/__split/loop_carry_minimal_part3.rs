use super::*;

#[test]
fn reference_eval_failing_program() {
    // Run the failing program through CPU reference eval. If carrier
    // semantics are correct in the IR, this should give count=8,
    // data=[10..17]. If it's wrong here too, the descriptor lowering is
    // wrong (not a wgpu bug).
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
                                Node::assign(
                                    "idx",
                                    Expr::add(Expr::var("idx"), Expr::u32(1)),
                                ),
                            ],
                        ),
                    ],
                ),
                Node::store("out_count", Expr::u32(0), Expr::var("idx")),
            ],
        )],
    );
    let bytes_in: Vec<u8> = (10u32..=17).flat_map(|w| w.to_le_bytes()).collect();
    let inputs: Vec<vyre_reference::value::Value> = vec![
        bytes_in.into(),
        vec![0u8; 32].into(),
        vec![0u8; 4].into(),
    ];
    let outs = vyre_reference::reference_eval(&prog, &inputs).expect("CPU eval");
    let out_count_bytes = outs.last().unwrap().clone().to_bytes();
    let count = u32::from_le_bytes([
        out_count_bytes[0],
        out_count_bytes[1],
        out_count_bytes[2],
        out_count_bytes[3],
    ]);
    let out_data_bytes = outs[outs.len() - 2].clone().to_bytes();
    let data: Vec<u32> = out_data_bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    eprintln!("CPU REFERENCE: count={} data={:?}", count, data);
    assert_eq!(count, 8, "CPU eval must produce count=8");
    assert_eq!(data, vec![10, 11, 12, 13, 14, 15, 16, 17], "CPU eval must store distinct slots");
}

#[test]
fn dump_load_cond_passing() {
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
    let dump_w = vyre_debug::dump_wgsl(&prog).unwrap();
    eprintln!("=== LOAD COND PASSING WGSL ===\n{}", dump_w.text);
}

#[test]
fn dump_minimal_passing() {
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
    let dump_w = vyre_debug::dump_wgsl(&prog).unwrap();
    eprintln!("=== PASSING WGSL ===\n{}", dump_w.text);
}

#[test]
fn carrier_with_store_then_assign_inside_if_then_dump() {
    // Same shape as the failing test, but dumps WGSL+descriptor for debug.
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
                                Node::assign(
                                    "idx",
                                    Expr::add(Expr::var("idx"), Expr::u32(1)),
                                ),
                            ],
                        ),
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
    eprintln!("=== DESCRIPTOR ===\n{}", dump_d.text);
    let dump_w = vyre_debug::dump_wgsl(&prog).unwrap();
    eprintln!("=== WGSL ===\n{}", dump_w.text);
}

#[test]
fn carrier_with_store_then_assign_inside_if_then() {
    // The carrier_indexed_store_writes_distinct_indices shape: a Store
    // that READS the carrier appears in the then-branch BEFORE the
    // assign that increments it. Two ops in the same then-branch.
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
                                Node::assign(
                                    "idx",
                                    Expr::add(Expr::var("idx"), Expr::u32(1)),
                                ),
                            ],
                        ),
                    ],
                ),
                Node::store("out_count", Expr::u32(0), Expr::var("idx")),
            ],
        )],
    );
    let backend = vyre::backend::acquire_preferred_dispatch_backend()
        .expect("preferred backend available");
    let cfg = DispatchConfig::default();
    let bytes_in: Vec<u8> = (10u32..=17).flat_map(|w| w.to_le_bytes()).collect();
    let outs = backend
        .dispatch(&prog, &[bytes_in, vec![0u8; 32], vec![0u8; 4]], &cfg)
        .expect("dispatch");
    let count_buf = outs.last().expect("output");
    let count = u32::from_le_bytes([count_buf[0], count_buf[1], count_buf[2], count_buf[3]]);
    let data_buf = &outs[outs.len() - 2];
    let data: Vec<u32> = data_buf
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    eprintln!("count={} data={:?} all_outs_len={:?}", count, data, outs.iter().map(|o| o.len()).collect::<Vec<_>>());
    assert_eq!(
        data,
        vec![10, 11, 12, 13, 14, 15, 16, 17],
        "each iter must store v at distinct carrier-indexed slot"
    );
    assert_eq!(count, 8, "store-then-assign: carrier still must increment 8");
}
