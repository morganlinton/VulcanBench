//! Int8 per-row quantization for token embeddings.
//!
//! Unpack: `x = packed * scale[row]` (F32 output).
//! Pack: mask to 8 bits (U32→U32).

use vyre::ir::{BinOp, BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

const PACK_OP_ID: &str = "vyre-libs::quant::int8_pack";
const UNPACK_OP_ID: &str = "vyre-libs::quant::int8_unpack";

/// Unpack int8: `output[i] = packed[i] * scale[row]` (F32).
#[must_use]
pub fn int8_unpack(packed: &str, scales: &str, output: &str, n: u32, cols: u32) -> Program {
    let rows = n.div_ceil(cols);
    let i = Expr::var("i");

    let row_idx = Expr::BinOp {
        op: BinOp::Div,
        left: Box::new(i.clone()),
        right: Box::new(Expr::u32(cols)),
    };
    let dequant = Expr::mul(
        Expr::cast(DataType::F32, Expr::load(packed, i.clone())),
        Expr::load(scales, row_idx),
    );

    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![Node::Store {
                buffer: output.into(),
                index: i,
                value: dequant,
            }],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(packed, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(scales, 1, BufferAccess::ReadOnly, DataType::F32).with_count(rows),
            BufferDecl::output(output, 2, DataType::F32).with_count(n),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(UNPACK_OP_ID, body)],
    )
}

/// Pack to int8: mask to 8 bits.
#[must_use]
pub fn int8_pack(input: &str, output: &str, n: u32) -> Program {
    let i = Expr::var("i");
    let value = Expr::bitand(Expr::load(input, i.clone()), Expr::u32(0xFF));

    let body = vec![
        Node::let_bind("i", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(i.clone(), Expr::u32(n)),
            vec![Node::Store {
                buffer: output.into(),
                index: i,
                value,
            }],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::output(output, 1, DataType::U32).with_count(n),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(PACK_OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: UNPACK_OP_ID,
        build: || int8_unpack("packed", "scales", "output", 4, 2),
        test_inputs: Some(|| {
            let to_u32 = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_u32(&[10, 20, 30, 40]),
                to_f32(&[0.5, 2.0]),  // 2 rows
            ]]
        }),
        expected_output: Some(|| {
            // row0: [10*0.5, 20*0.5]=[5,10], row1: [30*2, 40*2]=[60,80]
            let out = [5.0_f32, 10.0, 60.0, 80.0];
            let bytes = vyre_primitives::wire::pack_f32_slice(&out);
            vec![vec![bytes]]
        }),
        category: Some("nn"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: PACK_OP_ID,
        build: || int8_pack("input", "output", 4),
        test_inputs: Some(|| {
            let to_u32 = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
            vec![vec![
                to_u32(&[255, 256, 1, 0]),
            ]]
        }),
        expected_output: Some(|| {
            let to_u32 = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
            vec![vec![to_u32(&[255, 0, 1, 0])]]
        }),
        category: Some("nn"),
    }
}
