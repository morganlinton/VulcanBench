//! CSS-compatible linear gradient rasterization.
//!
//! Rasterizes a linear gradient with up to 16 color stops.
//! Category A composition  -  pure IR expressions.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::ir::model::expr::GeneratorRef;

const OP_ID: &str = "vyre-libs::visual::gradient";

/// A color stop with position (0.0..=1.0) and packed RGBA color.
#[derive(Clone, Copy, Debug)]
pub struct ColorStop {
    /// Normalized position along the gradient axis (0.0 = start, 1.0 = end).
    pub position: f32,
    /// Packed RGBA color.
    pub color: u32,
}

/// Build a Program that rasterizes a linear gradient into `output`.
///
/// - `output`: `[u32; width * height]`  -  rasterized gradient (packed RGBA)
/// - `angle_deg`: CSS angle (0 = bottom-to-top, 90 = left-to-right)
/// - `stops`: color stops (must be sorted by position, 2..=16)
#[must_use]
pub fn linear_gradient(
    output: &str,
    width: u32,
    height: u32,
    angle_deg: f32,
    stops: &[ColorStop],
) -> Program {
    try_linear_gradient(output, width, height, angle_deg, stops).unwrap_or_else(|error| {
        crate::builder::invalid_output_program(
            OP_ID,
            output,
            DataType::U32,
            format!("Fix: {error}"),
        )
    })
}

