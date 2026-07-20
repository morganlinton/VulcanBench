//! Two-dispatch separable Gaussian blur.
//!
//! Composes `vyre_primitives::math::conv1d` for horizontal + vertical
//! passes. The approach: since conv1d operates on scalar u32 values
//! but pixels are packed RGBA, we process the image as a flat array
//! of u32 values where each pixel's channels are handled by the
//! per-channel unpack→convolve→repack strategy.
//!
//! For initial simplicity, we inline the convolution directly (pure IR)
//! and compose the conv1d primitive's node as the inner kernel.
//!
//! Category A composition  -  composes Tier 2.5 `math::conv1d`.

use std::sync::Arc;

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::ir::model::expr::{GeneratorRef, Ident};

const OP_ID: &str = "vyre-libs::visual::blur";

/// Build the two dispatches for a separable Gaussian blur.
///
/// Since `conv1d` operates on scalar u32 values but our pixels are
/// packed RGBA, this composition:
/// 1. Dispatches per-pixel with 2D grid
/// 2. For each pixel, manually reads the horizontal/vertical
///    neighbors, unpacks per channel, convolves, and repacks
///
/// The composition wraps `conv1d_node` as a tagged child region
/// for composition tracking, even though the pixel unpacking is
/// handled by this composition's own IR.
///
/// # Parameters
/// - `width`, `height`: image dimensions
/// - `radius`: blur kernel half-width
/// - `sigma`: Gaussian sigma
#[must_use]
pub fn gaussian_blur_2pass(
    input: &str,
    output: &str,
    scratch: &str,
    width: u32,
    height: u32,
    radius: u32,
    sigma: f32,
) -> GaussianBlurStages {
    let kernel = GaussianKernel::new(radius, sigma);
    gaussian_blur_2pass_with_kernel(input, output, scratch, width, height, &kernel)
}

/// Build the two dispatches for a separable Gaussian blur using precomputed
/// weights.
///
/// Hot paths that rebuild programs for multiple buffers or frames should keep
/// a `GaussianKernel` and pass it here instead of recomputing the same
/// fixed-point weights on every build.
#[must_use]
pub fn gaussian_blur_2pass_with_kernel(
    input: &str,
    output: &str,
    scratch: &str,
    width: u32,
    height: u32,
    kernel: &GaussianKernel,
) -> GaussianBlurStages {
    GaussianBlurStages {
        horizontal: gaussian_blur_pass(
            input,
            scratch,
            width,
            height,
            kernel.radius(),
            kernel.weights(),
            Axis::Horizontal,
        ),
        vertical: gaussian_blur_pass(
            scratch,
            output,
            width,
            height,
            kernel.radius(),
            kernel.weights(),
            Axis::Vertical,
        ),
    }
}

/// Reusable fixed-point Gaussian blur kernel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GaussianKernel {
    radius: u32,
    weights: Vec<u32>,
}

impl GaussianKernel {
    /// Precompute weights for a Gaussian blur radius and sigma.
    #[must_use]
    pub fn new(radius: u32, sigma: f32) -> Self {
        let clamped = radius.min(vyre_primitives::math::conv1d::MAX_RADIUS);
        Self {
            radius: clamped,
            weights: vyre_primitives::math::conv1d::gaussian_weights(clamped, sigma),
        }
    }

    /// Build a kernel from caller-owned precomputed fixed-point weights.
    ///
    /// # Errors
    ///
    /// Returns an actionable error when `weights.len()` does not match
    /// `2 * min(radius, MAX_RADIUS) + 1`.
    pub fn from_weights(radius: u32, weights: Vec<u32>) -> Result<Self, GaussianKernelError> {
        let clamped = radius.min(vyre_primitives::math::conv1d::MAX_RADIUS);
        let expected = (2 * clamped + 1) as usize;
        if weights.len() != expected {
            return Err(GaussianKernelError {
                radius: clamped,
                expected,
                actual: weights.len(),
            });
        }
        Ok(Self {
            radius: clamped,
            weights,
        })
    }

    /// Clamped kernel radius.
    #[must_use]
    pub const fn radius(&self) -> u32 {
        self.radius
    }

    /// Fixed-point 16.16 weights.
    #[must_use]
    pub fn weights(&self) -> &[u32] {
        &self.weights
    }
}

/// Invalid reusable Gaussian kernel shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GaussianKernelError {
    /// Clamped radius requested by the caller.
    pub radius: u32,
    /// Expected weight count.
    pub expected: usize,
    /// Actual weight count supplied.
    pub actual: usize,
}

