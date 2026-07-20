//! 4-point complex radix-2 FFT.
//!
//! For complex input `x[0..4]` (interleaved re/im in a length-8
//! F32 buffer), compute `X[k] = sum_n x[n] * exp(-2πi·n·k/4)` for
//! `k ∈ {0,1,2,3}`. Twiddle factors for N=4:
//!
//! ```text
//! W^0 = 1
//! W^1 = exp(-iπ/2) = -i  → multiplying (re, im) yields (im, -re)
//! W^2 = -1
//! W^3 = exp(-i·3π/2) = i → multiplying (re, im) yields (-im, re)
//! ```
//!
//! Direct expansion of the DFT sum:
//!
//! ```text
//! X[0] = x[0] + x[1] + x[2] + x[3]
//! X[1] = x[0] + (-i)·x[1] + (-1)·x[2] + (i)·x[3]
//!      = (x0r + x1i - x2r - x3i,  x0i - x1r - x2i + x3r)
//! X[2] = x[0] - x[1] + x[2] - x[3]
//! X[3] = x[0] + (i)·x[1] + (-1)·x[2] + (-i)·x[3]
//!      = (x0r - x1i - x2r + x3i,  x0i + x1r - x2i - x3r)
//! ```
//!
//! Implementing the formula directly avoids the recursive butterfly
//! plumbing for the base case and keeps the IR straight-line for
//! const-fold + CSE to compress.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::region::wrap_anonymous;

const OP_ID: &str = "vyre-libs::math::fft::fft4_complex";

