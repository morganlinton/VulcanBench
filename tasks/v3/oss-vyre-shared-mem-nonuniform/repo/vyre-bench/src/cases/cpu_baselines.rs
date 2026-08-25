//! CPU baseline kernels used by benchmark cases.

use rayon::prelude::*;
use std::sync::OnceLock;
use wide::f32x8;

const ELEMENTWISE_STRIPE_BYTES: usize = 64 * 1024;
const F32X8_BYTES: usize = 8 * 4;

pub fn baseline_pool() -> &'static rayon::ThreadPool {
    static POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();
    POOL.get_or_init(|| {
        let threads_str = std::env::var("RAYON_NUM_THREADS").unwrap_or_else(|_| "8".to_string());
        let threads = threads_str.parse().unwrap_or(8);
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .spawn_handler(|thread| {
                let mut b = std::thread::Builder::new();
                if let Some(name) = thread.name() {
                    b = b.name(name.to_owned());
                }
                if let Some(stack_size) = thread.stack_size() {
                    b = b.stack_size(stack_size);
                }
                b.spawn(move || {
                    let core_ids = core_affinity::get_core_ids().unwrap_or_default();
                    if !core_ids.is_empty() {
                        let id = core_ids[thread.index() % core_ids.len()];
                        let _ = core_affinity::set_for_current(id);
                    }
                    thread.run()
                })
                .map(|_| ())
            })
            .build()
            .unwrap_or_else(|_| {
                rayon::ThreadPoolBuilder::new()
                    .num_threads(1)
                    .build()
                    .unwrap_or_else(|_| std::process::abort())
            })
    })
}

pub fn elementwise_add_f32_bytes(a: &[u8], b: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; a.len()];
    elementwise_add_f32_bytes_into(a, b, &mut out);
    out
}

