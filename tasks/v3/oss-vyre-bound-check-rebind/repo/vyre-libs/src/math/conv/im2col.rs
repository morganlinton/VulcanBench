//! Im2col reshape: rearrange an `[H, W]` image into an
//! `[H*W, 9]` matrix of flattened 3x3 patches centered at each
//! output pixel, with zero-padding on the boundary.
//!
//! Once an image is in im2col form, 2D convolution becomes a
//! matrix-vector multiply: `output[i] = sum_k im2col[i, k] *
//! kernel[k]`. This lets the existing tiled / vectorised `matmul`
//! primitive carry the convolution work  -  at the cost of `H*W*9 - H*W`
//! extra F32 of memory for the patch matrix.
//!
//! Decision wrapper (im2col-vs-direct, ROADMAP H3): use im2col when
//! `H*W` is large enough that the matmul tile / vectorisation win
//! exceeds the patch-matrix memory cost, and direct conv otherwise.
//! The decision substrate lives beside this primitive once the
//! profiling hooks are wired.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;
use crate::test_support::byte_pack::f32_bytes;

const OP_ID: &str = "vyre-libs::math::conv::im2col_3x3";

/// Build a Program that reshapes an `[H, W]` row-major F32 image
/// into an `[H*W, 9]` row-major F32 matrix of flattened 3x3
/// patches. Patches are zero-padded at the boundary.
///
/// # Errors
///
/// Returns `Err` when `h * w * 9` overflows `u32`.
pub fn im2col_3x3(input: &str, output: &str, h: u32, w: u32) -> Result<Program, String> {
    let pixels = h
        .checked_mul(w)
        .ok_or_else(|| "Fix: im2col_3x3 h*w overflows u32; reduce dimensions.".to_string())?;
    let cells = pixels
        .checked_mul(9)
        .ok_or_else(|| "Fix: im2col_3x3 h*w*9 overflows u32; reduce dimensions.".to_string())?;
    let mut tap_writes: Vec<Node> = Vec::new();
    for ky in 0..3u32 {
        for kx in 0..3u32 {
            let dy = (ky as i32) - 1;
            let dx = (kx as i32) - 1;
            let y_in_bounds = if dy < 0 {
                Expr::ge(Expr::var("y"), Expr::u32((-dy) as u32))
            } else {
                Expr::lt(
                    Expr::add(Expr::var("y"), Expr::u32(dy as u32)),
                    Expr::u32(h),
                )
            };
            let x_in_bounds = if dx < 0 {
                Expr::ge(Expr::var("x"), Expr::u32((-dx) as u32))
            } else {
                Expr::lt(
                    Expr::add(Expr::var("x"), Expr::u32(dx as u32)),
                    Expr::u32(w),
                )
            };
            let ny = if dy < 0 {
                Expr::sub(Expr::var("y"), Expr::u32((-dy) as u32))
            } else if dy > 0 {
                Expr::add(Expr::var("y"), Expr::u32(dy as u32))
            } else {
                Expr::var("y")
            };
            let nx = if dx < 0 {
                Expr::sub(Expr::var("x"), Expr::u32((-dx) as u32))
            } else if dx > 0 {
                Expr::add(Expr::var("x"), Expr::u32(dx as u32))
            } else {
                Expr::var("x")
            };
            let load_idx = Expr::add(Expr::mul(ny, Expr::u32(w)), nx);
            let in_bounds = Expr::and(y_in_bounds, x_in_bounds);
            let value = Expr::select(in_bounds, Expr::load(input, load_idx), Expr::f32(0.0));
            // im2col[flat, ky*3 + kx]
            let row = ky * 3 + kx;
            let dest_idx = Expr::add(Expr::mul(Expr::var("flat"), Expr::u32(9)), Expr::u32(row));
            tap_writes.push(Node::store(output, dest_idx, value));
        }
    }
    let body = vec![
        Node::let_bind("flat", Expr::InvocationId { axis: 0 }),
        Node::if_then(
            Expr::lt(Expr::var("flat"), Expr::u32(pixels)),
            vec![
                Node::let_bind("y", Expr::div(Expr::var("flat"), Expr::u32(w))),
                Node::let_bind("x", Expr::rem(Expr::var("flat"), Expr::u32(w))),
                Node::Block(tap_writes),
            ],
        ),
    ];
    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(pixels),
            BufferDecl::output(output, 1, DataType::F32).with_count(cells),
        ],
        [64, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    ))
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || {
            im2col_3x3("input", "output", 4, 4).unwrap_or_else(|error| {
                crate::builder::invalid_output_program(
                    OP_ID,
                    "output",
                    DataType::F32,
                    error,
                )
            })
        },
        test_inputs: Some(|| {
            vec![vec![f32_bytes(&im2col_fixture_input())]]
        }),
        expected_output: Some(|| {
            vec![vec![f32_bytes(&naive_im2col_3x3(&im2col_fixture_input(), 4, 4))]]
        }),
        category: Some("math"),
    }
}

