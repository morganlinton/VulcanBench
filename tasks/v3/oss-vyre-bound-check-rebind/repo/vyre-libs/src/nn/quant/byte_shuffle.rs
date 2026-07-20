//! Byte shuffle for Brotli compression preparation.
//!
//! Category A composition  -  gather/scatter reordering of bytes to
//! improve Brotli compression ratio. Groups similar bytes together
//! so the entropy coder sees longer runs.
//!
//! Pattern: transpose the byte matrix [N, element_size] →
//! [element_size, N] so all MSBs are adjacent, then all next bytes, etc.
//!
//! Used in the Parameter Golf submission pipeline after quantization
//! and before Brotli-11 compression.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Program};

use crate::builder::build_indexed_map;

const OP_ID: &str = "vyre-libs::quant::byte_shuffle";

/// Build a Program that transposes byte layout for Brotli compression.
///
/// `input[n]` → `output[n]` where output is byte-transposed.
/// `elem_bytes` is the byte width of each element (e.g. 3 for int6 groups).
///
/// For `n` elements each `elem_bytes` wide, the output puts all
/// byte-0s first, then byte-1s, etc.
///
/// # Errors
///
/// Returns `Err` if `n == 0` or `elem_bytes == 0`.
pub fn byte_shuffle(input: &str, output: &str, n: u32, elem_bytes: u32) -> Result<Program, String> {
    if n == 0 || elem_bytes == 0 {
        return Err("Fix: byte_shuffle requires non-zero dimensions".to_string());
    }

    let total = n * elem_bytes;
    let input_decl =
        BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(total);
    let out_decl = BufferDecl::output(output, 1, DataType::U32)
        .with_count(total)
        .with_output_byte_range(0..(total as usize).saturating_mul(4));

    Ok(build_indexed_map(
        OP_ID,
        vec![input_decl, out_decl],
        output,
        total,
        [64, 1, 1],
        |i| {
            let elem_idx = Expr::div(i.clone(), Expr::u32(elem_bytes));
            let byte_idx = Expr::sub(
                i.clone(),
                Expr::mul(elem_idx.clone(), Expr::u32(elem_bytes)),
            );
            let dst_idx = Expr::add(Expr::mul(byte_idx, Expr::u32(n)), elem_idx);
            (dst_idx, Expr::load(input, i))
        },
    ))
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || {
            byte_shuffle("input", "output", 3, 2)
                .unwrap_or_else(|error| crate::invalid_program(OP_ID, format!("Fix: byte_shuffle fixture must build: {error}")))
        },
        test_inputs: Some(|| vec![vec![
            // 3 elements × 2 bytes: [a0,a1, b0,b1, c0,c1]
            vyre_primitives::wire::pack_u32_slice(&[10u32, 11, 20, 21, 30, 31]),
        ]]),
        expected_output: Some(|| vec![vec![
            // Byte-transposed: [a0,b0,c0, a1,b1,c1]
            vyre_primitives::wire::pack_u32_slice(&[10u32, 20, 30, 11, 21, 31]),
        ]]),
        category: Some("nn"),
    }
}
