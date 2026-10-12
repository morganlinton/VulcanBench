//! CPU reference oracles and packing helpers for packed INT4 primitives.

// Used only by the CPU-parity oracle fns below, which are all gated behind
// `cfg(any(test, feature = "cpu-parity"))`; gate the import to match so a
// production build (cpu-parity off) carries neither the helpers nor a dead import.
#[cfg(any(test, feature = "cpu-parity"))]
use super::{i4_packed_words, I4_LANES_PER_WORD};

/// Pack signed INT4 values into u32 words using the CPU reference layout.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn pack_i4x8_cpu(values: &[i32]) -> Vec<u32> {
    let mut out = Vec::new();
    // Discarding the error and returning a partial/empty buffer makes a
    // GPU-vs-CPU parity assertion pass on empty==empty, silently masking a
    // divergence (Law 10 / Law 6). Fail loud; callers use try_pack_i4x8_cpu_into.
    if let Err(error) = try_pack_i4x8_cpu_into(values, &mut out) {
        panic!("vyre-primitives pack_i4x8 CPU reference failed: {error}");
    }
    out
}

/// Pack signed INT4 lanes into caller-owned u32 storage.
/// Pack signed INT4 values into caller-owned u32 word storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn pack_i4x8_cpu_into(values: &[i32], out: &mut Vec<u32>) {
    // Clearing to empty on failure silently masks a parity divergence
    // (Law 10 / Law 6). Fail loud; callers use try_pack_i4x8_cpu_into.
    if let Err(error) = try_pack_i4x8_cpu_into(values, out) {
        panic!("vyre-primitives pack_i4x8 CPU reference failed: {error}");
    }
}

/// Fallible pack of signed INT4 values into caller-owned u32 word storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_pack_i4x8_cpu_into(values: &[i32], out: &mut Vec<u32>) -> Result<(), String> {
    let lane_count = u32::try_from(values.len()).map_err(|_| {
        format!(
            "pack_i4x8 CPU oracle received {} lanes, exceeding u32 lane count. Fix: shard quantized activations before parity evaluation.",
            values.len()
        )
    })?;
    let word_count = i4_packed_words(lane_count) as usize;
    if word_count > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            word_count - out.len(),
            "quantized INT4 CPU oracle",
            "pack_i4x8 output words",
        )?;
    }
    out.clear();
    out.resize(word_count, 0);
    for (index, &value) in values.iter().enumerate() {
        let clamped = value.clamp(-8, 7);
        let nibble = (clamped as i8 as u8) & 0x0F;
        let word = index / I4_LANES_PER_WORD as usize;
        let shift = (index % I4_LANES_PER_WORD as usize) * 4;
        out[word] |= u32::from(nibble) << shift;
    }
    Ok(())
}

/// Unpack signed INT4 lanes from u32 words.
/// Unpack signed INT4 values from u32 words using the CPU reference layout.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn unpack_i4x8_cpu(packed: &[u32], lane_count: u32) -> Vec<i32> {
    let mut out = Vec::new();
    try_unpack_i4x8_cpu_into(packed, lane_count, &mut out)
        .unwrap_or_else(|error| panic!("{error}"));
    out
}

/// Unpack signed INT4 lanes into caller-owned output storage.
/// Unpack signed INT4 values into caller-owned lane storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn unpack_i4x8_cpu_into(packed: &[u32], lane_count: u32, out: &mut Vec<i32>) {
    try_unpack_i4x8_cpu_into(packed, lane_count, out).unwrap_or_else(|error| panic!("{error}"));
}

/// Fallible unpack of signed INT4 values into caller-owned lane storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_unpack_i4x8_cpu_into(
    packed: &[u32],
    lane_count: u32,
    out: &mut Vec<i32>,
) -> Result<(), String> {
    let lanes = lane_count as usize;
    if lanes > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            lanes - out.len(),
            "quantized INT4 CPU oracle",
            "unpack_i4x8 output lanes",
        )?;
    }
    out.clear();
    for lane in 0..lanes {
        out.push(extract_i4_lane(packed, lane));
    }
    Ok(())
}

/// Packed signed INT4 dot-product CPU oracle.
/// Compute the CPU reference i32 dot product over packed signed INT4 lanes.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn i4x8_dot_i32_cpu(lhs_packed: &[u32], rhs_packed: &[u32], lane_count: u32) -> i32 {
    let mut acc = 0i32;
    for lane in 0..lane_count as usize {
        let lhs = extract_i4_lane(lhs_packed, lane);
        let rhs = extract_i4_lane(rhs_packed, lane);
        acc = acc.wrapping_add(lhs.wrapping_mul(rhs));
    }
    acc
}

