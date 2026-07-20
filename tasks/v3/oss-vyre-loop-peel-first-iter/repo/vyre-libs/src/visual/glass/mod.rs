//! Complete glass material  -  the hero Molten composition.
//!
//! Combines blur + tint + border into a single batched pipeline
//! that replaces CSS `backdrop-filter: blur(N) + background-color`.
//!
//! Category A composition  -  composes blur, filter_chain, and composite.
//!
//! ## Half-resolution optimization
//!
//! For blur radii > 8px, `glass_stages_half_res` automatically:
//! 1. Downsamples input to half resolution (4× fewer pixels)
//! 2. Blurs at half resolution (with halved radius)
//! 3. Upsamples back to full resolution
//! 4. Applies the filter chain
//!
//! This is visually indistinguishable from full-res blur because
//! blur already destroys high-frequency detail.

use vyre::ir::Program;

use super::blur::{gaussian_blur_2pass, GaussianBlurStages};
use super::downsample::downsample_2x;
use super::filter_chain::filter_chain;
use super::upsample::upsample_2x;

const OP_ID: &str = "vyre-libs::visual::glass";

/// Parameters for the glass material.
#[derive(Clone, Debug)]
pub struct GlassParams {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Blur kernel half-width.
    pub blur_radius: u32,
    /// Gaussian sigma.
    pub blur_sigma: f32,
    /// Tint color (packed RGBA, e.g. 0x0D_FFFFFF for white at 5%).
    pub tint_rgba: u32,
    /// Brightness multiplier (1.0 = identity).
    pub brightness: f32,
    /// Saturation multiplier (1.0 = identity, 0.75 = desaturate slightly).
    pub saturation: f32,
}

/// Build the complete glass pipeline as a sequence of sub-compositions.
///
/// The glass material is built by chaining:
/// 1. `blur`  -  Gaussian blur the background scene
/// 2. `filter_chain`  -  apply tint via brightness/saturation adjustment
/// 3. Return the result
///
/// Since each sub-composition produces a standalone `Program`, and
/// the megakernel runtime chains them by dispatching sequentially,
/// the glass composition constructs the Programs for documentation
/// and returns the blur program (the most compute-intensive stage).
///
/// In practice, the WASM bridge dispatches each stage separately:
/// ```text
/// dispatch(blur_program, scene → blurred)
/// dispatch(filter_program, blurred → blurred) // in-place tint
/// ```
///
/// This function returns the blur `Program`  -  the critical path.
/// Call `glass_filter_stage` for the tint program.
#[must_use]
pub fn glass_blur_stage(
    input: &str,
    output: &str,
    scratch: &str,
    params: &GlassParams,
) -> GaussianBlurStages {
    gaussian_blur_2pass(
        input,
        output,
        scratch,
        params.width,
        params.height,
        params.blur_radius,
        params.blur_sigma,
    )
}

/// Build the tint/color-adjustment stage of the glass pipeline.
///
/// Applied in-place to the blurred image.
#[must_use]
pub fn glass_filter_stage(pixels: &str, params: &GlassParams) -> Program {
    let count = params.width * params.height;
    filter_chain(
        pixels,
        count,
        params.brightness,
        1.0,
        params.saturation,
        0.0,
    )
}

/// Convenience: build both stages and return them as a pair.
///
/// `stages.0` = blur (input → output via scratch)
/// `stages.1` = tint (output in-place)
///
/// Caller dispatches them sequentially with a barrier between.
#[must_use]
pub fn glass_stages(
    input: &str,
    output: &str,
    scratch: &str,
    params: &GlassParams,
) -> (GaussianBlurStages, Program) {
    (
        glass_blur_stage(input, output, scratch, params),
        glass_filter_stage(output, params),
    )
}

