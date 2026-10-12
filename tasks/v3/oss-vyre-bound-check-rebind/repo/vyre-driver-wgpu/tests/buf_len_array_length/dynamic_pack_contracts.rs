use super::*;

fn dynamic_four_byte_pack_writer_program(words: u32) -> Program {
    let w = Expr::var("w");
    fn byte_expr(k: u32) -> Expr {
        Expr::cast(
            DataType::U32,
            Expr::load(
                "input",
                Expr::add(Expr::mul(Expr::var("w"), Expr::u32(4)), Expr::u32(k)),
            ),
        )
    }
    let body = vec![
        Node::let_bind("w", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(w.clone(), Expr::u32(words)),
            vec![Node::store(
                "out",
                w,
                Expr::bitor(
                    byte_expr(0),
                    Expr::bitor(
                        Expr::shl(byte_expr(1), Expr::u32(8)),
                        Expr::bitor(
                            Expr::shl(byte_expr(2), Expr::u32(16)),
                            Expr::shl(byte_expr(3), Expr::u32(24)),
                        ),
                    ),
                ),
            )],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U8),
            BufferDecl::output("out", 1, DataType::U32).with_count(words),
        ],
        [256, 1, 1],
        body,
    )
}

fn dynamic_four_byte_atomic_pack_writer_program(words: u32) -> Program {
    let w = Expr::var("w");
    fn byte_expr(k: u32) -> Expr {
        Expr::cast(
            DataType::U32,
            Expr::load(
                "input",
                Expr::add(Expr::mul(Expr::var("w"), Expr::u32(4)), Expr::u32(k)),
            ),
        )
    }
    fn atomic_lane(k: u32) -> Node {
        Node::let_bind(
            format!("prev_{k}"),
            Expr::atomic_or(
                "out",
                Expr::var("w"),
                Expr::shl(byte_expr(k), Expr::u32(k * 8)),
            ),
        )
    }
    let body = vec![
        Node::let_bind("w", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(w.clone(), Expr::u32(words)),
            vec![
                atomic_lane(0),
                atomic_lane(1),
                atomic_lane(2),
                atomic_lane(3),
            ],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U8),
            BufferDecl::storage("out", 1, BufferAccess::ReadWrite, DataType::U32).with_count(words),
        ],
        [256, 1, 1],
        body,
    )
}

fn dynamic_four_byte_assigned_atomic_pack_writer_program(words: u32) -> Program {
    let w = Expr::var("w");
    fn byte_expr(k: u32) -> Expr {
        Expr::cast(
            DataType::U32,
            Expr::load(
                "input",
                Expr::add(Expr::mul(Expr::var("w"), Expr::u32(4)), Expr::u32(k)),
            ),
        )
    }
    fn lane_nodes(k: u32) -> Vec<Node> {
        vec![
            Node::let_bind(format!("in_byte_{k}"), Expr::u32(0)),
            Node::if_then_else(
                Expr::eq(Expr::u32(0), Expr::u32(1)),
                vec![Node::assign(
                    &format!("in_byte_{k}"),
                    Expr::u32(b' ' as u32),
                )],
                vec![Node::assign(&format!("in_byte_{k}"), byte_expr(k))],
            ),
            Node::let_bind(
                format!("prev_{k}"),
                Expr::atomic_or(
                    "out",
                    Expr::var("w"),
                    Expr::shl(Expr::var(format!("in_byte_{k}")), Expr::u32(k * 8)),
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
            BufferDecl::storage("out", 1, BufferAccess::ReadWrite, DataType::U32).with_count(words),
        ],
        [256, 1, 1],
        body,
    )
}

fn dynamic_four_byte_clamped_pack_writer_program(words: u32) -> Program {
    let w = Expr::var("w");
    fn source_byte(k: u32) -> Expr {
        let addr = Expr::add(Expr::mul(Expr::var("w"), Expr::u32(4)), Expr::u32(k));
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
        vec![
            Node::let_bind(format!("in_byte_{k}"), Expr::u32(0)),
            Node::if_then_else(
                Expr::eq(Expr::u32(0), Expr::u32(1)),
                vec![Node::assign(
                    &format!("in_byte_{k}"),
                    Expr::u32(b' ' as u32),
                )],
                vec![Node::assign(&format!("in_byte_{k}"), source_byte(k))],
            ),
            Node::let_bind(
                format!("prev_{k}"),
                Expr::atomic_or(
                    "out",
                    Expr::var("w"),
                    Expr::shl(Expr::var(format!("in_byte_{k}")), Expr::u32(k * 8)),
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
            BufferDecl::storage("out", 1, BufferAccess::ReadWrite, DataType::U32).with_count(words),
        ],
        [256, 1, 1],
        body,
    )
}

#[test]
fn dynamic_byte_loads_pack_invocation_indexed_lanes_from_u8_input() {
    let program = dynamic_four_byte_pack_writer_program(4);
    let words = dispatch_and_read_words(&program, b"int x = 1; // trailing\n".to_vec());
    assert_eq!(
        words.get(2).copied().unwrap_or_default().to_le_bytes(),
        [b'1', b';', b' ', b'/'],
        "invocation-indexed U8 loads must preserve byte-addressed lanes before byte compaction."
    );
}

#[test]
fn dynamic_byte_loads_atomic_or_pack_invocation_indexed_lanes_from_u8_input() {
    let program = dynamic_four_byte_atomic_pack_writer_program(4);
    let words = dispatch_and_read_words(&program, b"int x = 1; // trailing\n".to_vec());
    assert_eq!(
        words.get(2).copied().unwrap_or_default().to_le_bytes(),
        [b'1', b';', b' ', b'/'],
        "atomic-or byte packing must preserve invocation-indexed U8 lanes before byte compaction."
    );
}

#[test]
fn assigned_dynamic_byte_loads_atomic_or_pack_invocation_indexed_lanes_from_u8_input() {
    let program = dynamic_four_byte_assigned_atomic_pack_writer_program(4);
    let words = dispatch_and_read_words(&program, b"int x = 1; // trailing\n".to_vec());
    assert_eq!(
        words.get(2).copied().unwrap_or_default().to_le_bytes(),
        [b'1', b';', b' ', b'/'],
        "assigned byte variables must preserve invocation-indexed U8 lanes before byte compaction."
    );
}

#[test]
fn clamped_dynamic_byte_loads_atomic_or_pack_invocation_indexed_lanes_from_u8_input() {
    let program = dynamic_four_byte_clamped_pack_writer_program(4);
    let words = dispatch_and_read_words(&program, b"int x = 1; // trailing\n".to_vec());
    assert_eq!(
        words.get(2).copied().unwrap_or_default().to_le_bytes(),
        [b'1', b';', b' ', b'/'],
        "buf_len-clamped byte variables must preserve invocation-indexed U8 lanes before byte compaction."
    );
}