/// Build a Program that computes a 4-point complex DFT.
/// `input` is a length-8 F32 buffer holding 4 complex values as
/// `[re0, im0, re1, im1, re2, im2, re3, im3]`. `output` has the
/// same shape and holds the 4 frequency bins in the same layout.
#[must_use]
pub fn fft4_complex(input: &str, output: &str) -> Program {
    let body = vec![
        Node::let_bind("x0r", Expr::load(input, Expr::u32(0))),
        Node::let_bind("x0i", Expr::load(input, Expr::u32(1))),
        Node::let_bind("x1r", Expr::load(input, Expr::u32(2))),
        Node::let_bind("x1i", Expr::load(input, Expr::u32(3))),
        Node::let_bind("x2r", Expr::load(input, Expr::u32(4))),
        Node::let_bind("x2i", Expr::load(input, Expr::u32(5))),
        Node::let_bind("x3r", Expr::load(input, Expr::u32(6))),
        Node::let_bind("x3i", Expr::load(input, Expr::u32(7))),
        // X[0] = x0 + x1 + x2 + x3
        Node::Store {
            buffer: output.into(),
            index: Expr::u32(0),
            value: Expr::add(
                Expr::add(Expr::var("x0r"), Expr::var("x1r")),
                Expr::add(Expr::var("x2r"), Expr::var("x3r")),
            ),
        },
        Node::Store {
            buffer: output.into(),
            index: Expr::u32(1),
            value: Expr::add(
                Expr::add(Expr::var("x0i"), Expr::var("x1i")),
                Expr::add(Expr::var("x2i"), Expr::var("x3i")),
            ),
        },
        // X[1] = x0 + (-i)x1 + (-1)x2 + (i)x3
        // Re: x0r + x1i - x2r - x3i
        // Im: x0i - x1r - x2i + x3r
        Node::Store {
            buffer: output.into(),
            index: Expr::u32(2),
            value: Expr::sub(
                Expr::sub(
                    Expr::add(Expr::var("x0r"), Expr::var("x1i")),
                    Expr::var("x2r"),
                ),
                Expr::var("x3i"),
            ),
        },
        Node::Store {
            buffer: output.into(),
            index: Expr::u32(3),
            value: Expr::add(
                Expr::sub(
                    Expr::sub(Expr::var("x0i"), Expr::var("x1r")),
                    Expr::var("x2i"),
                ),
                Expr::var("x3r"),
            ),
        },
        // X[2] = x0 - x1 + x2 - x3
        Node::Store {
            buffer: output.into(),
            index: Expr::u32(4),
            value: Expr::add(
                Expr::sub(Expr::var("x0r"), Expr::var("x1r")),
                Expr::sub(Expr::var("x2r"), Expr::var("x3r")),
            ),
        },
        Node::Store {
            buffer: output.into(),
            index: Expr::u32(5),
            value: Expr::add(
                Expr::sub(Expr::var("x0i"), Expr::var("x1i")),
                Expr::sub(Expr::var("x2i"), Expr::var("x3i")),
            ),
        },
        // X[3] = x0 + (i)x1 + (-1)x2 + (-i)x3
        // Re: x0r - x1i - x2r + x3i
        // Im: x0i + x1r - x2i - x3r
        Node::Store {
            buffer: output.into(),
            index: Expr::u32(6),
            value: Expr::add(
                Expr::sub(
                    Expr::sub(Expr::var("x0r"), Expr::var("x1i")),
                    Expr::var("x2r"),
                ),
                Expr::var("x3i"),
            ),
        },
        Node::Store {
            buffer: output.into(),
            index: Expr::u32(7),
            value: Expr::sub(
                Expr::sub(
                    Expr::add(Expr::var("x0i"), Expr::var("x1r")),
                    Expr::var("x2i"),
                ),
                Expr::var("x3r"),
            ),
        },
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(8),
            BufferDecl::output(output, 1, DataType::F32).with_count(8),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || fft4_complex("input", "output"),
        test_inputs: Some(|| {
            // Real-valued sequence [1, 0, 0, 0] (impulse): all bins = 1+0i
            let input = crate::test_support::byte_pack::f32_bytes(&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
            vec![vec![input]]
        }),
        expected_output: Some(|| {
            // FFT of impulse = uniform [1, 1, 1, 1] across all bins.
            vec![vec![crate::test_support::byte_pack::f32_bytes(&[1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0])]]
        }),
        category: Some("math"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::f32_bytes;
    use vyre_reference::value::Value;

    fn decode(bytes: &[u8]) -> Vec<f32> {
        bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
            .collect()
    }

    fn naive_dft4(input: &[f32]) -> Vec<f32> {
        // Reference DFT for N=4 (interleaved re/im).
        let n = 4usize;
        let mut out = vec![0.0f32; 8];
        for k in 0..n {
            let mut re = 0.0_f32;
            let mut im = 0.0_f32;
            for nn in 0..n {
                let xr = input[2 * nn];
                let xi = input[2 * nn + 1];
                let theta = -2.0 * std::f32::consts::PI * (nn as f32) * (k as f32) / (n as f32);
                let cos_t = theta.cos();
                let sin_t = theta.sin();
                re += xr * cos_t - xi * sin_t;
                im += xr * sin_t + xi * cos_t;
            }
            out[2 * k] = re;
            out[2 * k + 1] = im;
        }
        out
    }

    fn run(input: &[f32]) -> Vec<f32> {
        let prog = fft4_complex("input", "output");
        let outputs = vyre_reference::reference_eval(
            &prog,
            &[Value::from(f32_bytes(input)), Value::from(vec![0u8; 32])],
        )
        .expect("Fix: fft4_complex must execute in the reference interpreter.");
        decode(&outputs[0].to_bytes())
    }

    /// Impulse response: FFT of [1, 0, 0, 0] is [1, 1, 1, 1] across
    /// all bins (each bin sums one term, x[0] = 1).
    #[test]
    fn fft4_impulse_yields_uniform_bins() {
        let input = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let actual = run(&input);
        let expected = [1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0];
        for (a, e) in actual.iter().zip(expected.iter()) {
            assert!((a - e).abs() <= 1.0e-5, "{a} != {e}");
        }
    }

    /// DC signal: FFT of [1, 1, 1, 1] is [4, 0, 0, 0] (all energy
    /// in the DC bin).
    #[test]
    fn fft4_dc_signal_concentrates_in_dc_bin() {
        let input = [1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0];
        let actual = run(&input);
        let expected = [4.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        for (a, e) in actual.iter().zip(expected.iter()) {
            assert!((a - e).abs() <= 1.0e-5, "{a} != {e}");
        }
    }

    /// Frequency bin 1: FFT of [cos(2π·n/4)] for n=0..3 (real-axis
    /// alternating cosine) puts energy in bin 1 (and its conjugate
    /// bin 3 by Hermitian symmetry).
    #[test]
    fn fft4_freq1_cosine_concentrates_in_bins_1_and_3() {
        // cos(2π·n/4) for n = 0, 1, 2, 3 = [1, 0, -1, 0]
        let input = [1.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0];
        let actual = run(&input);
        // X[0] = 0, X[1] = 2, X[2] = 0, X[3] = 2 (real-only output
        // because the input is real and even-symmetric).
        let expected = [0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 2.0, 0.0];
        for (a, e) in actual.iter().zip(expected.iter()) {
            assert!((a - e).abs() <= 1.0e-5, "{a} != {e}");
        }
    }

    /// Random fuzz: 50 random length-4 complex sequences, agree
    /// with the naive DFT formula within 1.0e-4 absolute tolerance.
    #[test]
    fn fft4_matches_naive_dft_on_random_fuzz() {
        let mut state = 0xCAFEBABE_u64;
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 33) as f32 / (u32::MAX as f32 / 2.0)) - 1.0
        };
        for _ in 0..50 {
            let input: Vec<f32> = (0..8).map(|_| next()).collect();
            let actual = run(&input);
            let expected = naive_dft4(&input);
            for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
                assert!(
                    (a - e).abs() <= 1.0e-4,
                    "lane {i}: fft={a} naive={e} diff={}",
                    (a - e).abs()
                );
            }
        }
    }

    // ------------------------------------------------------------------
    // Adversarial fixtures exposing real gaps
    // ------------------------------------------------------------------

    /// NaN in the real part of x[0] propagates to the real part of every
    /// output bin (the imaginary parts remain finite because x0i=0).
    #[test]
    fn fft4_nan_input_propagates_to_real_parts() {
        let input = [f32::NAN, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let actual = run(&input);
        for k in 0..4 {
            assert!(
                actual[2 * k].is_nan(),
                "FFT bin {k} real part must be NaN when x0r is NaN"
            );
        }
    }

    /// NaN in both re and im of x[0] must make every output component NaN.
    #[test]
    fn fft4_nan_both_components_propagates_everywhere() {
        let input = [f32::NAN, f32::NAN, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let actual = run(&input);
        for (i, &v) in actual.iter().enumerate() {
            assert!(
                v.is_nan(),
                "FFT lane {i} must be NaN when both re/im inputs are NaN"
            );
        }
    }

    /// Inf in the real part of x[0] propagates to the real part of every
    /// output bin.
    #[test]
    fn fft4_inf_input_propagates_to_real_parts() {
        let input = [f32::INFINITY, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let actual = run(&input);
        for k in 0..4 {
            assert!(
                actual[2 * k].is_infinite(),
                "FFT bin {k} real part must be Inf when x0r is Inf"
            );
        }
    }
}
