//! Int6 pack/unpack for GPTQ-SDClip quantized weights.
//!
//! Pack layout: 4 int6 values per 3 bytes.
//! Scale/zero buffers are F32. Packed data is U32.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Program};

use crate::builder::build_indexed_map;

const PACK_OP_ID: &str = "vyre-libs::quant::int6_pack";
const UNPACK_OP_ID: &str = "vyre-libs::quant::int6_unpack";

/// Unpack int6 values: `output[i] = (packed[i] & 0x3F) * scale[block] + zero[block]`.
///
/// `packed[n]` (U32 raw), `scale[n_blocks]` (F32), `zero[n_blocks]` (F32),
/// `output[n]` (F32 dequantized).
#[must_use]
pub fn int6_unpack(
    packed: &str,
    scale: &str,
    zero: &str,
    output: &str,
    n: u32,
    block_size: u32,
) -> Program {
    let n_blocks = n.div_ceil(block_size);

    build_indexed_map(
        UNPACK_OP_ID,
        vec![
            BufferDecl::storage(packed, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(scale, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(n_blocks),
            BufferDecl::storage(zero, 2, BufferAccess::ReadOnly, DataType::F32)
                .with_count(n_blocks),
            BufferDecl::output(output, 3, DataType::F32).with_count(n),
        ],
        output,
        n,
        [64, 1, 1],
        |i| {
            let block_idx = Expr::div(i.clone(), Expr::u32(block_size));
            let masked = Expr::bitand(Expr::load(packed, i.clone()), Expr::u32(0x3F));
            let dequant = Expr::add(
                Expr::mul(
                    Expr::cast(DataType::F32, masked),
                    Expr::load(scale, block_idx.clone()),
                ),
                Expr::load(zero, block_idx),
            );
            (i, dequant)
        },
    )
}

/// Pack to int6: mask to 6 bits.
#[must_use]
pub fn int6_pack(input: &str, output: &str, n: u32) -> Program {
    build_indexed_map(
        PACK_OP_ID,
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::output(output, 1, DataType::U32).with_count(n),
        ],
        output,
        n,
        [64, 1, 1],
        |i| {
            let value = Expr::bitand(Expr::load(input, i.clone()), Expr::u32(0x3F));
            (i, value)
        },
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: UNPACK_OP_ID,
        build: || int6_unpack("packed", "scale", "zero", "output", 4, 4),
        test_inputs: Some(|| {
            let to_u32 = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
            let to_f32 = |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            vec![vec![
                to_u32(&[63, 32, 1, 0]),     // packed (6-bit values)
                to_f32(&[0.1]),               // scale (1 block)
                to_f32(&[0.0]),               // zero (1 block)
            ]]
        }),
        expected_output: Some(|| {
            // 63*0.1=6.3, 32*0.1=3.2, 1*0.1=0.1, 0*0.1=0.0
            let out = [6.3_f32, 3.2, 0.1, 0.0];
            let bytes = vyre_primitives::wire::pack_f32_slice(&out);
            vec![vec![bytes]]
        }),
        category: Some("nn"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: PACK_OP_ID,
        build: || int6_pack("input", "output", 4),
        test_inputs: Some(|| {
            let to_u32 = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
            vec![vec![
                to_u32(&[63, 100, 1, 0]),
            ]]
        }),
        expected_output: Some(|| {
            let to_u32 = |w: &[u32]| vyre_primitives::wire::pack_u32_slice(w);
            // 63&0x3F=63, 100&0x3F=36, 1&0x3F=1, 0
            vec![vec![to_u32(&[63, 36, 1, 0])]]
        }),
        category: Some("nn"),
    }
}
