//! 1D separable convolution primitive.
//!
//! Applies a 1D kernel of precomputed weights along a single axis of
//! a buffer. Domain-neutral: reused by image blur (horizontal/vertical
//! passes), signal processing, audio filtering, and NLP.
//!
//! # Wire format
//!
//! - `input`:   `[u32; count]`  -  source data
//! - `output`:  `[u32; count]`  -  convolved result
//! - `weights`: `[u32; diameter]`  -  kernel weights (fixed-point 16.16)
//! - `params`:  `[u32; 4]`  -  `[count, stride, radius, _reserved]`
//!
//! `stride` controls axis selection: for a 2D buffer of width W,
//! `stride=1` convolves along rows (horizontal) and `stride=W`
//! convolves along columns (vertical).

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Stable Tier 2.5 op id.
pub const OP_ID: &str = "vyre-primitives::math::conv1d";

/// Maximum supported kernel half-width.
pub const MAX_RADIUS: u32 = 64;

/// `min(a, b)` expressed via `select(lt(a, b), a, b)`.
fn expr_min(a: Expr, b: Expr) -> Expr {
    Expr::select(Expr::lt(a.clone(), b.clone()), a, b)
}

/// Emit the 1D convolution loop for a single output element.
///
/// Each invocation at index `gid.x` reads `2*radius+1` input elements
/// centered at `gid.x` (with clamped boundary), multiplies by the
/// corresponding weight, and writes the weighted sum to `output[gid.x]`.
///
/// The `stride` parameter selects the axis: stride=1 for contiguous
/// (row-major horizontal), stride=width for column-major vertical.
/// Boundary handling: clamp indices to `[0, count-1]`.
#[must_use]
pub fn conv1d_node(input: &str, output: &str, weights: &str, params: &str) -> Node {
    Node::Region {
        generator: Ident::from(OP_ID),
        source_region: None,
        body: Arc::new(vec![
            // Load params.
            Node::let_bind("count", Expr::load(params, Expr::u32(0))),
            Node::let_bind("stride", Expr::load(params, Expr::u32(1))),
            Node::let_bind("radius", Expr::load(params, Expr::u32(2))),
            // Output index from global invocation id.
            Node::let_bind("idx", Expr::gid_x()),
            // Bounds check.
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::var("count")),
                vec![
                    // Kernel diameter = 2 * radius + 1.
                    Node::let_bind(
                        "diameter",
                        Expr::add(Expr::mul(Expr::var("radius"), Expr::u32(2)), Expr::u32(1)),
                    ),
                    // Accumulator.
                    Node::let_bind("acc", Expr::u32(0)),
                    // Convolution loop: k in 0..diameter.
                    Node::loop_for(
                        "k",
                        Expr::u32(0),
                        Expr::var("diameter"),
                        vec![
                            // Offset from center: k - radius (can be negative,
                            // but we work in u32 and clamp the final index).
                            // src_raw = idx + (k - radius) * stride
                            // We compute carefully to avoid u32 underflow:
                            //   if k >= radius:
                            //     src_raw = idx + (k - radius) * stride
                            //   else:
                            //     src_raw = idx - (radius - k) * stride
                            //   then clamp to [0, count-1]
                            Node::let_bind(
                                "src_idx",
                                Expr::select(
                                    Expr::ge(Expr::var("k"), Expr::var("radius")),
                                    // k >= radius: add offset
                                    expr_min(
                                        Expr::add(
                                            Expr::var("idx"),
                                            Expr::mul(
                                                Expr::sub(Expr::var("k"), Expr::var("radius")),
                                                Expr::var("stride"),
                                            ),
                                        ),
                                        Expr::sub(Expr::var("count"), Expr::u32(1)),
                                    ),
                                    // k < radius: subtract offset, floor at 0
                                    Expr::select(
                                        Expr::ge(
                                            Expr::var("idx"),
                                            Expr::mul(
                                                Expr::sub(Expr::var("radius"), Expr::var("k")),
                                                Expr::var("stride"),
                                            ),
                                        ),
                                        Expr::sub(
                                            Expr::var("idx"),
                                            Expr::mul(
                                                Expr::sub(Expr::var("radius"), Expr::var("k")),
                                                Expr::var("stride"),
                                            ),
                                        ),
                                        Expr::u32(0),
                                    ),
                                ),
                            ),
                            // Load source value and kernel weight.
                            Node::let_bind("val", Expr::load(input, Expr::var("src_idx"))),
                            Node::let_bind("w", Expr::load(weights, Expr::var("k"))),
                            // Accumulate: acc += val * w.
                            Node::assign(
                                "acc",
                                Expr::add(
                                    Expr::var("acc"),
                                    Expr::mul(Expr::var("val"), Expr::var("w")),
                                ),
                            ),
                        ],
                    ),
                    // Write result (still in fixed-point  -  caller normalizes).
                    Node::store(output, Expr::var("idx"), Expr::var("acc")),
                ],
            ),
        ]),
    }
}

