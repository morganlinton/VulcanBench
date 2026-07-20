//! 2× box-filter downsample for half-resolution blur.
//!
//! Averages each 2×2 block of pixels into one output pixel.
//! Category A composition  -  pure IR. No Tier 2.5 primitives.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

const OP_ID: &str = "vyre-libs::visual::downsample";

/// Build a Program that 2× downsamples `input` into `output`.
///
/// - `input`:  `[u32; width * height]`  -  source pixels (packed RGBA)
/// - `output`: `[u32; (width/2) * (height/2)]`  -  downsampled result
/// - Width and height must be even.
#[must_use]
pub fn downsample_2x(input: &str, output: &str, width: u32, height: u32) -> Program {
    let out_w = width / 2;
    let out_h = height / 2;
    let input_count = width.saturating_mul(height);
    let output_count = out_w.saturating_mul(out_h);

    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(input_count),
            BufferDecl::storage(output, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(output_count),
        ],
        super::PIXEL_WORKGROUP_SIZE,
        vec![crate::region::wrap_anonymous(
            OP_ID,
            vec![
                Node::let_bind("idx", Expr::gid_x()),
                Node::if_then(
                    Expr::lt(Expr::var("idx"), Expr::u32(output_count)),
                    vec![
                        Node::let_bind("ox", Expr::rem(Expr::var("idx"), Expr::u32(out_w.max(1)))),
                        Node::let_bind("oy", Expr::div(Expr::var("idx"), Expr::u32(out_w.max(1)))),
                        // Source pixel coordinates.
                        Node::let_bind("sx", Expr::mul(Expr::var("ox"), Expr::u32(2))),
                        Node::let_bind("sy", Expr::mul(Expr::var("oy"), Expr::u32(2))),
                        // Load 4 source pixels.
                        Node::let_bind(
                            "p00",
                            Expr::load(
                                input,
                                Expr::add(
                                    Expr::mul(Expr::var("sy"), Expr::u32(width)),
                                    Expr::var("sx"),
                                ),
                            ),
                        ),
                        Node::let_bind(
                            "p10",
                            Expr::load(
                                input,
                                Expr::add(
                                    Expr::mul(Expr::var("sy"), Expr::u32(width)),
                                    Expr::add(Expr::var("sx"), Expr::u32(1)),
                                ),
                            ),
                        ),
                        Node::let_bind(
                            "p01",
                            Expr::load(
                                input,
                                Expr::add(
                                    Expr::mul(
                                        Expr::add(Expr::var("sy"), Expr::u32(1)),
                                        Expr::u32(width),
                                    ),
                                    Expr::var("sx"),
                                ),
                            ),
                        ),
                        Node::let_bind(
                            "p11",
                            Expr::load(
                                input,
                                Expr::add(
                                    Expr::mul(
                                        Expr::add(Expr::var("sy"), Expr::u32(1)),
                                        Expr::u32(width),
                                    ),
                                    Expr::add(Expr::var("sx"), Expr::u32(1)),
                                ),
                            ),
                        ),
                        // Average each channel: (c0+c1+c2+c3+2) >> 2
                        // R channel
                        Node::let_bind(
                            "r",
                            Expr::shr(
                                Expr::add(
                                    Expr::add(
                                        Expr::add(
                                            Expr::bitand(Expr::var("p00"), Expr::u32(0xFF)),
                                            Expr::bitand(Expr::var("p10"), Expr::u32(0xFF)),
                                        ),
                                        Expr::add(
                                            Expr::bitand(Expr::var("p01"), Expr::u32(0xFF)),
                                            Expr::bitand(Expr::var("p11"), Expr::u32(0xFF)),
                                        ),
                                    ),
                                    Expr::u32(2),
                                ),
                                Expr::u32(2),
                            ),
                        ),
                        // G channel
                        Node::let_bind(
                            "g",
                            Expr::shr(
                                Expr::add(
                                    Expr::add(
                                        Expr::add(
                                            Expr::bitand(
                                                Expr::shr(Expr::var("p00"), Expr::u32(8)),
                                                Expr::u32(0xFF),
                                            ),
                                            Expr::bitand(
                                                Expr::shr(Expr::var("p10"), Expr::u32(8)),
                                                Expr::u32(0xFF),
                                            ),
                                        ),
                                        Expr::add(
                                            Expr::bitand(
                                                Expr::shr(Expr::var("p01"), Expr::u32(8)),
                                                Expr::u32(0xFF),
                                            ),
                                            Expr::bitand(
                                                Expr::shr(Expr::var("p11"), Expr::u32(8)),
                                                Expr::u32(0xFF),
                                            ),
                                        ),
                                    ),
                                    Expr::u32(2),
                                ),
                                Expr::u32(2),
                            ),
                        ),
                        // B channel
                        Node::let_bind(
                            "b",
                            Expr::shr(
                                Expr::add(
                                    Expr::add(
                                        Expr::add(
                                            Expr::bitand(
                                                Expr::shr(Expr::var("p00"), Expr::u32(16)),
                                                Expr::u32(0xFF),
                                            ),
                                            Expr::bitand(
                                                Expr::shr(Expr::var("p10"), Expr::u32(16)),
                                                Expr::u32(0xFF),
                                            ),
                                        ),
                                        Expr::add(
                                            Expr::bitand(
                                                Expr::shr(Expr::var("p01"), Expr::u32(16)),
                                                Expr::u32(0xFF),
                                            ),
                                            Expr::bitand(
                                                Expr::shr(Expr::var("p11"), Expr::u32(16)),
                                                Expr::u32(0xFF),
                                            ),
                                        ),
                                    ),
                                    Expr::u32(2),
                                ),
                                Expr::u32(2),
                            ),
                        ),
                        // A channel
                        Node::let_bind(
                            "a",
                            Expr::shr(
                                Expr::add(
                                    Expr::add(
                                        Expr::add(
                                            Expr::shr(Expr::var("p00"), Expr::u32(24)),
                                            Expr::shr(Expr::var("p10"), Expr::u32(24)),
                                        ),
                                        Expr::add(
                                            Expr::shr(Expr::var("p01"), Expr::u32(24)),
                                            Expr::shr(Expr::var("p11"), Expr::u32(24)),
                                        ),
                                    ),
                                    Expr::u32(2),
                                ),
                                Expr::u32(2),
                            ),
                        ),
                        // Pack RGBA.
                        Node::let_bind(
                            "packed",
                            Expr::bitor(
                                Expr::bitor(
                                    Expr::var("r"),
                                    Expr::shl(Expr::var("g"), Expr::u32(8)),
                                ),
                                Expr::bitor(
                                    Expr::shl(Expr::var("b"), Expr::u32(16)),
                                    Expr::shl(Expr::var("a"), Expr::u32(24)),
                                ),
                            ),
                        ),
                        // Write output.
                        Node::let_bind(
                            "oidx",
                            Expr::add(
                                Expr::mul(Expr::var("oy"), Expr::u32(out_w)),
                                Expr::var("ox"),
                            ),
                        ),
                        Node::store(output, Expr::var("oidx"), Expr::var("packed")),
                    ],
                ),
            ],
        )],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || downsample_2x("input", "output", 4, 4),
        test_inputs: Some(|| {
            // 4×4 all-white → 2×2 all-white
            let input = vec![0xFFFF_FFFFu32; 16];
            vec![vec![
                crate::visual::byte_helpers::u32_words_to_le_bytes(&input),
                vec![0u8; 16],
            ]]
        }),
        expected_output: Some(|| {
            let expected = vec![0xFFFF_FFFFu32; 4];
            vec![vec![crate::visual::byte_helpers::u32_words_to_le_bytes(&expected)]]
        }),
        category: Some("visual"),
    }
}
