// (use super::* removed  -  flat-included into wire_roundtrip_proptest_suite scope)

#[test]
fn every_expression_variant_roundtrips_in_one_program() {
    let opaque_expr = || {
        Expr::Opaque(Arc::new(TestOpaqueExpr {
            payload: vec![0xde, 0xad, 0xbe, 0xef],
        }))
    };

    let program = Program::wrapped(
        vec![
            BufferDecl::output("out", 0, DataType::U32).with_count(8),
            BufferDecl::read("input", 1, DataType::U32).with_count(8),
            BufferDecl::read_write("rw", 2, DataType::U32).with_count(8),
        ],
        [1, 1, 1],
        vec![
            Node::let_bind("lit_u32", Expr::LitU32(7)),
            Node::let_bind("lit_i32", Expr::LitI32(-7)),
            Node::let_bind("lit_f32", Expr::LitF32(1.25)),
            Node::let_bind("lit_bool", Expr::LitBool(true)),
            Node::let_bind("var", Expr::var("lit_u32")),
            Node::let_bind("load", Expr::load("input", Expr::u32(0))),
            Node::let_bind("buf_len", Expr::buf_len("out")),
            Node::let_bind("invocation", Expr::InvocationId { axis: 0 }),
            Node::let_bind("workgroup", Expr::WorkgroupId { axis: 1 }),
            Node::let_bind("local", Expr::LocalId { axis: 2 }),
            Node::let_bind(
                "binop",
                Expr::BinOp {
                    op: BinOp::RotateLeft,
                    left: Box::new(Expr::u32(1)),
                    right: Box::new(Expr::u32(2)),
                },
            ),
            Node::let_bind(
                "unop",
                Expr::UnOp {
                    op: UnOp::IsFinite,
                    operand: Box::new(Expr::LitF32(1.25)),
                },
            ),
            Node::let_bind(
                "call",
                Expr::Call {
                    op_id: "call::all_variants".into(),
                    args: vec![Expr::u32(1), Expr::u32(2)],
                },
            ),
            Node::let_bind(
                "select",
                Expr::Select {
                    cond: Box::new(Expr::LitBool(true)),
                    true_val: Box::new(Expr::u32(1)),
                    false_val: Box::new(Expr::u32(0)),
                },
            ),
            Node::let_bind(
                "cast",
                Expr::Cast {
                    target: DataType::F64,
                    value: Box::new(Expr::u32(9)),
                },
            ),
            Node::let_bind(
                "fma",
                Expr::Fma {
                    a: Box::new(Expr::u32(2)),
                    b: Box::new(Expr::u32(3)),
                    c: Box::new(Expr::u32(4)),
                },
            ),
            Node::let_bind(
                "atomic",
                Expr::Atomic {
                    op: AtomicOp::CompareExchange,
                    buffer: "rw".into(),
                    index: Box::new(Expr::u32(0)),
                    expected: Some(Box::new(Expr::u32(1))),
                    value: Box::new(Expr::u32(2)),
                    ordering: MemoryOrdering::SeqCst,
                },
            ),
            Node::let_bind(
                "subgroup_ballot",
                Expr::SubgroupBallot {
                    cond: Box::new(Expr::LitBool(true)),
                },
            ),
            Node::let_bind(
                "subgroup_shuffle",
                Expr::SubgroupShuffle {
                    value: Box::new(Expr::u32(5)),
                    lane: Box::new(Expr::u32(1)),
                },
            ),
            Node::let_bind(
                "subgroup_add",
                Expr::subgroup_add(Expr::u32(6)),
            ),
            Node::let_bind("opaque", opaque_expr()),
            Node::Return,
        ],
    );

    let decoded = Program::from_wire(
        &program
            .to_wire()
            .expect("Fix: full expression-surface program must encode"),
    )
    .expect("Fix: full expression-surface program must decode");

    assert_eq!(decoded, program);
}

