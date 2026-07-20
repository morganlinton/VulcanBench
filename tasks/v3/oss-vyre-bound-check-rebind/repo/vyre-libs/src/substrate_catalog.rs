use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::builder::{
    build_indexed_map, strided_accumulate_child, INDEXED_MAP_OP_ID, STRIDED_ACCUMULATE_OP_ID,
};
use crate::harness::OpEntry;
use crate::region::wrap_anonymous;

fn u32s(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}

fn indexed_map_program() -> Program {
    build_indexed_map(
        INDEXED_MAP_OP_ID,
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32).with_count(4),
            BufferDecl::output("out", 1, DataType::U32).with_count(4),
        ],
        "out",
        4,
        [4, 1, 1],
        |i| (i.clone(), Expr::add(Expr::load("input", i), Expr::u32(1))),
    )
}

fn strided_accumulate_program() -> Program {
    let tile = 4;
    let body = vec![
        Node::let_bind("local", Expr::LocalId { axis: 0 }),
        strided_accumulate_child(
            STRIDED_ACCUMULATE_OP_ID,
            tile,
            1,
            4,
            "acc",
            Expr::u32(0),
            "scratch",
            |idx, acc| Expr::add(acc, Expr::load("values", idx)),
        ),
        Node::barrier(),
        Node::if_then(
            Expr::eq(Expr::var("local"), Expr::u32(0)),
            vec![Node::store(
                "out",
                Expr::u32(0),
                Expr::load("scratch", Expr::u32(0)),
            )],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage("values", 0, BufferAccess::ReadOnly, DataType::U32).with_count(4),
            BufferDecl::workgroup("scratch", tile, DataType::U32),
            BufferDecl::output("out", 1, DataType::U32).with_count(1),
        ],
        [tile, 1, 1],
        vec![wrap_anonymous(STRIDED_ACCUMULATE_OP_ID, body)],
    )
}

inventory::submit! {
    OpEntry {
        id: INDEXED_MAP_OP_ID,
        build: indexed_map_program,
        test_inputs: Some(|| vec![vec![
            u32s(&[1, 2, 3, 4]),
        ]]),
        expected_output: Some(|| vec![vec![u32s(&[2, 3, 4, 5])]]),
        category: None,
    }
}

inventory::submit! {
    OpEntry {
        id: STRIDED_ACCUMULATE_OP_ID,
        build: strided_accumulate_program,
        test_inputs: Some(|| vec![vec![
            u32s(&[7, 11, 13, 17]),
        ]]),
        expected_output: Some(|| vec![vec![u32s(&[7])]]),
        category: None,
    }
}