/// Standalone 1D convolution Program.
///
/// Dispatches one invocation per element. The caller is responsible
/// for precomputing kernel weights and choosing the correct stride.
#[must_use]
pub fn conv1d_program(count: u32, radius: u32) -> Program {
    let clamped_radius = radius.min(MAX_RADIUS);
    let diameter = 2 * clamped_radius + 1;
    Program::wrapped(
        vec![
            BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(count),
            BufferDecl::storage("output", 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(count),
            BufferDecl::storage("weights", 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(diameter),
            BufferDecl::storage("params", 3, BufferAccess::ReadOnly, DataType::U32).with_count(4),
        ],
        [256, 1, 1],
        vec![conv1d_node("input", "output", "weights", "params")],
    )
}

/// Precompute Gaussian kernel weights as fixed-point 16.16 u32 values.
///
/// Returns a Vec suitable for uploading to the `weights` buffer.
/// The kernel is normalized: sum of weights ≈ 1.0 (65536 in fixed-point).
#[must_use]
pub fn gaussian_weights(radius: u32, sigma: f32) -> Vec<u32> {
    let clamped = radius.min(MAX_RADIUS);
    let diameter = (2 * clamped + 1) as usize;
    let mut weights = vec![0.0f64; diameter];
    let s2 = 2.0 * (sigma as f64) * (sigma as f64);
    let mut sum = 0.0;

    for (i, w) in weights.iter_mut().enumerate() {
        let x = i as f64 - clamped as f64;
        *w = (-x * x / s2).exp();
        sum += *w;
    }

    weights
        .iter()
        .map(|w| ((w / sum) * 65536.0).round() as u32)
        .collect()
}

/// Pack conv1d params: `[count, stride, radius, 0]`.
#[must_use]
pub fn pack_params(count: u32, stride: u32, radius: u32) -> Vec<u32> {
    vec![count, stride, radius.min(MAX_RADIUS), 0]
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || conv1d_program(8, 1),
        Some(|| {
            // 8-element signal, identity-like kernel (center-heavy).
            let input: Vec<u32> = vec![100, 200, 300, 400, 500, 600, 700, 800];
            let params = pack_params(8, 1, 1);
            // Simple averaging kernel: [0.25, 0.5, 0.25] in fixed-point 16.16.
            let weights: Vec<u32> = vec![16384, 32768, 16384];
            let to_bytes = |v: &[u32]| crate::wire::pack_u32_slice(v);
            vec![vec![
                to_bytes(&input),
                vec![0u8; 32],       // output (zeroed)
                to_bytes(&weights),
                to_bytes(&params),
            ]]
        }),
        Some(|| {
            // Expected fixed-point accumulators before caller-side normalization.
            let to_bytes = |v: &[u32]| crate::wire::pack_u32_slice(v);
            vec![vec![to_bytes(&[
                8_192_000, 13_107_200, 19_660_800, 26_214_400, 32_768_000, 39_321_600,
                45_875_200, 50_790_400,
            ])]]
        }),
    )
}

// ---------------------------------------------------------------------------
// CPU reference implementation
// ---------------------------------------------------------------------------

/// CPU reference: 1D convolution with clamped boundary, matching the GPU
/// kernel's fixed-point accumulation. Weights are in 16.16 fixed-point.
///
/// Returns one output u32 per input element (pre-normalization accumulator).
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_conv1d(input: &[u32], weights: &[u32], stride: u32) -> Vec<u32> {
    let mut output = Vec::new();
    cpu_conv1d_into(input, weights, stride, &mut output);
    output
}

/// CPU reference writing into caller-owned output storage.
///
/// Reuses `output` across repeated convolution parity checks and preserves the
/// same fixed-point, clamped-boundary semantics as [`cpu_conv1d`].
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_conv1d_into(input: &[u32], weights: &[u32], stride: u32, output: &mut Vec<u32>) {
    output.clear();
    let count = input.len();
    if count == 0 {
        return;
    }
    let diameter = weights.len();
    let radius = diameter / 2;
    output.reserve(count);

    for idx in 0..count {
        let mut acc: u32 = 0;
        for k in 0..diameter {
            let src_idx = if k >= radius {
                let offset = (k - radius) as u32 * stride;
                let raw = idx as u32 + offset;
                raw.min(count as u32 - 1) as usize
            } else {
                let offset = (radius - k) as u32 * stride;
                if idx as u32 >= offset {
                    (idx as u32 - offset) as usize
                } else {
                    0
                }
            };
            acc = acc.wrapping_add(input[src_idx].wrapping_mul(weights[k]));
        }
        output.push(acc);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_conv1d_identity_kernel() {
        // Identity kernel: [0, 1.0, 0] in fixed-point = [0, 65536, 0]
        let input = vec![10, 20, 30, 40, 50];
        let weights = vec![0, 65536, 0];
        let result = cpu_conv1d(&input, &weights, 1);
        // Each output should be input[i] * 65536
        let expected: Vec<u32> = input.iter().map(|&v| v * 65536).collect();
        assert_eq!(result, expected);
    }

    #[test]
    fn cpu_conv1d_averaging_kernel_matches_inventory() {
        // Must match the inventory expected output.
        let input = vec![100u32, 200, 300, 400, 500, 600, 700, 800];
        let weights = vec![16384u32, 32768, 16384]; // [0.25, 0.5, 0.25]
        let result = cpu_conv1d(&input, &weights, 1);
        let expected = vec![
            8_192_000, 13_107_200, 19_660_800, 26_214_400, 32_768_000, 39_321_600, 45_875_200,
            50_790_400,
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn cpu_conv1d_empty() {
        let result = cpu_conv1d(&[], &[65536], 1);
        assert!(result.is_empty());
    }

    #[test]
    fn cpu_conv1d_single_element() {
        let result = cpu_conv1d(&[42], &[16384, 32768, 16384], 1);
        // Clamped boundaries: all lookups hit index 0 (value 42).
        // acc = 42*16384 + 42*32768 + 42*16384 = 42*65536 = 2752512
        assert_eq!(result, vec![42 * 65536]);
    }

    #[test]
    fn cpu_conv1d_into_reuses_output_and_removes_stale_tail() {
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        let capacity = out.capacity();

        cpu_conv1d_into(&[10, 20, 30], &[0, 65536, 0], 1, &mut out);
        assert_eq!(out, vec![655_360, 1_310_720, 1_966_080]);
        assert_eq!(out.capacity(), capacity);

        cpu_conv1d_into(&[7], &[65536], 1, &mut out);
        assert_eq!(out, vec![458_752]);
        assert_eq!(out.capacity(), capacity);
    }
}