pub fn elementwise_add_f32_bytes_into(a: &[u8], b: &[u8], out: &mut [u8]) {
    assert_eq!(
        a.len(),
        b.len(),
        "elementwise inputs must have equal byte length"
    );
    assert_eq!(
        a.len(),
        out.len(),
        "elementwise output must match input byte length"
    );
    assert_eq!(
        a.len() % 4,
        0,
        "elementwise input byte length must be a multiple of sizeof(f32)"
    );
    baseline_pool().install(|| {
        out.par_chunks_mut(ELEMENTWISE_STRIPE_BYTES)
            .enumerate()
            .for_each(|(stripe, dst)| {
                let start = stripe * ELEMENTWISE_STRIPE_BYTES;
                let a = &a[start..start + dst.len()];
                let b = &b[start..start + dst.len()];
                let mut offset = 0;
                while offset + F32X8_BYTES <= dst.len() {
                    write_f32x8(
                        &mut dst[offset..offset + F32X8_BYTES],
                        read_f32x8(&a[offset..offset + F32X8_BYTES])
                            + read_f32x8(&b[offset..offset + F32X8_BYTES]),
                    );
                    offset += F32X8_BYTES;
                }
                while offset < dst.len() {
                    let word_index = offset / 4;
                    let value =
                        read_f32_word_or_zero(a, word_index) + read_f32_word_or_zero(b, word_index);
                    dst[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                    offset += 4;
                }
            });
    });
}

#[inline]
fn read_f32x8(bytes: &[u8]) -> f32x8 {
    debug_assert_eq!(bytes.len(), F32X8_BYTES);
    f32x8::new([
        read_f32_word_or_zero(bytes, 0),
        read_f32_word_or_zero(bytes, 1),
        read_f32_word_or_zero(bytes, 2),
        read_f32_word_or_zero(bytes, 3),
        read_f32_word_or_zero(bytes, 4),
        read_f32_word_or_zero(bytes, 5),
        read_f32_word_or_zero(bytes, 6),
        read_f32_word_or_zero(bytes, 7),
    ])
}

#[inline]
fn read_u32_word_or_zero(bytes: &[u8], word_index: usize) -> u32 {
    vyre_primitives::wire::read_u32_le_word(bytes, word_index, "cpu baseline input").unwrap_or(0)
}

#[inline]
fn read_f32_word_or_zero(bytes: &[u8], word_index: usize) -> f32 {
    f32::from_bits(read_u32_word_or_zero(bytes, word_index))
}

#[inline]
fn write_f32x8(dst: &mut [u8], value: f32x8) {
    debug_assert_eq!(dst.len(), F32X8_BYTES);
    for (lane, value) in value.to_array().into_iter().enumerate() {
        let offset = lane * 4;
        dst[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}

pub fn reduce_sum_u32_bytes(values: &[u8]) -> Vec<u8> {
    baseline_pool().install(|| {
        let sum = values
            .par_chunks_exact(4)
            .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .reduce(|| 0, u32::wrapping_add);
        sum.to_le_bytes().to_vec()
    })
}

pub fn matmul_f32_bytes(a: &[u8], b: &[u8], m: usize, n: usize, k: usize) -> Vec<u8> {
    baseline_pool().install(|| {
        let lhs = faer::Mat::<f32>::from_fn(m, k, |row, col| {
            read_f32_word_or_zero(a, row * k + col)
        });
        let rhs = faer::Mat::<f32>::from_fn(k, n, |row, col| {
            read_f32_word_or_zero(b, row * n + col)
        });
        let mut dst = faer::Mat::<f32>::zeros(m, n);
        faer::linalg::matmul::matmul(
            dst.as_mut(),
            faer::Accum::Replace,
            lhs.as_ref(),
            rhs.as_ref(),
            1.0f32,
            faer::Par::rayon(0),
        );
        let mut out = vec![0u8; m * n * 4];
        for row in 0..m {
            for col in 0..n {
                let offset = (row * n + col) * 4;
                out[offset..offset + 4].copy_from_slice(&dst[(row, col)].to_le_bytes());
            }
        }
        out
    })
}

pub fn attention_proxy_f32_bytes(q: &[u8], k: &[u8], v: &[u8], seq: usize, dim: usize) -> Vec<u8> {
    baseline_pool().install(|| {
        let mut out = vec![0u8; seq * dim * 4];
        out.par_chunks_exact_mut(dim * 4)
            .enumerate()
            .for_each(|(row, row_bytes)| {
                for col in 0..dim {
                    let q = read_f32_word_or_zero(q, row * dim + col);
                    let mut acc = 0.0f32;
                    for kk in 0..seq {
                        let word_index = kk * dim + col;
                        let k_val = read_f32_word_or_zero(k, word_index);
                        let v_val = read_f32_word_or_zero(v, word_index);
                        acc += q * k_val * v_val;
                    }
                    let out_idx = col * 4;
                    row_bytes[out_idx..out_idx + 4].copy_from_slice(&acc.to_le_bytes());
                }
            });
        out
    })
}

pub fn dfa_vyre_match_count_bytes(text: &[u8]) -> Vec<u8> {
    let matches = memchr::memmem::find_iter(text, b"vyre").count();
    let matches = match u32::try_from(matches) {
        Ok(value) => value,
        Err(_) => u32::MAX,
    };
    matches.to_le_bytes().to_vec()
}

pub fn transpose_f32_bytes(input: &[u8], rows: usize, cols: usize) -> Vec<u8> {
    assert_eq!(
        input.len(),
        rows.saturating_mul(cols).saturating_mul(4),
        "transpose input length must match rows * cols * sizeof(f32)"
    );
    baseline_pool().install(|| {
        let mut out = vec![0u8; input.len()];
        out.par_chunks_exact_mut(rows * 4)
            .enumerate()
            .for_each(|(col, dst_col)| {
                for row in 0..rows {
                    let src = (row * cols + col) * 4;
                    let dst = row * 4;
                    dst_col[dst..dst + 4].copy_from_slice(&input[src..src + 4]);
                }
            });
        out
    })
}

pub fn histogram_u32_256_bytes(values: &[u8]) -> Vec<u8> {
    baseline_pool().install(|| {
        let word_count = values.len() / 4;
        let worker_count = rayon::current_num_threads().max(1);
        let words_per_chunk = word_count.div_ceil(worker_count).max(1);
        let chunk_bytes = words_per_chunk.saturating_mul(4);
        let bins = values
            .par_chunks(chunk_bytes)
            .map(|chunk_bytes| {
                let mut local = Box::new([0u32; 256]);
                for chunk in chunk_bytes.chunks_exact(4) {
                    let value = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as usize;
                    local[value & 255] = local[value & 255].wrapping_add(1);
                }
                local
            })
            .reduce(
                || Box::new([0u32; 256]),
                |mut left, right| {
                    for bin in 0..256 {
                        left[bin] = left[bin].wrapping_add(right[bin]);
                    }
                    left
                },
            );
        let mut out = vec![0u8; 256 * 4];
        for (bin, count) in bins.iter().enumerate() {
            out[bin * 4..bin * 4 + 4].copy_from_slice(&count.to_le_bytes());
        }
        out
    })
}

pub fn gather_u32_bytes(values: &[u8], indices: &[u8]) -> Vec<u8> {
    baseline_pool().install(|| {
        let count = indices.len() / 4;
        let value_count = values.len() / 4;
        let mut out = vec![0u8; count * 4];
        out.par_chunks_exact_mut(4)
            .enumerate()
            .for_each(|(lane, dst)| {
                let index = read_u32_word_or_zero(indices, lane) as usize % value_count;
                let src = index * 4;
                dst.copy_from_slice(&values[src..src + 4]);
            });
        out
    })
}

pub fn stencil3_u32_bytes(values: &[u8]) -> Vec<u8> {
    baseline_pool().install(|| {
        let count = values.len() / 4;
        let mut out = vec![0u8; values.len()];
        out.par_chunks_exact_mut(4)
            .enumerate()
            .for_each(|(index, dst)| {
                if index == 0 || index + 1 >= count {
                    return;
                }
                let left = (index - 1) * 4;
                let mid = index * 4;
                let right = (index + 1) * 4;
                let value = read_u32_word_or_zero(values, left / 4)
                    .wrapping_add(read_u32_word_or_zero(values, mid / 4))
                    .wrapping_add(read_u32_word_or_zero(values, right / 4));
                dst.copy_from_slice(&value.to_le_bytes());
            });
        out
    })
}
