//! Tier 3 visual compute compositions.
//!
//! GPU-accelerated image processing ops for the Molten visual effects
//! engine. Each sub-module exposes one reusable composition built from
//! Tier 2.5 primitives (`math::conv1d`) and Tier 1 IR expressions
//! (bitwise pack/unpack, lerp, select).
//!
//! All compositions operate on RGBA u8 pixel buffers packed as `u32`
//! (one pixel per u32 word, little-endian RGBA: bits `[7:0]` = R,
//! `[15:8]` = G, `[23:16]` = B, `[31:24]` = A).
//!
//! # Discovery checklist (LEGO-BLOCK-RULE compliance)
//!
//! - `blur`  -  composes `math::conv1d` (horizontal + vertical weight tables)
//! - `shadow`  -  private SDF helper (single caller, stays inline)
//! - `filter_chain`  -  IR expressions only (mul, add, select)
//! - `composite`  -  IR expressions only (alpha arithmetic)
//! - `gradient`  -  IR expressions only (dot product + lerp)
//! - `downsample`  -  IR expressions only (box filter = average of 4)
//! - `glass`  -  composes blur + filter_chain (hero composition)

use vyre::ir::Expr;

/// Two-pass separable Gaussian blur (composes `math::conv1d`).
pub mod blur;
pub(crate) mod byte_helpers;
/// Porter-Duff alpha compositing.
pub mod composite;
/// 2× box-filter downsample for half-resolution blur.
pub mod downsample;
/// Composable per-pixel filter chain (brightness, contrast, saturate, invert).
pub mod filter_chain;
/// Complete glass material (blur + tint + border)  -  the hero composition.
pub mod glass;
/// CSS-compatible gradient rasterization (linear, radial, conic).
pub mod gradient;
/// GPU-computed box shadow with SDF falloff.
pub mod shadow;
/// 2× nearest-neighbor upsample for the half-resolution blur return path.
pub mod upsample;

// Re-exports for the public API surface.
pub use blur::{gaussian_blur_2pass, GaussianBlurStages};
pub use composite::alpha_over;
pub use downsample::downsample_2x;
pub use filter_chain::filter_chain;
pub use glass::{
    glass_blur_stage, glass_filter_stage, glass_stages, glass_stages_half_res, GlassParams,
};
pub use gradient::{linear_gradient, ColorStop};
pub use shadow::box_shadow;
pub use upsample::upsample_2x;

pub(crate) const PIXEL_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];

/// Return `(left * right) >> shift` without losing the high half of the
/// unsigned 32-bit product before the rescale.
pub(crate) fn wide_mul_shr_u32(left: Expr, right: Expr, shift: u32) -> Expr {
    debug_assert!((1..32).contains(&shift));
    let low = Expr::mul(left.clone(), right.clone());
    let high = Expr::mulhi(left, right);
    Expr::bitor(
        Expr::shr(low, Expr::u32(shift)),
        Expr::shl(high, Expr::u32(32 - shift)),
    )
}

/// Return `(left * right) >> 16` for unsigned 16.16 fixed-point pixel math.
pub(crate) fn fixed_mul_16_16_expr(left: Expr, right: Expr) -> Expr {
    wide_mul_shr_u32(left, right, 16)
}
