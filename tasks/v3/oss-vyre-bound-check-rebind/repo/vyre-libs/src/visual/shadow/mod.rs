//! GPU-computed box shadow with signed-distance-field falloff.
//!
//! Category A composition  -  pure IR. Private SDF helper (single caller,
//! not promoted to Tier 2.5 per LEGO-BLOCK-RULE).

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

const OP_ID: &str = "vyre-libs::visual::box_shadow";

/// Build a Program that renders a box shadow into `output`.
///
/// - `output`: `[u32; width * height]`  -  shadow mask (packed RGBA)
/// - Shadow rect, blur, color, and corner radius baked into IR as constants.
#[must_use]
pub fn box_shadow(
    output: &str,
    img_w: u32,
    img_h: u32,
    rect_x: u32,
    rect_y: u32,
    rect_w: u32,
    rect_h: u32,
    blur_radius: f32,
    color_rgba: u32,
) -> Program {
    let count = img_w.saturating_mul(img_h);

    // Shadow rect center and half-sizes.
    let cx = rect_x + rect_w / 2;
    let cy = rect_y + rect_h / 2;
    let hw = rect_w / 2;
    let hh = rect_h / 2;
    // Blur in pixels (integer). Minimum 1 to avoid div-by-zero.
    let blur = (blur_radius.round() as u32).max(1);

    // Unpack shadow color at compile time.
    let s_r = color_rgba & 0xFF;
    let s_g = (color_rgba >> 8) & 0xFF;
    let s_b = (color_rgba >> 16) & 0xFF;
    let s_a = color_rgba >> 24;

    Program::wrapped(
        vec![
            BufferDecl::storage(output, 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(count),
        ],
        super::PIXEL_WORKGROUP_SIZE,
        vec![crate::region::wrap_anonymous(
            OP_ID,
            vec![
                Node::let_bind("idx", Expr::gid_x()),
                Node::if_then(
                    Expr::lt(Expr::var("idx"), Expr::u32(count)),
                    vec![
                        Node::let_bind("px", Expr::rem(Expr::var("idx"), Expr::u32(img_w.max(1)))),
                        Node::let_bind("py", Expr::div(Expr::var("idx"), Expr::u32(img_w.max(1)))),
                        // Signed distance to rect (Chebyshev for box).
                        // dx = abs(px - cx) - hw
                        // dy = abs(py - cy) - hh
                        // sd = max(dx, dy) ... but we need max(sd, 0)
                        // for outside distance.
                        //
                        // abs(a-b) = abs_diff(a, b) in Vyre IR.
                        Node::let_bind("dx_abs", Expr::abs_diff(Expr::var("px"), Expr::u32(cx))),
                        Node::let_bind("dy_abs", Expr::abs_diff(Expr::var("py"), Expr::u32(cy))),
                        // sd_x = dx_abs - hw (clamped to 0 if inside)
                        // If dx_abs > hw: outside by (dx_abs - hw)
                        // If dx_abs <= hw: inside, sd_x = 0
                        Node::let_bind(
                            "sd_x",
                            Expr::select(
                                Expr::gt(Expr::var("dx_abs"), Expr::u32(hw)),
                                Expr::sub(Expr::var("dx_abs"), Expr::u32(hw)),
                                Expr::u32(0),
                            ),
                        ),
                        Node::let_bind(
                            "sd_y",
                            Expr::select(
                                Expr::gt(Expr::var("dy_abs"), Expr::u32(hh)),
                                Expr::sub(Expr::var("dy_abs"), Expr::u32(hh)),
                                Expr::u32(0),
                            ),
                        ),
                        // Chebyshev distance: max(sd_x, sd_y)
                        Node::let_bind(
                            "sd",
                            Expr::select(
                                Expr::gt(Expr::var("sd_x"), Expr::var("sd_y")),
                                Expr::var("sd_x"),
                                Expr::var("sd_y"),
                            ),
                        ),
                        // Linear falloff: alpha_ratio = clamp(1 - sd/blur, 0, 1)
                        // In integer: alpha_256 = max(0, 256 - sd * 256 / blur)
                        Node::let_bind(
                            "falloff",
                            Expr::select(
                                Expr::ge(
                                    Expr::mul(Expr::var("sd"), Expr::u32(256)),
                                    Expr::mul(Expr::u32(blur), Expr::u32(256)),
                                ),
                                Expr::u32(0),
                                Expr::sub(
                                    Expr::u32(256),
                                    Expr::div(
                                        Expr::mul(Expr::var("sd"), Expr::u32(256)),
                                        Expr::u32(blur),
                                    ),
                                ),
                            ),
                        ),
                        // Inside the rect: full alpha (falloff=256).
                        // Use: if both sd_x==0 AND sd_y==0, pixel is inside rect.
                        Node::let_bind(
                            "inside",
                            Expr::and(
                                Expr::le(Expr::var("dx_abs"), Expr::u32(hw)),
                                Expr::le(Expr::var("dy_abs"), Expr::u32(hh)),
                            ),
                        ),
                        Node::let_bind(
                            "final_falloff",
                            Expr::select(Expr::var("inside"), Expr::u32(256), Expr::var("falloff")),
                        ),
                        // Modulate shadow alpha by falloff.
                        // final_a = s_a * final_falloff / 256
                        Node::let_bind(
                            "final_a_unclamped",
                            super::wide_mul_shr_u32(Expr::u32(s_a), Expr::var("final_falloff"), 8),
                        ),
                        Node::let_bind(
                            "final_a",
                            Expr::select(
                                Expr::gt(Expr::var("final_a_unclamped"), Expr::u32(255)),
                                Expr::u32(255),
                                Expr::var("final_a_unclamped"),
                            ),
                        ),
                        // Pack output pixel.
                        Node::let_bind(
                            "out_px",
                            Expr::bitor(
                                Expr::bitor(
                                    Expr::u32(s_r),
                                    Expr::shl(Expr::u32(s_g), Expr::u32(8)),
                                ),
                                Expr::bitor(
                                    Expr::shl(Expr::u32(s_b), Expr::u32(16)),
                                    Expr::shl(Expr::var("final_a"), Expr::u32(24)),
                                ),
                            ),
                        ),
                        Node::let_bind(
                            "oidx",
                            Expr::add(
                                Expr::mul(Expr::var("py"), Expr::u32(img_w)),
                                Expr::var("px"),
                            ),
                        ),
                        Node::store(output, Expr::var("oidx"), Expr::var("out_px")),
                    ],
                ),
            ],
        )],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || box_shadow("out", 8, 8, 2, 2, 4, 4, 2.0, 0x80_000000),
        test_inputs: Some(|| {
            vec![vec![vec![0u8; 256]]]  // initial 8×8 output buffer
        }),
        expected_output: Some(|| {
            // Center pixel (4,4) is inside rect → alpha = shadow alpha (0x80).
            // Corner pixel (0,0) is far outside → alpha ≈ 0.
            // Exact values computed by reference interpreter.
            vec![
                vec![                                           // run 0
                    vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x40,
                         0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x40,
                         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80,
                         0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x40,
                         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80,
                         0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x40,
                         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80,
                         0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x40,
                         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80,
                         0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x40,
                         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80,
                         0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x40,
                         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x40,
                         0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x40, ],   // output buffer 0 (256 bytes)
                ],
            ]
        }),
        category: Some("visual"),
    }
}