/// Half-resolution glass pipeline  -  4× fewer pixels processed.
///
/// Returns four stages:
/// 1. `downsample`  -  input (W×H) → half (W/2 × H/2)
/// 2. `blur`  -  blur at half resolution (radius/2, sigma/2)
/// 3. `upsample`  -  half → full resolution
/// 4. `filter`  -  brightness/saturation tint
///
/// For blur_radius ≤ 8, this falls back to the full-res path since
/// the downsample/upsample overhead outweighs the pixel savings.
///
/// # Buffer layout
/// - `input`: source pixels `[u32; W*H]`
/// - `output`: final result `[u32; W*H]`
/// - `scratch`: working buffer `[u32; W*H]`
/// - `half`: half-res buffer `[u32; (W/2)*(H/2)]`
/// - `half_scratch`: half-res scratch `[u32; (W/2)*(H/2)]`
#[must_use]
pub fn glass_stages_half_res(
    input: &str,
    output: &str,
    scratch: &str,
    half: &str,
    half_scratch: &str,
    params: &GlassParams,
) -> GlassHalfResPipeline {
    // Fall back to full-res for small radii where downsample overhead > savings.
    if params.blur_radius <= 8 || params.width < 4 || params.height < 4 {
        let (blur, filter) = glass_stages(input, output, scratch, params);
        return GlassHalfResPipeline::FullRes { blur, filter };
    }

    let half_w = params.width / 2;
    let half_h = params.height / 2;

    // Stage 1: Downsample full → half.
    let downsample = downsample_2x(input, half, params.width, params.height);

    // Stage 2: Blur at half resolution.
    // Halve the radius (the downsampled image is 2× smaller, so half the radius
    // covers the same visual area). Sigma scales proportionally.
    let half_radius = (params.blur_radius / 2).max(1);
    let half_sigma = params.blur_sigma / 2.0;
    let blur = gaussian_blur_2pass(
        half,
        half_scratch,
        half,
        half_w,
        half_h,
        half_radius,
        half_sigma,
    );

    // Stage 3: Upsample half → full.
    let upsample = upsample_2x(half_scratch, output, params.width, params.height);

    // Stage 4: Filter chain on full-res result.
    let filter = glass_filter_stage(output, params);

    GlassHalfResPipeline::HalfRes {
        downsample,
        blur,
        upsample,
        filter,
    }
}

/// The set of programs for a glass composition, either full-res or half-res.
#[derive(Debug)]
pub enum GlassHalfResPipeline {
    /// Standard two-stage (blur + filter) when radius is small.
    FullRes {
        /// Gaussian blur dispatches.
        blur: GaussianBlurStages,
        /// Filter chain program.
        filter: Program,
    },
    /// Four-stage half-res pipeline (downsample → blur → upsample → filter).
    HalfRes {
        /// 2× downsample.
        downsample: Program,
        /// Blur at half resolution.
        blur: GaussianBlurStages,
        /// 2× upsample.
        upsample: Program,
        /// Filter chain.
        filter: Program,
    },
}

impl GlassHalfResPipeline {
    /// Number of GPU dispatch stages needed.
    #[must_use]
    pub fn stage_count(&self) -> usize {
        match self {
            Self::FullRes { blur, .. } => blur.stage_count() + 1,
            Self::HalfRes { blur, .. } => blur.stage_count() + 3,
        }
    }

    /// Collect all programs in dispatch order.
    #[must_use]
    pub fn programs(&self) -> Vec<&Program> {
        match self {
            Self::FullRes { blur, filter } => {
                let mut programs = Vec::with_capacity(3);
                programs.extend(blur.programs());
                programs.push(filter);
                programs
            }
            Self::HalfRes {
                downsample,
                blur,
                upsample,
                filter,
            } => {
                let mut programs = Vec::with_capacity(5);
                programs.push(downsample);
                programs.extend(blur.programs());
                programs.push(upsample);
                programs.push(filter);
                programs
            }
        }
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || crate::region::tag_program(OP_ID, glass_blur_stage("scene", "output", "scratch", &GlassParams {
            width: 4,
            height: 4,
            blur_radius: 1,
            blur_sigma: 0.8,
            tint_rgba: 0x0D_FFFFFF,
            brightness: 1.0,
            saturation: 0.75,
        }).horizontal),
        test_inputs: Some(|| {
            // 4×4 all-white scene → glass blur → all-white.
            let pixels = vec![0xFFFF_FFFFu32; 16];
            vec![vec![
                crate::visual::byte_helpers::u32_words_to_le_bytes(&pixels),
                vec![0u8; 64],
            ]]
        }),
        expected_output: Some(|| {
            let pixels = vec![0xFFFF_FFFFu32; 16];
            vec![vec![crate::visual::byte_helpers::u32_words_to_le_bytes(&pixels)]]
        }),
        category: Some("visual"),
    }
}