/// Fallible linear-gradient builder.
///
/// # Errors
///
/// Returns an error when the stop count is outside the supported
/// 2..=16 interval.
pub fn try_linear_gradient(
    output: &str,
    width: u32,
    height: u32,
    angle_deg: f32,
    stops: &[ColorStop],
) -> Result<Program, String> {
    let count = width.saturating_mul(height);

    if !(2..=16).contains(&stops.len()) {
        return Err(format!(
            "linear_gradient needs 2..=16 stops, got {}. Fix: provide at least two color stops and at most sixteen.",
            stops.len()
        ));
    }

    // CSS linear-gradient angle convention: 0deg points upward and 90deg
    // points right. The parameter is the pixel projection shifted by the
    // minimum projection of the image corners, then divided by the corner
    // projection range. That keeps negative directions, such as 0deg and
    // 270deg, exact instead of clamping half the image to the first stop.

    let angle_rad = angle_deg.to_radians();
    let dx = angle_rad.sin();
    let dy = -angle_rad.cos();

    // Direction vector scaled to fixed-point.
    let dx_fp = (dx * 65536.0).round() as i32;
    let dy_fp = (dy * 65536.0).round() as i32;

    let width_extent = width.saturating_sub(1) as i64;
    let height_extent = height.saturating_sub(1) as i64;
    let corner_projections = [
        0i64,
        width_extent * i64::from(dx_fp),
        height_extent * i64::from(dy_fp),
        width_extent * i64::from(dx_fp) + height_extent * i64::from(dy_fp),
    ];
    let min_projection = corner_projections.into_iter().min().unwrap_or(0);
    let max_projection = corner_projections.into_iter().max().unwrap_or(0);
    let projection_offset = min_projection
        .saturating_neg()
        .clamp(0, i64::from(u32::MAX)) as u32;
    let projection_range = (max_projection - min_projection).max(1);
    let projection_range_pixels =
        ((projection_range + 65_535) / 65_536).clamp(1, i64::from(u32::MAX)) as u32;

    // Precompute stop positions in fixed-point and colors per channel.
    let stop_positions: Vec<u32> = stops
        .iter()
        .map(|s| (s.position.clamp(0.0, 1.0) * 65536.0).round() as u32)
        .collect();

    let stop_r: Vec<u32> = stops.iter().map(|s| s.color & 0xFF).collect();
    let stop_g: Vec<u32> = stops.iter().map(|s| (s.color >> 8) & 0xFF).collect();
    let stop_b: Vec<u32> = stops.iter().map(|s| (s.color >> 16) & 0xFF).collect();
    let stop_a: Vec<u32> = stops.iter().map(|s| s.color >> 24).collect();

    // Build the body. For each pixel:
    // 1. Compute t (parametric position along gradient)
    // 2. Find enclosing stop pair
    // 3. Lerp between stops

    let mut body = vec![Node::let_bind("idx", Expr::gid_x())];

    body.push(Node::if_then(
        Expr::lt(Expr::var("idx"), Expr::u32(count)),
        {
            let mut inner = vec![
                Node::let_bind("px", Expr::rem(Expr::var("idx"), Expr::u32(width.max(1)))),
                Node::let_bind("py", Expr::div(Expr::var("idx"), Expr::u32(width.max(1)))),
            ];

            // Compute dot product: dp = px * dx + py * dy
            // Handle signed direction with select.
            let dp_x = if dx_fp >= 0 {
                Expr::mul(Expr::var("px"), Expr::u32(dx_fp as u32))
            } else {
                // Negative: dp_x = -(px * |dx|)
                // We'll handle sign at the end.
                Expr::mul(Expr::var("px"), Expr::u32((-dx_fp) as u32))
            };
            let dp_y = if dy_fp >= 0 {
                Expr::mul(Expr::var("py"), Expr::u32(dy_fp as u32))
            } else {
                Expr::mul(Expr::var("py"), Expr::u32((-dy_fp) as u32))
            };

            // Signed projection is represented as positive and negative
            // unsigned parts, then shifted by `-min_corner_projection`.
            let pos_part = Expr::add(
                if dx_fp >= 0 {
                    dp_x.clone()
                } else {
                    Expr::u32(0)
                },
                if dy_fp >= 0 {
                    dp_y.clone()
                } else {
                    Expr::u32(0)
                },
            );
            let neg_part = Expr::add(
                if dx_fp < 0 { dp_x } else { Expr::u32(0) },
                if dy_fp < 0 { dp_y } else { Expr::u32(0) },
            );

            inner.push(Node::let_bind("pos_dp", pos_part));
            inner.push(Node::let_bind("neg_dp", neg_part));

            // t = (dot(pixel, direction) - min_corner_projection) / range.
            // `raw_dp` is still fixed-point 16.16, so division by the
            // pixel-space range preserves a 16.16 normalized parameter while
            // avoiding a wide multiply on backends without native u64.
            let shifted_pos = Expr::add(Expr::var("pos_dp"), Expr::u32(projection_offset));
            inner.push(Node::let_bind(
                "raw_dp",
                Expr::select(
                    Expr::ge(shifted_pos.clone(), Expr::var("neg_dp")),
                    Expr::sub(shifted_pos, Expr::var("neg_dp")),
                    Expr::u32(0),
                ),
            ));
            inner.push(Node::let_bind(
                "t",
                Expr::select(
                    Expr::gt(
                        Expr::div(Expr::var("raw_dp"), Expr::u32(projection_range_pixels)),
                        Expr::u32(65536),
                    ),
                    Expr::u32(65536),
                    Expr::div(Expr::var("raw_dp"), Expr::u32(projection_range_pixels)),
                ),
            ));

            // Find enclosing stop pair and lerp.
            // For simplicity with IR, we do a flat scan: pick the last stop
            // whose position <= t, then lerp between it and the next.
            inner.push(Node::let_bind("out_r", Expr::u32(stop_r[0])));
            inner.push(Node::let_bind("out_g", Expr::u32(stop_g[0])));
            inner.push(Node::let_bind("out_b", Expr::u32(stop_b[0])));
            inner.push(Node::let_bind("out_a", Expr::u32(stop_a[0])));

            for i in 0..stops.len() - 1 {
                let t0 = stop_positions[i];
                let t1 = stop_positions[i + 1];
                let span = if t1 > t0 { t1 - t0 } else { 1 }; // avoid div by 0

                // If t >= t0 AND t < t1: lerp between stop[i] and stop[i+1]
                // Channel delta is rounded in fixed-point stop space.
                let lerp_ch = |ch: &str, c0: u32, c1: u32| -> Node {
                    let stop_delta = Expr::sub(Expr::var("t"), Expr::u32(t0));
                    let rounded_delta = |delta: u32| {
                        Expr::div(
                            Expr::add(
                                Expr::mul(Expr::u32(delta), stop_delta.clone()),
                                Expr::u32(span / 2),
                            ),
                            Expr::u32(span),
                        )
                    };
                    Node::assign(
                        ch,
                        Expr::select(
                            Expr::and(
                                Expr::ge(Expr::var("t"), Expr::u32(t0)),
                                Expr::lt(Expr::var("t"), Expr::u32(t1)),
                            ),
                            // lerp: c0 + round((c1 - c0) * (t - t0) / span)
                            if c1 >= c0 {
                                Expr::add(Expr::u32(c0), rounded_delta(c1 - c0))
                            } else {
                                Expr::sub(Expr::u32(c0), rounded_delta(c0 - c1))
                            },
                            Expr::var(ch),
                        ),
                    )
                };

                inner.push(lerp_ch("out_r", stop_r[i], stop_r[i + 1]));
                inner.push(lerp_ch("out_g", stop_g[i], stop_g[i + 1]));
                inner.push(lerp_ch("out_b", stop_b[i], stop_b[i + 1]));
                inner.push(lerp_ch("out_a", stop_a[i], stop_a[i + 1]));
            }

            // If t >= last stop position, use last stop color.
            let last = stops.len() - 1;
            inner.push(Node::assign(
                "out_r",
                Expr::select(
                    Expr::ge(Expr::var("t"), Expr::u32(stop_positions[last])),
                    Expr::u32(stop_r[last]),
                    Expr::var("out_r"),
                ),
            ));
            inner.push(Node::assign(
                "out_g",
                Expr::select(
                    Expr::ge(Expr::var("t"), Expr::u32(stop_positions[last])),
                    Expr::u32(stop_g[last]),
                    Expr::var("out_g"),
                ),
            ));
            inner.push(Node::assign(
                "out_b",
                Expr::select(
                    Expr::ge(Expr::var("t"), Expr::u32(stop_positions[last])),
                    Expr::u32(stop_b[last]),
                    Expr::var("out_b"),
                ),
            ));
            inner.push(Node::assign(
                "out_a",
                Expr::select(
                    Expr::ge(Expr::var("t"), Expr::u32(stop_positions[last])),
                    Expr::u32(stop_a[last]),
                    Expr::var("out_a"),
                ),
            ));

            // Pack output.
            inner.push(Node::let_bind(
                "packed",
                Expr::bitor(
                    Expr::bitor(
                        Expr::var("out_r"),
                        Expr::shl(Expr::var("out_g"), Expr::u32(8)),
                    ),
                    Expr::bitor(
                        Expr::shl(Expr::var("out_b"), Expr::u32(16)),
                        Expr::shl(Expr::var("out_a"), Expr::u32(24)),
                    ),
                ),
            ));
            inner.push(Node::let_bind(
                "oidx",
                Expr::add(
                    Expr::mul(Expr::var("py"), Expr::u32(width)),
                    Expr::var("px"),
                ),
            ));
            inner.push(Node::store(output, Expr::var("oidx"), Expr::var("packed")));
            inner
        },
    ));

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(output, 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(count),
        ],
        super::PIXEL_WORKGROUP_SIZE,
        vec![crate::region::wrap_anonymous(
            OP_ID,
            vec![crate::region::wrap_child(
                vyre_primitives::visual::packed_rgba_map::OP_ID,
                GeneratorRef {
                    name: OP_ID.to_string(),
                },
                body,
            )],
        )],
    ))
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || linear_gradient(
            "output", 4, 1, 90.0,
            &[
                ColorStop { position: 0.0, color: 0xFF_0000FF }, // red
                ColorStop { position: 1.0, color: 0xFF_FF0000 }, // blue
            ],
        ),
        test_inputs: Some(|| {
            vec![vec![vec![0u8; 16]]]  // initial 4×1 output buffer
        }),
        expected_output: Some(|| {
            // 4-pixel horizontal gradient: red → blue.
            // Pixel 0: pure red, Pixel 3: pure blue.
            // Exact values depend on interpolation rounding.
            let expected = [0xFF_0000FFu32, 0xFF_5500AAu32, 0xFF_AA0055u32, 0xFF_FF0000u32];
            vec![vec![crate::visual::byte_helpers::u32_words_to_le_bytes(&expected)]]
        }),
        category: Some("visual"),
    }
}