fn im2col_fixture_input() -> Vec<f32> {
    (0..16).map(|i| i as f32 + 1.0).collect()
}

fn naive_im2col_3x3(input: &[f32], h: usize, w: usize) -> Vec<f32> {
    let mut out = vec![0.0_f32; h * w * 9];
    for y in 0..h {
        for x in 0..w {
            let flat = y * w + x;
            for ky in 0..3usize {
                for kx in 0..3usize {
                    let ny = (y as i32) + (ky as i32) - 1;
                    let nx = (x as i32) + (kx as i32) - 1;
                    let value = if ny < 0 || ny >= h as i32 || nx < 0 || nx >= w as i32 {
                        0.0
                    } else {
                        input[(ny as usize) * w + (nx as usize)]
                    };
                    out[flat * 9 + ky * 3 + kx] = value;
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::f32_bytes;
    use vyre_reference::value::Value;

    fn decode(bytes: &[u8]) -> Vec<f32> {
        vyre_primitives::wire::decode_f32_le_bytes_all(bytes)
    }

    fn run(input: &[f32], h: u32, w: u32) -> Vec<f32> {
        let prog = im2col_3x3("input", "output", h, w).expect("Fix: build");
        let outputs = vyre_reference::reference_eval(&prog, &[Value::from(f32_bytes(input))])
            .expect("Fix: im2col_3x3 must execute in the reference interpreter.");
        decode(&outputs[0].to_bytes())
    }

    /// Im2col on a 4x4 input matches the naive reference layout.
    #[test]
    fn im2col_matches_naive_on_4x4() {
        let input: Vec<f32> = (0..16).map(|i| i as f32 + 1.0).collect();
        let actual = run(&input, 4, 4);
        let expected = naive_im2col_3x3(&input, 4, 4);
        assert_eq!(actual.len(), expected.len());
        for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
            assert!((a - e).abs() <= 1.0e-5, "lane {i}: {a} != {e}");
        }
    }

    /// Centre pixel of a 3x3 image holds the full input as a row
    /// (no boundary clipping)  -  `im2col[4, k] = input[k]` for the
    /// (1, 1) pixel.
    #[test]
    fn im2col_centre_pixel_holds_full_3x3() {
        let input: Vec<f32> = (0..9).map(|i| i as f32 + 1.0).collect();
        let actual = run(&input, 3, 3);
        // Centre pixel is (1, 1) flat index 4; its 9 taps span rows 0..3 cols 0..3 of input
        let centre_row = &actual[4 * 9..4 * 9 + 9];
        for (k, &v) in centre_row.iter().enumerate() {
            assert!(
                (v - input[k]).abs() <= 1.0e-5,
                "tap {k}: {v} != {}",
                input[k]
            );
        }
    }

    /// Corner pixel (0, 0) of a 3x3 image clips 5 of 9 taps to 0.
    /// Only the bottom-right 2x2 sub-window survives  -  taps (1,1),
    /// (1,2), (2,1), (2,2) of the kernel correspond to input
    /// positions (0,0), (0,1), (1,0), (1,1).
    #[test]
    fn im2col_corner_pixel_zero_pads_5_of_9_taps() {
        let input: Vec<f32> = (0..9).map(|i| i as f32 + 1.0).collect();
        let actual = run(&input, 3, 3);
        let corner_row = &actual[0..9];
        // ky=0 (above), kx=0/1/2 → all out of bounds → 0
        assert_eq!(corner_row[0], 0.0);
        assert_eq!(corner_row[1], 0.0);
        assert_eq!(corner_row[2], 0.0);
        // ky=1 (current row), kx=0 → out of bounds left → 0
        assert_eq!(corner_row[3], 0.0);
        // ky=1, kx=1 → input[0, 0] = 1.0
        assert!((corner_row[4] - 1.0).abs() <= 1.0e-5);
        // ky=1, kx=2 → input[0, 1] = 2.0
        assert!((corner_row[5] - 2.0).abs() <= 1.0e-5);
        // ky=2 (below), kx=0 → out of bounds left → 0
        assert_eq!(corner_row[6], 0.0);
        // ky=2, kx=1 → input[1, 0] = 4.0
        assert!((corner_row[7] - 4.0).abs() <= 1.0e-5);
        // ky=2, kx=2 → input[1, 1] = 5.0
        assert!((corner_row[8] - 5.0).abs() <= 1.0e-5);
    }
}
