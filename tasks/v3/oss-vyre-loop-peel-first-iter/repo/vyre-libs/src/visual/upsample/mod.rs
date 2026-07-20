//! 2× nearest-neighbor upsample for the half-resolution blur path.
//!
//! Each input pixel maps to a 2×2 block in the output. This is intentionally
//! nearest-neighbor (no bilinear) because the input is already blurred  -
//! the blur itself provides the smoothing that bilinear would add.
//!
//! Category A composition  -  pure IR. No Tier 2.5 primitives.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

const OP_ID: &str = "vyre-libs::visual::upsample";

/// Build a Program that 2× upsamples `input` into `output`.
///
/// - `input`:  `[u32; (width/2) * (height/2)]`  -  source pixels (packed RGBA)
/// - `output`: `[u32; width * height]`  -  upsampled result
/// - `width`, `height`: the FULL output dimensions (must be even).
#[must_use]
pub fn upsample_2x(input: &str, output: &str, width: u32, height: u32) -> Program {
    let in_w = width / 2;
    let in_h = height / 2;
    let input_count = in_w.saturating_mul(in_h);
    let output_count = width.saturating_mul(height);

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
                        // Output coordinates.
                        Node::let_bind("ox", Expr::rem(Expr::var("idx"), Expr::u32(width.max(1)))),
                        Node::let_bind("oy", Expr::div(Expr::var("idx"), Expr::u32(width.max(1)))),
                        // Map to input coordinates (integer division = nearest-neighbor).
                        Node::let_bind("ix", Expr::div(Expr::var("ox"), Expr::u32(2))),
                        Node::let_bind("iy", Expr::div(Expr::var("oy"), Expr::u32(2))),
                        // Load input pixel.
                        Node::let_bind(
                            "pixel",
                            Expr::load(
                                input,
                                Expr::add(
                                    Expr::mul(Expr::var("iy"), Expr::u32(in_w.max(1))),
                                    Expr::var("ix"),
                                ),
                            ),
                        ),
                        // Write to output.
                        Node::store(output, Expr::var("idx"), Expr::var("pixel")),
                    ],
                ),
            ],
        )],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || upsample_2x("input", "output", 4, 4),
        test_inputs: Some(|| {
            // 2×2 all-white → 4×4 all-white
            let input = vec![0xFFFF_FFFFu32; 4];
            vec![vec![
                crate::visual::byte_helpers::u32_words_to_le_bytes(&input),
                vec![0u8; 64],
            ]]
        }),
        expected_output: Some(|| {
            let expected = vec![0xFFFF_FFFFu32; 16];
            vec![vec![crate::visual::byte_helpers::u32_words_to_le_bytes(&expected)]]
        }),
        category: Some("visual"),
    }
}
