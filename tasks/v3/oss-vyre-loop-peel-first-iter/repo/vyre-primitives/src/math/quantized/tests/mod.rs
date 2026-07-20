//! Unit tests for packed INT4 quantized primitives.

use super::*;

fn pack_i4_matrix_rows(rows: &[Vec<i32>]) -> Vec<u32> {
    let cols = rows.first().map_or(0, Vec::len) as u32;
    let words_per_row = i4_packed_words(cols) as usize;
    let mut out = Vec::with_capacity(rows.len() * words_per_row);
    for row in rows {
        let mut packed = pack_i4x8_cpu(row);
        packed.resize(words_per_row, 0);
        out.extend_from_slice(&packed);
    }
    out
}

#[path = "pack_unpack_contracts.rs"]
mod pack_unpack_contracts;
#[path = "dot_contracts.rs"]
mod dot_contracts;
#[path = "matvec_contracts.rs"]
mod matvec_contracts;
#[path = "batched_matmul_contracts.rs"]
mod batched_matmul_contracts;
#[path = "layout_contracts.rs"]
mod layout_contracts;
#[path = "zero_shape_contracts.rs"]
mod zero_shape_contracts;