#[test]
fn every_statement_variant_roundtrips_in_one_program() {
    let expr = || Expr::BinOp {
        op: BinOp::Add,
        left: Box::new(Expr::LitU32(1)),
        right: Box::new(Expr::Opaque(Arc::new(TestOpaqueExpr {
            payload: vec![0x00, 0xff, 0xc0, 0xaf],
        }))),
    };

    let region_body = Arc::new(vec![
        Node::AsyncStore {
            source: "rw".into(),
            destination: "bytes_out".into(),
            offset: Box::new(Expr::LitU32(0)),
            size: Box::new(Expr::LitU32(4)),
            tag: "region-tag".into(),
        },
        Node::Return,
    ]);

    let program = Program::wrapped(
        vec![
            BufferDecl::output("out", 0, DataType::U32).with_count(8),
            BufferDecl::read("input", 1, DataType::U32).with_count(8),
            BufferDecl::read_write("rw", 2, DataType::U32).with_count(8),
            BufferDecl::read("bytes_in", 3, DataType::Bytes).with_count(16),
            BufferDecl::read_write("bytes_out", 4, DataType::Bytes).with_count(16),
            BufferDecl::read("counts", 5, DataType::U32).with_count(8),
            BufferDecl::workgroup("scratch", 4, DataType::U32),
        ],
        [1, 1, 1],
        vec![
            Node::Let {
                name: "x".into(),
                value: expr(),
            },
            Node::Assign {
                name: "x".into(),
                value: Expr::SubgroupShuffle {
                    value: Box::new(Expr::LitI32(-4)),
                    lane: Box::new(Expr::InvocationId { axis: 1 }),
                },
            },
            Node::Store {
                buffer: "out".into(),
                index: Expr::LitU32(0),
                value: Expr::Atomic {
                    op: AtomicOp::CompareExchange,
                    buffer: "rw".into(),
                    index: Box::new(Expr::LitU32(1)),
                    expected: Some(Box::new(Expr::LitU32(2))),
                    value: Box::new(Expr::LitU32(3)),
                    ordering: MemoryOrdering::SeqCst,
                },
            },
            Node::If {
                cond: Expr::SubgroupBallot {
                    cond: Box::new(Expr::LitBool(true)),
                },
                then: vec![Node::barrier()],
                otherwise: vec![Node::Block(vec![Node::Return])],
            },
            Node::Loop {
                var: "i".into(),
                from: Expr::LitU32(0),
                to: Expr::LitU32(2),
                body: vec![Node::Store {
                    buffer: "rw".into(),
                    index: Expr::Var("x".into()),
                    value: Expr::Cast {
                        target: DataType::F64,
                        value: Box::new(Expr::LitF32(1.25)),
                    },
                }],
            },
            Node::IndirectDispatch {
                count_buffer: "counts".into(),
                count_offset: 8,
            },
            Node::AsyncLoad {
                source: "bytes_in".into(),
                destination: "rw".into(),
                offset: Box::new(Expr::LitU32(0)),
                size: Box::new(Expr::LitU32(4)),
                tag: "stream-tag".into(),
            },
            Node::AsyncStore {
                source: "rw".into(),
                destination: "bytes_out".into(),
                offset: Box::new(Expr::LitU32(4)),
                size: Box::new(Expr::LitU32(4)),
                tag: "stream-tag".into(),
            },
            Node::AsyncWait {
                tag: "stream-tag".into(),
            },
            Node::Trap {
                address: Box::new(Expr::BufLen {
                    buffer: "input".into(),
                }),
                tag: "trap-tag".into(),
            },
            Node::Resume {
                tag: "trap-tag".into(),
            },
            Node::Region {
                generator: "gen".into(),
                source_region: Some(vyre_foundation::ir::model::expr::GeneratorRef {
                    name: "src".to_string(),
                }),
                body: region_body,
            },
            Node::Opaque(Arc::new(TestOpaqueNode {
                payload: vec![0x00, 0xff, 0xc0, 0xaf, 0x80],
            })),
            Node::Return,
        ],
    );

    let decoded = Program::from_wire(
        &program
            .to_wire()
            .expect("Fix: full node-surface program must encode"),
    )
    .expect("Fix: full node-surface program must decode");

    assert_eq!(decoded, program);
}
