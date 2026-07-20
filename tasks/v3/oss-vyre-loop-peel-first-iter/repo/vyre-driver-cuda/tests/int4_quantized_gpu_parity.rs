//! Live CUDA parity for packed INT4 quantized primitives.

use vyre_driver::DispatchConfig;
use vyre_driver_cuda::CudaBackend;

fn pack_i4x8(values: &[i32]) -> Vec<u32> {
    let mut out = vec![0_u32; values.len().div_ceil(8)];
    for (index, &value) in values.iter().enumerate() {
        let clamped = value.clamp(-8, 7);
        let nibble = (clamped as i8 as u8) & 0x0f;
        let word = index / 8;
        let shift = (index % 8) * 4;
        out[word] |= u32::from(nibble) << shift;
    }
    out
}

fn extract_i4(packed: &[u32], lane: usize) -> i32 {
    let word = packed.get(lane / 8).copied().unwrap_or(0);
    let nibble = ((word >> ((lane % 8) * 4)) & 0x0f) as i32;
    if nibble & 0x8 == 0 {
        nibble
    } else {
        nibble - 16
    }
}

fn dot_scaled_oracle(
    lhs_packed: &[u32],
    rhs_packed: &[u32],
    lhs_scale: f32,
    rhs_scale: f32,
    lane_count: u32,
) -> f32 {
    let mut acc = 0.0_f32;
    for lane in 0..lane_count as usize {
        acc += extract_i4(lhs_packed, lane) as f32 * extract_i4(rhs_packed, lane) as f32;
    }
    acc * lhs_scale * rhs_scale
}

fn dot_i32_oracle(lhs_packed: &[u32], rhs_packed: &[u32], lane_count: u32) -> i32 {
    let mut acc = 0_i32;
    for lane in 0..lane_count as usize {
        acc += extract_i4(lhs_packed, lane) * extract_i4(rhs_packed, lane);
    }
    acc
}

fn pack_i4_matrix_rows(rows: &[Vec<i32>]) -> Vec<u32> {
    let cols = rows.first().map_or(0, Vec::len);
    let words_per_row = cols.div_ceil(8);
    let mut out = Vec::with_capacity(rows.len() * words_per_row);
    for row in rows {
        let mut packed = pack_i4x8(row);
        packed.resize(words_per_row, 0);
        out.extend_from_slice(&packed);
    }
    out
}

fn matvec_scaled_oracle(
    weights_packed: &[u32],
    x: &[f32],
    scales: &[f32],
    rows: u32,
    cols: u32,
) -> Vec<f32> {
    let words_per_row = (cols as usize).div_ceil(8);
    let mut out = vec![0.0_f32; rows as usize];
    for row in 0..rows as usize {
        let mut acc = 0.0_f32;
        let row_words = &weights_packed[row * words_per_row..];
        for col in 0..cols as usize {
            acc += extract_i4(row_words, col) as f32 * x[col];
        }
        out[row] = acc * scales[row];
    }
    out
}

fn batched_matvec_scaled_oracle(
    weights_packed: &[u32],
    x_batches: &[f32],
    scales: &[f32],
    batch: u32,
    rows: u32,
    cols: u32,
) -> Vec<f32> {
    let mut out = Vec::with_capacity((batch * rows) as usize);
    for batch_index in 0..batch as usize {
        let x_start = batch_index * cols as usize;
        let x_end = x_start + cols as usize;
        out.extend(matvec_scaled_oracle(
            weights_packed,
            &x_batches[x_start..x_end],
            scales,
            rows,
            cols,
        ));
    }
    out
}

fn batched_packed_matmul_scaled_oracle(
    weights_packed: &[u32],
    activation_batches_packed: &[u32],
    row_scales: &[f32],
    batch_scales: &[f32],
    batch: u32,
    rows: u32,
    cols: u32,
) -> Vec<f32> {
    let words_per_row = (cols as usize).div_ceil(8);
    let mut out = vec![0.0_f32; (batch * rows) as usize];
    for batch_index in 0..batch as usize {
        let activation_words = &activation_batches_packed[batch_index * words_per_row..];
        for row in 0..rows as usize {
            let weight_words = &weights_packed[row * words_per_row..];
            let mut acc = 0.0_f32;
            for col in 0..cols as usize {
                acc +=
                    extract_i4(weight_words, col) as f32 * extract_i4(activation_words, col) as f32;
            }
            out[batch_index * rows as usize + row] =
                acc * row_scales[row] * batch_scales[batch_index];
        }
    }
    out
}

fn batched_packed_matmul_top1_scaled_oracle(
    weights_packed: &[u32],
    activation_batches_packed: &[u32],
    row_scales: &[f32],
    batch_scales: &[f32],
    batch: u32,
    rows: u32,
    cols: u32,
) -> (Vec<f32>, Vec<u32>) {
    let logits = batched_packed_matmul_scaled_oracle(
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

fn pack_u32(words: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(words.len() * 4);
    for word in words {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out
}

fn pack_f32(words: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(words.len() * 4);
    for word in words {
        out.extend_from_slice(&word.to_le_bytes());
    }
    out
}

fn read_f32(bytes: &[u8]) -> f32 {
    f32::from_le_bytes(
        bytes
            .get(0..4)
            .expect("Fix: CUDA INT4 scaled dot must emit one f32.")
            .try_into()
            .expect("Fix: f32 CUDA output must be exactly four bytes."),
    )
}

fn read_i32(bytes: &[u8]) -> i32 {
    i32::from_le_bytes(
        bytes
            .get(0..4)
            .expect("Fix: CUDA INT4 dot must emit one i32.")
            .try_into()
            .expect("Fix: i32 CUDA output must be exactly four bytes."),
    )
}

fn read_f32_vec(bytes: &[u8], count: usize) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .take(count)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().expect("Fix: f32 chunk is four bytes.")))
        .collect()
}

fn generated_i4_values(len: usize, seed: u32) -> Vec<i32> {
    let mut state = seed ^ 0x9E37_79B9;
    (0..len)
        .map(|index| {
            state = state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223)
                .rotate_left((index % 17) as u32);
            ((state >> 28) as i32) - 8
        })
        .collect()
}

fn generated_f32_values(len: usize, seed: u32) -> Vec<f32> {
    let mut state = seed ^ 0xA5A5_5A5A;
    (0..len)
        .map(|index| {
            state = state
                .wrapping_mul(747_796_405)
                .wrapping_add(2_891_336_453)
                .rotate_right((index % 11) as u32);
            (((state >> 27) & 0x1f) as f32 - 16.0) * 0.0625
        })
        .collect()
}

fn generated_positive_scales(len: usize, seed: u32) -> Vec<f32> {
    (0..len)
        .map(|index| 0.0625_f32 * (1 + ((seed as usize + index * 3) % 13)) as f32)
        .collect()
}

fn generated_i4_rows(rows: u32, cols: u32, seed: u32) -> Vec<Vec<i32>> {
    (0..rows)
        .map(|row| generated_i4_values(cols as usize, seed.wrapping_add(row * 97)))
        .collect()
}

fn f32_bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|value| value.to_bits()).collect()
}

#[path = "int4_quantized_gpu_parity/dot_contracts.rs"]
mod dot_contracts;
#[path = "int4_quantized_gpu_parity/matvec_contracts.rs"]
mod matvec_contracts;
#[path = "int4_quantized_gpu_parity/batched_matmul_contracts.rs"]
mod batched_matmul_contracts;
#[path = "int4_quantized_gpu_parity/generated_sweep_contracts.rs"]
mod generated_sweep_contracts;
