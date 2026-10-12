use super::*;

fn dynamic_offset_scatter_pack_writer_program(words: u32) -> Program {
    let w = Expr::var("w");
    fn i_expr(k: u32) -> Expr {
        Expr::add(Expr::mul(Expr::var("w"), Expr::u32(4)), Expr::u32(k))
    }
    fn source_byte(k: u32) -> Expr {
        let addr = i_expr(k);
        let len = Expr::buf_len("input");
        let safe_addr = Expr::select(
            Expr::lt(addr.clone(), len.clone()),
            addr,
            Expr::saturating_sub(len, Expr::u32(1)),
        );
        Expr::bitand(
            Expr::cast(DataType::U32, Expr::load("input", safe_addr)),
            Expr::u32(0xFF),
        )
    }
    fn lane_nodes(k: u32) -> Vec<Node> {
        let i = i_expr(k);
        vec![
            Node::let_bind(format!("off_{k}"), Expr::load("offsets", i.clone())),
            Node::let_bind(
                format!("out_pos_{k}"),
                Expr::saturating_sub(Expr::var(format!("off_{k}")), Expr::u32(1)),
            ),
            Node::let_bind(
                format!("out_word_idx_{k}"),
                Expr::div(Expr::var(format!("out_pos_{k}")), Expr::u32(4)),
            ),
            Node::let_bind(
                format!("out_shift_{k}"),
                Expr::mul(
                    Expr::rem(Expr::var(format!("out_pos_{k}")), Expr::u32(4)),
                    Expr::u32(8),
                ),
            ),
            Node::let_bind(format!("in_byte_{k}"), source_byte(k)),
            Node::let_bind(
                format!("prev_{k}"),
                Expr::atomic_or(
                    "out",
                    Expr::var(format!("out_word_idx_{k}")),
                    Expr::shl(
                        Expr::var(format!("in_byte_{k}")),
                        Expr::var(format!("out_shift_{k}")),
                    ),
                ),
            ),
        ]
    }
    let mut lanes = Vec::new();
    for k in 0..4 {
        lanes.extend(lane_nodes(k));
    }
    let body = vec![
        Node::let_bind("w", Expr::InvocationId { axis: 0 }),
        Node::if_then(Expr::lt(w.clone(), Expr::u32(words)), lanes),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U8),
            BufferDecl::storage("offsets", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words * 4),
            BufferDecl::storage("out", 2, BufferAccess::ReadWrite, DataType::U32).with_count(words),
        ],
        [256, 1, 1],
        body,
    )
}

