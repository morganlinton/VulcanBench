//! Porter-Duff alpha compositing ("over" operation).
//!
//! `result = fg + bg * (1 - fg_alpha)`
//!
//! Category A composition  -  pure IR over existing expressions.
//! No Tier 2.5 primitives consumed.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::ir::model::expr::GeneratorRef;

const OP_ID: &str = "vyre-libs::visual::composite";

/// Build a Program that composites `fg` over `bg` using Porter-Duff
/// "over" arithmetic, writing the result to `output`.
///
/// All buffers are `[u32; count]`  -  packed RGBA pixels.
#[must_use]
pub fn alpha_over(fg: &str, bg: &str, output: &str, count: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(fg, 0, BufferAccess::ReadOnly, DataType::U32).with_count(count),
            BufferDecl::storage(bg, 1, BufferAccess::ReadOnly, DataType::U32).with_count(count),
            BufferDecl::storage(output, 2, BufferAccess::ReadWrite, DataType::U32)
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
                vec![
                    Node::let_bind("idx", Expr::gid_x()),
                    Node::if_then(
                        Expr::lt(Expr::var("idx"), Expr::u32(count)),
                        vec![
                            // Load foreground and background pixels.
                            Node::let_bind("fg_px", Expr::load(fg, Expr::var("idx"))),
                            Node::let_bind("bg_px", Expr::load(bg, Expr::var("idx"))),
                            // Unpack fg.
                            Node::let_bind(
                                "fg_r",
                                Expr::bitand(Expr::var("fg_px"), Expr::u32(0xFF)),
                            ),
                            Node::let_bind(
                                "fg_g",
                                Expr::bitand(
                                    Expr::shr(Expr::var("fg_px"), Expr::u32(8)),
                                    Expr::u32(0xFF),
                                ),
                            ),
                            Node::let_bind(
                                "fg_b",
                                Expr::bitand(
                                    Expr::shr(Expr::var("fg_px"), Expr::u32(16)),
                                    Expr::u32(0xFF),
                                ),
                            ),
                            Node::let_bind("fg_a", Expr::shr(Expr::var("fg_px"), Expr::u32(24))),
                            // Unpack bg.
                            Node::let_bind(
                                "bg_r",
                                Expr::bitand(Expr::var("bg_px"), Expr::u32(0xFF)),
                            ),
                            Node::let_bind(
                                "bg_g",
                                Expr::bitand(
                                    Expr::shr(Expr::var("bg_px"), Expr::u32(8)),
                                    Expr::u32(0xFF),
                                ),
                            ),
                            Node::let_bind(
                                "bg_b",
                                Expr::bitand(
                                    Expr::shr(Expr::var("bg_px"), Expr::u32(16)),
                                    Expr::u32(0xFF),
                                ),
                            ),
                            Node::let_bind("bg_a", Expr::shr(Expr::var("bg_px"), Expr::u32(24))),
                            // inv_a = 255 - fg_a
                            Node::let_bind("inv_a", Expr::sub(Expr::u32(255), Expr::var("fg_a"))),
                            // Porter-Duff over per channel:
                            //   out_c = fg_c + bg_c * inv_a / 255
                            //   division by 255 ≈ (x * 257 + 256) >> 16
                            // Simplified: (bg_c * inv_a + 127) / 255
                            //   ≈ (bg_c * inv_a * 257 + 256) >> 16
                            // But for GPU integer math, the simplest correct
                            // form is: (bg_c * inv_a + 128) / 255
                            //   ≈ ((bg_c * inv_a + 128) * 257) >> 16
                            Node::let_bind(
                                "out_r",
                                Expr::add(
                                    Expr::var("fg_r"),
                                    super::wide_mul_shr_u32(
                                        Expr::add(
                                            Expr::mul(Expr::var("bg_r"), Expr::var("inv_a")),
                                            Expr::u32(128),
                                        ),
                                        Expr::u32(257),
                                        16,
                                    ),
                                ),
                            ),
                            Node::let_bind(
                                "out_g",
                                Expr::add(
                                    Expr::var("fg_g"),
                                    super::wide_mul_shr_u32(
                                        Expr::add(
                                            Expr::mul(Expr::var("bg_g"), Expr::var("inv_a")),
                                            Expr::u32(128),
                                        ),
                                        Expr::u32(257),
                                        16,
                                    ),
                                ),
                            ),
                            Node::let_bind(
                                "out_b",
                                Expr::add(
                                    Expr::var("fg_b"),
                                    super::wide_mul_shr_u32(
                                        Expr::add(
                                            Expr::mul(Expr::var("bg_b"), Expr::var("inv_a")),
                                            Expr::u32(128),
                                        ),
                                        Expr::u32(257),
                                        16,
                                    ),
                                ),
                            ),
                            // out_a = fg_a + bg_a * inv_a / 255
                            Node::let_bind(
                                "out_a",
                                Expr::add(
                                    Expr::var("fg_a"),
                                    super::wide_mul_shr_u32(
                                        Expr::add(
                                            Expr::mul(Expr::var("bg_a"), Expr::var("inv_a")),
                                            Expr::u32(128),
                                        ),
                                        Expr::u32(257),
                                        16,
                                    ),
                                ),
                            ),
                            // Clamp to 255 using select.
                            Node::let_bind(
                                "cr",
                                Expr::select(
                                    Expr::gt(Expr::var("out_r"), Expr::u32(255)),
                                    Expr::u32(255),
                                    Expr::var("out_r"),
                                ),
                            ),
                            Node::let_bind(
                                "cg",
                                Expr::select(
                                    Expr::gt(Expr::var("out_g"), Expr::u32(255)),
                                    Expr::u32(255),
                                    Expr::var("out_g"),
                                ),
                            ),
                            Node::let_bind(
                                "cb",
                                Expr::select(
                                    Expr::gt(Expr::var("out_b"), Expr::u32(255)),
                                    Expr::u32(255),
                                    Expr::var("out_b"),
                                ),
                            ),
                            Node::let_bind(
                                "ca",
                                Expr::select(
                                    Expr::gt(Expr::var("out_a"), Expr::u32(255)),
                                    Expr::u32(255),
                                    Expr::var("out_a"),
                                ),
                            ),
                            // Pack RGBA.
                            Node::let_bind(
                                "packed",
                                Expr::bitor(
                                    Expr::bitor(
                                        Expr::var("cr"),
                                        Expr::shl(Expr::var("cg"), Expr::u32(8)),
                                    ),
                                    Expr::bitor(
                                        Expr::shl(Expr::var("cb"), Expr::u32(16)),
                                        Expr::shl(Expr::var("ca"), Expr::u32(24)),
                                    ),
                                ),
                            ),
                            Node::store(output, Expr::var("idx"), Expr::var("packed")),
                        ],
                    ),
                ],
            )],
        )],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || alpha_over("fg", "bg", "out", 2),
        test_inputs: Some(|| {
            // Pixel 0: semi-transparent red (128 alpha) over opaque blue.
            // Pixel 1: fully opaque green over opaque white.
            let fg = [0x8000_00FFu32, 0xFF00_FF00u32]; // RGBA: R=255 A=128; R=0 G=255 A=255
            let bg = [0xFF_FF0000u32, 0xFFFF_FFFFu32]; // RGBA: B=255 A=255; white A=255
            vec![vec![
                crate::visual::byte_helpers::u32_words_to_le_bytes(&fg),
                crate::visual::byte_helpers::u32_words_to_le_bytes(&bg),
                vec![0u8; 8],   // output
            ]]
        }),
        expected_output: Some(|| {
            // Pixel 0: fg_r=255 fg_a=128, bg_b=255 bg_a=255
            //   inv_a = 127
            //   out_r = 255 + 0 = 255
            //   out_g = 0 + 0 = 0
            //   out_b = 0 + (255*127+128)*257>>16 = 0 + 127 = 127
            //   out_a = 128 + (255*127+128)*257>>16 = 128 + 127 = 255
            // Pixel 1: fg fully opaque → output == fg
            //   out = 0xFF00FF00 (green)
            let expected = [0xFF7F_00FFu32, 0xFF00_FF00u32];
            vec![vec![crate::visual::byte_helpers::u32_words_to_le_bytes(&expected)]]
        }),
        category: Some("visual"),
    }
}