/// Packed signed INT4 scaled dot-product CPU oracle.
/// Compute the CPU reference scaled f32 dot product over packed signed INT4 lanes.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn i4x8_dot_f32_scaled_cpu(
    lhs_packed: &[u32],
    rhs_packed: &[u32],
    lhs_scale: f32,
    rhs_scale: f32,
    lane_count: u32,
) -> f32 {
    let mut acc = 0.0_f32;
    for lane in 0..lane_count as usize {
        let lhs = extract_i4_lane(lhs_packed, lane) as f32;
        let rhs = extract_i4_lane(rhs_packed, lane) as f32;
        acc += lhs * rhs;
    }
    acc * lhs_scale * rhs_scale
}

/// Packed signed INT4 scaled matrix-vector CPU oracle.
/// Compute the CPU reference row-scaled packed INT4 matrix-vector product.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn i4x8_matvec_f32_scaled_cpu(
    weights_packed: &[u32],
    x: &[f32],
    row_scales: &[f32],
    rows: u32,
    cols: u32,
) -> Vec<f32> {
    let words_per_row = i4_packed_words(cols) as usize;
    let mut out = vec![0.0_f32; rows as usize];
    for row in 0..rows as usize {
        let row_base = row * words_per_row;
        let mut acc = 0.0_f32;
        for col in 0..cols as usize {
            acc += extract_i4_lane(&weights_packed[row_base..], col) as f32
                * x.get(col).copied().unwrap_or(0.0);
        }
        out[row] = acc * row_scales.get(row).copied().unwrap_or(0.0);
    }
    out
}

/// Batched packed signed INT4 scaled matrix-vector CPU oracle.
/// Compute the CPU reference batched row-scaled packed INT4 matrix-vector product.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn i4x8_batched_matvec_f32_scaled_cpu(
    weights_packed: &[u32],
    x_batches: &[f32],
    row_scales: &[f32],
    batch: u32,
    rows: u32,
    cols: u32,
) -> Vec<f32> {
    let mut out = vec![0.0_f32; (batch * rows) as usize];
    for batch_index in 0..batch as usize {
        let x_start = batch_index * cols as usize;
        let x_end = x_start + cols as usize;
        let row_out = i4x8_matvec_f32_scaled_cpu(
            weights_packed,
            x_batches.get(x_start..x_end).unwrap_or(&[]),
            row_scales,
            rows,
            cols,
        );
        let out_start = batch_index * rows as usize;
        out[out_start..out_start + rows as usize].copy_from_slice(&row_out);
    }
    out
}

/// Batched packed signed INT4 scaled matrix-matrix CPU oracle.
/// Compute the CPU reference batched packed-activation INT4 matrix multiply.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn i4x8_batched_matmul_f32_scaled_cpu(
    weights_packed: &[u32],
    activation_batches_packed: &[u32],
    row_scales: &[f32],
    batch_scales: &[f32],
    batch: u32,
    rows: u32,
    cols: u32,
) -> Vec<f32> {
    let words_per_row = i4_packed_words(cols) as usize;
    let mut out = vec![0.0_f32; (batch * rows) as usize];
    for batch_index in 0..batch as usize {
        let activation_base = batch_index * words_per_row;
        for row in 0..rows as usize {
            let weight_base = row * words_per_row;
            let mut acc = 0.0_f32;
            for col in 0..cols as usize {
                acc += extract_i4_lane(&weights_packed[weight_base..], col) as f32
                    * extract_i4_lane(&activation_batches_packed[activation_base..], col) as f32;
            }
            out[batch_index * rows as usize + row] = acc
                * row_scales.get(row).copied().unwrap_or(0.0)
                * batch_scales.get(batch_index).copied().unwrap_or(0.0);
        }
    }
    out
}

/// Batched packed signed INT4 scaled matmul top-1 CPU oracle.
/// Compute CPU reference top-1 scores and indices for packed INT4 batched matmul.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn i4x8_batched_matmul_top1_f32_scaled_cpu(
    weights_packed: &[u32],
    activation_batches_packed: &[u32],
    row_scales: &[f32],
    batch_scales: &[f32],
    batch: u32,
    rows: u32,
    cols: u32,
) -> (Vec<f32>, Vec<u32>) {
    let logits = i4x8_batched_matmul_f32_scaled_cpu(
        weights_packed,
        activation_batches_packed,
        row_scales,
        batch_scales,
        batch,
        rows,
        cols,
    );
    let mut scores = vec![f32::MIN; batch as usize];
    let mut indices = vec![0_u32; batch as usize];
    for batch_index in 0..batch as usize {
        let row_start = batch_index * rows as usize;
        for row in 0..rows as usize {
            let score = logits[row_start + row];
            if score > scores[batch_index] {
                scores[batch_index] = score;
                indices[batch_index] = row as u32;
            }
        }
    }
    (scores, indices)
}

#[cfg(any(test, feature = "cpu-parity"))]
fn extract_i4_lane(packed: &[u32], lane: usize) -> i32 {
    let word = packed
        .get(lane / I4_LANES_PER_WORD as usize)
        .copied()
        .unwrap_or(0);
    let shift = (lane % I4_LANES_PER_WORD as usize) * 4;
    let nibble = ((word >> shift) & 0x0F) as i32;
    if nibble & 0x8 == 0 {
        nibble
    } else {
        nibble - 16
    }
}