fn dynamic_masked_comment_scatter_pack_writer_program(words: u32) -> Program {
    let w = Expr::var("w");
    fn i_expr(k: u32) -> Expr {
        Expr::add(Expr::mul(Expr::var("w"), Expr::u32(4)), Expr::u32(k))
    }
    fn source_byte(i: Expr) -> Expr {
        let len = Expr::buf_len("input");
        let safe_addr = Expr::select(
            Expr::lt(i.clone(), len.clone()),
            i,
            Expr::saturating_sub(len, Expr::u32(1)),
        );
        Expr::bitand(
            Expr::cast(DataType::U32, Expr::load("input", safe_addr)),
            Expr::u32(0xFF),
        )
    }
    fn lane_nodes(k: u32, total_bytes: u32) -> Vec<Node> {
        let i = i_expr(k);
        vec![Node::if_then(
            Expr::lt(i.clone(), Expr::u32(total_bytes)),
            vec![
                Node::let_bind(format!("m_{k}"), Expr::load("mask", i.clone())),
                Node::let_bind(format!("off_{k}"), Expr::load("offsets", i.clone())),
                Node::if_then(
                    Expr::eq(Expr::var(format!("m_{k}")), Expr::u32(1)),
                    vec![
                        Node::let_bind(format!("cm_{k}"), Expr::load("comment_mask", i.clone())),
                        Node::let_bind(format!("in_byte_{k}"), Expr::u32(0)),
                        Node::if_then_else(
                            Expr::eq(Expr::var(format!("cm_{k}")), Expr::u32(2)),
                            vec![Node::assign(
                                &format!("in_byte_{k}"),
                                Expr::u32(b' ' as u32),
                            )],
                            vec![Node::assign(&format!("in_byte_{k}"), source_byte(i))],
                        ),
                        Node::let_bind(
                            format!("out_pos_{k}"),
                            Expr::saturating_sub(Expr::var(format!("off_{k}")), Expr::u32(1)),
                        ),
                        Node::let_bind(
                            format!("out_word_idx_{k}"),
                            Expr::div(Expr::var(format!("out_pos_{k}")), Expr::u32(4)),
                        ),
                        Node::let_bind(
                            format!("out_shift_{k}"),
                            Expr::mul(
                                Expr::rem(Expr::var(format!("out_pos_{k}")), Expr::u32(4)),
                                Expr::u32(8),
                            ),
                        ),
                        Node::let_bind(
                            format!("prev_{k}"),
                            Expr::atomic_or(
                                "out",
                                Expr::var(format!("out_word_idx_{k}")),
                                Expr::shl(
                                    Expr::var(format!("in_byte_{k}")),
                                    Expr::var(format!("out_shift_{k}")),
                                ),
                            ),
                        ),
                    ],
                ),
            ],
        )]
    }
    let total_bytes = words * 4;
    let mut lanes = Vec::new();
    for k in 0..4 {
        lanes.extend(lane_nodes(k, total_bytes));
    }
    let body = vec![
        Node::let_bind("w", Expr::InvocationId { axis: 0 }),
        Node::if_then(Expr::lt(w.clone(), Expr::u32(words)), lanes),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U8),
            BufferDecl::storage("mask", 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(total_bytes),
            BufferDecl::storage("comment_mask", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(total_bytes),
            BufferDecl::storage("offsets", 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(total_bytes),
            BufferDecl::storage("out", 4, BufferAccess::ReadWrite, DataType::U32).with_count(words),
        ],
        [256, 1, 1],
        body,
    )
}

#[test]
fn dynamic_offset_scatter_packs_invocation_indexed_lanes_from_u8_input() {
    let program = dynamic_offset_scatter_pack_writer_program(4);
    let offsets: Vec<u32> = (1..=16).collect();
    let words = dispatch_and_read_words_with_inputs(
        &program,
        vec![
            b"int x = 1; // trailing\n".to_vec(),
            u32_bytes(&offsets),
            vec![0u8; 16],
        ],
    );
    assert_eq!(
        words.get(2).copied().unwrap_or_default().to_le_bytes(),
        [b'1', b';', b' ', b'/'],
        "offset-driven byte scatter must preserve invocation-indexed U8 lanes before byte compaction."
    );
}

#[test]
fn dynamic_masked_comment_scatter_packs_expected_lanes_from_u8_input() {
    let program = dynamic_masked_comment_scatter_pack_writer_program(256);
    let keep_prefix = [
        1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1,
        1, 1, 1, 1, 0, 0, 0, 0, 0, 0,
    ];
    let comment_prefix = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let offsets_prefix = [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 13, 14, 15,
        16, 17, 18, 19, 20, 21, 22, 23, 24, 24, 24, 24, 24, 24, 24,
    ];
    let mut keep = vec![0u32; 1024];
    let mut comment = vec![0u32; 1024];
    let mut offsets = vec![24u32; 1024];
    keep[..keep_prefix.len()].copy_from_slice(&keep_prefix);
    comment[..comment_prefix.len()].copy_from_slice(&comment_prefix);
    offsets[..offsets_prefix.len()].copy_from_slice(&offsets_prefix);
    let words = dispatch_and_read_words_with_inputs(
        &program,
        vec![
            b"int x = 1; // trailing\nint y = 2;\n".to_vec(),
            u32_bytes(&keep),
            u32_bytes(&comment),
            u32_bytes(&offsets),
            vec![0u8; 1024],
        ],
    );
    let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    assert_eq!(
        &bytes[..24],
        b"int x = 1;  \nint y = 2;\n",
        "mask/comment-driven byte scatter must match simple line comment compaction."
    );
}
