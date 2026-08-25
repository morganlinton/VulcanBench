use super::*;

#[test]
fn set_token_chain_inside_region_classifies_correctly() {
    // c11_lexer's classify_at_pos is wrapped in a Node::Region. Repro the
    // exact shape: outer loop body has a Region, inside which lives the
    // set_token chain plus the carrier-incrementing if_then(emit==1).
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
                                    Expr::and(
                                        Expr::eq(Expr::var("emit"), Expr::u32(0)),
                                        Expr::eq(Expr::var("v"), Expr::u32(3)),
                                    ),
                                    vec![
                                        Node::assign("emit", Expr::u32(1)),
                                        Node::assign("tok_type", Expr::u32(33)),
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
    let backend = vyre::backend::acquire_preferred_dispatch_backend()
        .expect("preferred backend available");
    let cfg = DispatchConfig::default();
    let bytes_in: Vec<u8> = [1u32, 2, 3, 1, 2, 3, 1, 2]
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
    eprintln!("REGION_SET_TOKEN count={} data={:?}", count, data);
    assert_eq!(count, 8, "Region-wrapped set_token chain must classify all 8");
    assert_eq!(data, vec![11, 22, 33, 11, 22, 33, 11, 22]);
}

#[test]
fn set_token_chain_classifies_correctly() {
    // Mimic c11_lexer's set_token chain pattern:
    // for each iter:
    //   let emit=0; let tok_type=0;
    //   if (emit==0 && v==1) { emit=1; tok_type=11; }
    //   if (emit==0 && v==2) { emit=1; tok_type=22; }
    //   if (emit==0 && v==3) { emit=1; tok_type=33; }
    //   if (emit==1) { out_data[i] = tok_type; idx++; }
    // For input v=[1,2,3,1,2,3,1,2], output should be [11,22,33,11,22,33,11,22] count=8
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
                            Expr::and(
                                Expr::eq(Expr::var("emit"), Expr::u32(0)),
                                Expr::eq(Expr::var("v"), Expr::u32(3)),
                            ),
                            vec![
                                Node::assign("emit", Expr::u32(1)),
                                Node::assign("tok_type", Expr::u32(33)),
                            ],
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var("emit"), Expr::u32(1)),
                            vec![
                                Node::store("out_data", Expr::var("idx"), Expr::var("tok_type")),
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
    let bytes_in: Vec<u8> = [1u32, 2, 3, 1, 2, 3, 1, 2]
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
    eprintln!("SET_TOKEN count={} data={:?}", count, data);
    assert_eq!(count, 8, "set_token chain must classify all 8 inputs");
    assert_eq!(data, vec![11, 22, 33, 11, 22, 33, 11, 22], "tok_type per iter");
}

#[test]
fn c11_lexer_emits_tokens_for_increasing_sources() {
    use vyre_libs::parsing::c::lex::lexer::c11_lexer;
    let backend = vyre::backend::acquire_preferred_dispatch_backend()
        .expect("preferred backend available");
    let cfg = DispatchConfig::default();
    for (label, src) in &[
        ("tiny", "int main(){return 0;}\n"),
        ("medium", "int a; int b; int c; int d;\n"),
        ("kernel-ish", include_str!("../loop_carry_minimal_fixture.c")),
    ] {
        let bytes = src.as_bytes();
        let len = bytes.len() as u32;
        let prog = c11_lexer(
            "haystack",
            "out_tok_types",
            "out_tok_starts",
            "out_tok_lens",
            "out_counts",
            len,
        );
        let n = len.max(1) as usize;
        let outs = backend
            .dispatch(
                &prog,
                &[
                    bytes.to_vec(),
                    vec![0u8; n * 4],
                    vec![0u8; n * 4],
                    vec![0u8; n * 4],
                    vec![0u8; 4],
                ],
                &cfg,
            )
            .expect("dispatch");
        let count_buf = outs.last().unwrap();
        let count = u32::from_le_bytes([count_buf[0], count_buf[1], count_buf[2], count_buf[3]]);
        eprintln!("[label={label}] len={len} count={count}");
    }
}

#[test]
fn c11_lexer_emits_tokens_for_int_main_void() {
    use vyre_libs::parsing::c::lex::lexer::c11_lexer;
    let src = "int main(void) { return 0; }";
    // c11_lexer expects haystack[i] = ith byte stored as a u32 word
    // (one byte per u32). pack_haystack in pipeline/buffers.rs does the
    // packing for the production pipeline; replicate it here.
    let bytes_words: Vec<u8> = src
        .bytes()
        .flat_map(|b| (b as u32).to_le_bytes())
        .collect();
    let len = src.len() as u32;
    let bytes = bytes_words.as_slice();
    let prog = c11_lexer(
        "haystack",
        "out_tok_types",
        "out_tok_starts",
        "out_tok_lens",
        "out_counts",
        len,
    );
    let backend = vyre::backend::acquire_preferred_dispatch_backend()
        .expect("preferred backend available");
    let cfg = DispatchConfig::default();
    let n = len.max(1) as usize;
    let outs = backend
        .dispatch(
            &prog,
            &[
                bytes.to_vec(),
                vec![0u8; n * 4],
                vec![0u8; n * 4],
                vec![0u8; n * 4],
                vec![0u8; 4],
            ],
            &cfg,
        )
        .expect("c11_lexer dispatch");
    let count_buf = outs.last().unwrap();
    let count = u32::from_le_bytes([count_buf[0], count_buf[1], count_buf[2], count_buf[3]]);
    let types_buf = &outs[0];
    let types: Vec<u32> = types_buf
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    eprintln!(
        "C11_LEXER count={} first_types={:?}",
        count,
        &types[..(count as usize).min(types.len())]
    );
    assert!(count > 0, "c11_lexer must emit at least one token for `int main(void) {{ return 0; }}`");
}

#[test]
fn carrier_increment_with_global_store_no_inner_if() {
    // Strip the inner if. Loop body just has a Store + Assign.
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
                        Node::store("out_data", Expr::var("idx"), Expr::var("v")),
                        Node::assign("idx", Expr::add(Expr::var("idx"), Expr::u32(1))),
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
    eprintln!("NO-INNER-IF count={} data={:?}", count, data);
    assert_eq!(count, 8);
    assert_eq!(data, vec![10, 11, 12, 13, 14, 15, 16, 17]);
}

#[test]
fn carrier_with_assign_then_store_inside_if_then() {
    // Same as the failing test but Assign happens BEFORE Store.
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
                                Node::assign(
                                    "idx",
                                    Expr::add(Expr::var("idx"), Expr::u32(1)),
                                ),
                                Node::store("out_data", Expr::var("idx"), Expr::var("v")),
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
    eprintln!("ASSIGN-THEN-STORE count={} data={:?}", count, data);
    assert_eq!(count, 8, "Assign-then-Store: carrier must increment 8");
}

