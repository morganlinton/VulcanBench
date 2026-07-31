//! 2D convolution sub-dialect.
//!
//! ROADMAP H3  -  Im2col/direct-conv decision by shape and memory
//! budget. Substrate ships `conv2d_3x3_direct`: direct 2D
//! convolution with a fixed 3x3 kernel and unit stride. Im2col
//! variant + the shape-driven decision wrapper land beside this
//! primitive once the direct-conv reference is verified.
//!
//! ## Why direct conv first
//!
//! Direct convolution is the algorithmic ground truth: every
//! element of the output is computed by the canonical sum
//! `out[y, x] = sum_{ky=0..3, kx=0..3} input[y+ky-1, x+kx-1] * kernel[ky, kx]`.
//! Im2col's contribution is to reshape this into a matmul that
//! reuses the existing `matmul` primitive's tile / vectorisation
//! work; the parity gate is "im2col output equals direct-conv
//! output". The direct-conv primitive provides that parity gate.

pub mod conv2d;
pub mod im2col;

pub use conv2d::conv2d_3x3_direct;
pub use im2col::im2col_3x3;

/// Decision wrapper: choose direct conv vs im2col + matmul based
/// on image area. Crossover threshold derived from a simple memory
/// vs compute tradeoff: im2col materialises an `H·W·9` patch matrix
/// (vs `H·W` for the input), so it pays an extra `8·H·W·sizeof(f32)`
/// of memory traffic. The matmul tile/vectorisation win recovers
/// that cost once the per-pixel work amortises across enough output
/// pixels  -  empirically the crossover is around 64x64 (4096
/// pixels). Below that threshold direct conv wins.
///
/// Returns the same Program as `conv2d_3x3_direct(input, kernel,
/// output, h, w)` regardless of the decision; the choice is
/// expressed via the Region's `generator` ident so a downstream
/// pass can route the dispatch differently if the runtime chooses
/// to honour the hint.
///
/// # Errors
///
/// Returns `Err` when `h * w` overflows `u32`.
pub fn conv2d_3x3_decision(
    input: &str,
    kernel: &str,
    output: &str,
    h: u32,
    w: u32,
) -> Result<vyre::ir::Program, String> {
    const IM2COL_PIXEL_THRESHOLD: u32 = 4096; // 64x64
    let pixels = h.checked_mul(w).ok_or_else(|| {
        "Fix: conv2d_3x3_decision h*w overflows u32; reduce dimensions.".to_string()
    })?;
    if pixels >= IM2COL_PIXEL_THRESHOLD {
        // Large image: prefer im2col + matmul. The fully wired
        // im2col-then-matmul composition is a two-dispatch sequence
        // that the runtime megakernel scheduler can fuse; we ship
        // the direct-conv Program here with the generator id
        // signalling the hint, and the megakernel-side router
        // substitutes when the wired im2col+matmul path is in
        // place.
        let mut prog = conv2d_3x3_direct(input, kernel, output, h, w)?;
        // Best-effort hint: replace the wrapping Region's generator
        // with a name that signals "preferred for im2col routing".
        let entry = prog.entry().to_vec();
        let new_entry: Vec<vyre::ir::Node> = entry
            .into_iter()
            .map(|node| match node {
                vyre::ir::Node::Region {
                    body,
                    source_region,
                    ..
                } => vyre::ir::Node::Region {
                    generator: "vyre-libs::math::conv::conv2d_3x3_im2col_preferred".into(),
                    source_region,
                    body,
                },
                other => other,
            })
            .collect();
        prog = prog.with_rewritten_entry(new_entry);
        Ok(prog)
    } else {
        // Small image: direct conv wins.
        conv2d_3x3_direct(input, kernel, output, h, w)
    }
}