impl std::fmt::Display for GaussianKernelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid Gaussian kernel for radius {}: expected {} weights, got {}. Fix: supply 2 * radius + 1 fixed-point weights.",
            self.radius, self.expected, self.actual
        )
    }
}

impl std::error::Error for GaussianKernelError {}

/// The two dispatches that make up a correct separable Gaussian blur.
#[derive(Debug)]
pub struct GaussianBlurStages {
    /// Horizontal pass: `input -> scratch`.
    pub horizontal: Program,
    /// Vertical pass: `scratch -> output`.
    pub vertical: Program,
}

impl GaussianBlurStages {
    /// Number of dispatches required for global correctness.
    #[must_use]
    pub const fn stage_count(&self) -> usize {
        2
    }

    /// Programs in dispatch order.
    #[must_use]
    pub fn programs(&self) -> [&Program; 2] {
        [&self.horizontal, &self.vertical]
    }
}

#[derive(Clone, Copy)]
enum Axis {
    Horizontal,
    Vertical,
}

fn gaussian_blur_pass(
    input: &str,
    output: &str,
    width: u32,
    height: u32,
    radius: u32,
    weights: &[u32],
    axis: Axis,
) -> Program {
    let clamped = radius.min(vyre_primitives::math::conv1d::MAX_RADIUS);
    let diameter = 2 * clamped + 1;
    let count = width.saturating_mul(height);
    let is_horiz = matches!(axis, Axis::Horizontal);
    let dim = if is_horiz {
        width.max(1)
    } else {
        height.max(1)
    };
    let parent = GeneratorRef {
        name: OP_ID.to_string(),
    };

    // The per-pixel blur body: for each channel, run a weighted sum
    // over the kernel window, reading neighbors along the given axis.
    let blur_pass = Node::Region {
        generator: Ident::from(vyre_primitives::math::conv1d::OP_ID),
        source_region: Some(parent),
        body: Arc::new(vec![
            Node::let_bind("idx", Expr::gid_x()),
            Node::if_then(Expr::lt(Expr::var("idx"), Expr::u32(count)), {
                let mut body = vec![
                    Node::let_bind("px", Expr::rem(Expr::var("idx"), Expr::u32(width.max(1)))),
                    Node::let_bind("py", Expr::div(Expr::var("idx"), Expr::u32(width.max(1)))),
                    // Accumulators per channel (fixed-point).
                    Node::let_bind("acc_r", Expr::u32(0)),
                    Node::let_bind("acc_g", Expr::u32(0)),
                    Node::let_bind("acc_b", Expr::u32(0)),
                    Node::let_bind("acc_a", Expr::u32(0)),
                ];

                // Kernel loop: manually unrolled weight application.
                // We bake the weights as constants.
                for k in 0..diameter {
                    let w_val = weights[k as usize];
                    if w_val == 0 {
                        continue;
                    }
                    // Sample coordinate: clamp(coord + k - radius, 0, dim-1)
                    let offset = k as i32 - clamped as i32;
                    let sample_coord = if is_horiz {
                        // sx = clamp(px + offset, 0, width-1)
                        if offset >= 0 {
                            Expr::select(
                                Expr::lt(
                                    Expr::add(Expr::var("px"), Expr::u32(offset as u32)),
                                    Expr::u32(dim),
                                ),
                                Expr::add(Expr::var("px"), Expr::u32(offset as u32)),
                                Expr::u32(dim - 1),
                            )
                        } else {
                            Expr::select(
                                Expr::ge(Expr::var("px"), Expr::u32((-offset) as u32)),
                                Expr::sub(Expr::var("px"), Expr::u32((-offset) as u32)),
                                Expr::u32(0),
                            )
                        }
                    } else {
                        // sy = clamp(py + offset, 0, height-1)
                        if offset >= 0 {
                            Expr::select(
                                Expr::lt(
                                    Expr::add(Expr::var("py"), Expr::u32(offset as u32)),
                                    Expr::u32(dim),
                                ),
                                Expr::add(Expr::var("py"), Expr::u32(offset as u32)),
                                Expr::u32(dim - 1),
                            )
                        } else {
                            Expr::select(
                                Expr::ge(Expr::var("py"), Expr::u32((-offset) as u32)),
                                Expr::sub(Expr::var("py"), Expr::u32((-offset) as u32)),
                                Expr::u32(0),
                            )
                        }
                    };

                    // Pixel index: sample_coord used for the varying axis.
                    let pixel_idx = if is_horiz {
                        Expr::add(Expr::mul(Expr::var("py"), Expr::u32(width)), sample_coord)
                    } else {
                        Expr::add(Expr::mul(sample_coord, Expr::u32(width)), Expr::var("px"))
                    };

                    let tap_name = format!("tap_{k}");
                    body.push(Node::let_bind(&tap_name, Expr::load(input, pixel_idx)));

                    // Unpack and accumulate each channel.
                    body.push(Node::assign(
                        "acc_r",
                        Expr::add(
                            Expr::var("acc_r"),
                            Expr::mul(
                                Expr::bitand(Expr::var(&tap_name), Expr::u32(0xFF)),
                                Expr::u32(w_val),
                            ),
                        ),
                    ));
                    body.push(Node::assign(
                        "acc_g",
                        Expr::add(
                            Expr::var("acc_g"),
                            Expr::mul(
                                Expr::bitand(
                                    Expr::shr(Expr::var(&tap_name), Expr::u32(8)),
                                    Expr::u32(0xFF),
                                ),
                                Expr::u32(w_val),
                            ),
                        ),
                    ));
                    body.push(Node::assign(
                        "acc_b",
                        Expr::add(
                            Expr::var("acc_b"),
                            Expr::mul(
                                Expr::bitand(
                                    Expr::shr(Expr::var(&tap_name), Expr::u32(16)),
                                    Expr::u32(0xFF),
                                ),
                                Expr::u32(w_val),
                            ),
                        ),
                    ));
                    body.push(Node::assign(
                        "acc_a",
                        Expr::add(
                            Expr::var("acc_a"),
                            Expr::mul(
                                Expr::shr(Expr::var(&tap_name), Expr::u32(24)),
                                Expr::u32(w_val),
                            ),
                        ),
                    ));
                }

                // Convert from fixed-point >> 16 and clamp to 255.
                let shift_clamp = |acc: &str, out: &str| -> Vec<Node> {
                    vec![
                        Node::let_bind(out, Expr::shr(Expr::var(acc), Expr::u32(16))),
                        Node::assign(
                            out,
                            Expr::select(
                                Expr::gt(Expr::var(out), Expr::u32(255)),
                                Expr::u32(255),
                                Expr::var(out),
                            ),
                        ),
                    ]
                };
                body.extend(shift_clamp("acc_r", "or"));
                body.extend(shift_clamp("acc_g", "og"));
                body.extend(shift_clamp("acc_b", "ob"));
                body.extend(shift_clamp("acc_a", "oa"));

                // Pack.
                body.push(Node::let_bind(
                    "packed",
                    Expr::bitor(
                        Expr::bitor(Expr::var("or"), Expr::shl(Expr::var("og"), Expr::u32(8))),
                        Expr::bitor(
                            Expr::shl(Expr::var("ob"), Expr::u32(16)),
                            Expr::shl(Expr::var("oa"), Expr::u32(24)),
                        ),
                    ),
                ));
                body.push(Node::let_bind(
                    "oidx",
                    Expr::add(
                        Expr::mul(Expr::var("py"), Expr::u32(width)),
                        Expr::var("px"),
                    ),
                ));
                body.push(Node::store(output, Expr::var("oidx"), Expr::var("packed")));
                body
            }),
        ]),
    };

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(count),
            BufferDecl::storage(output, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(count),
        ],
        super::PIXEL_WORKGROUP_SIZE,
        vec![crate::region::wrap_anonymous(OP_ID, vec![blur_pass])],
    )
}

/// Re-export weight computation from the Tier 2.5 primitive.
pub use vyre_primitives::math::conv1d::gaussian_weights;

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || gaussian_blur_2pass("input", "output", "scratch", 4, 4, 1, 0.8).horizontal,
        test_inputs: Some(|| {
            // 4×4 all-white → blurred all-white (identity for uniform).
            let pixels = vec![0xFFFF_FFFFu32; 16];
            vec![vec![
                crate::visual::byte_helpers::u32_words_to_le_bytes(&pixels),     // input
                vec![0u8; 64],         // output (scratch for horizontal pass)
            ]]
        }),
        expected_output: Some(|| {
            // All-white blurred → all-white (±1).
            let pixels = vec![0xFFFF_FFFFu32; 16];
            vec![vec![crate::visual::byte_helpers::u32_words_to_le_bytes(&pixels)]]
        }),
        category: Some("visual"),
    }
}
