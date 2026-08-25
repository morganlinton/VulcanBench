//! Recursive radix-2 Cooley-Tukey FFT for power-of-two N.
//!
//! ROADMAP H2  -  companion to `fft4_complex`. Builds an N-point
//! complex FFT by recursive bit-reversal + butterfly stages on top
//! of the verified 4-point base case.
//!
//! ## Algorithm
//!
//! 1. Bit-reverse the input order (so x`i` is loaded at index
//!    bit_reverse(i, log2(N))).
//! 2. For each stage `s ∈ 1..=log2(N)`:
//!    - sub-FFT size `m = 2^s`
//!    - twiddle `W = exp(-2πi/m)`
//!    - For each block of `m` consecutive samples:
//!      - For each pair offset `k ∈ 0..m/2`:
//!        - t = W^k · X[block + k + m/2]
//!        - X[block + k + m/2] = X[block + k] - t
//!        - X[block + k]       = X[block + k] + t
//!
//! This implementation builds the entire algorithm at IR-build
//! time (the loop bounds are baked in once N is known), so the
//! generated Program is straight-line: no host-side recursion at
//! dispatch time, no twiddle-factor table lookup overhead.
//!
//! Soundness: standard Cooley-Tukey identity. Verified via fuzz
//! against the naive O(N²) DFT formula (1.0e-3 absolute tolerance
//! for N=8 due to f32 rounding accumulating across log2(N) stages).

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use super::common::{bit_reverse, validate_complex_len};
use crate::region::wrap_anonymous;

const OP_ID: &str = "vyre-libs::math::fft::fft_radix2";

/// Build a Program that computes an N-point complex DFT via
/// recursive radix-2 Cooley-Tukey. `input` is a length-`2*N` F32
/// buffer holding interleaved (re, im) pairs; `output` has the
/// same shape.
///
/// # Errors
///
/// Returns `Err` when `n` is not a power of two, when `n < 2`, or
/// when `2 * n` overflows `u32`.
pub fn fft_radix2_complex(input: &str, output: &str, n: u32) -> Result<Program, String> {
    let elements = validate_complex_len(n, "fft_radix2_complex")?;
    let log2_n = n.trailing_zeros() as usize;

    let mut body = Vec::new();
    // Step 1: bit-reverse load. Write `output[2*i..2*i+1] = input[2*j..2*j+1]`
    // where `j = bit_reverse(i, log2_n)`.
    for i in 0..n {
        let j = bit_reverse(i, log2_n);
        body.push(Node::store(
            output,
            Expr::u32(2 * i),
            Expr::load(input, Expr::u32(2 * j)),
        ));
        body.push(Node::store(
            output,
            Expr::u32(2 * i + 1),
            Expr::load(input, Expr::u32(2 * j + 1)),
        ));
    }
    // Step 2: log2(n) butterfly stages.
    for stage in 1..=log2_n {
        let m = 1u32 << stage;
        let half_m = m / 2;
        let theta_step = -2.0_f32 * std::f32::consts::PI / (m as f32);
        for block in (0..n).step_by(m as usize) {
            for k in 0..half_m {
                // Twiddle factor W^k = (cos(k·θ), sin(k·θ)) where θ = -2π/m
                let theta = (k as f32) * theta_step;
                let w_re = theta.cos();
                let w_im = theta.sin();
                let upper = block + k;
                let lower = block + k + half_m;
                let upper_re_idx = 2 * upper;
                let upper_im_idx = 2 * upper + 1;
                let lower_re_idx = 2 * lower;
                let lower_im_idx = 2 * lower + 1;
                // Load current values into named Lets.
                body.push(Node::let_bind(
                    format!("u_re_s{stage}_b{block}_k{k}"),
                    Expr::load(output, Expr::u32(upper_re_idx)),
                ));
                body.push(Node::let_bind(
                    format!("u_im_s{stage}_b{block}_k{k}"),
                    Expr::load(output, Expr::u32(upper_im_idx)),
                ));
                body.push(Node::let_bind(
                    format!("l_re_s{stage}_b{block}_k{k}"),
                    Expr::load(output, Expr::u32(lower_re_idx)),
                ));
                body.push(Node::let_bind(
                    format!("l_im_s{stage}_b{block}_k{k}"),
                    Expr::load(output, Expr::u32(lower_im_idx)),
                ));
                // t = W * lower (complex multiplication)
                // t_re = w_re * l_re - w_im * l_im
                // t_im = w_re * l_im + w_im * l_re
                let l_re = Expr::var(format!("l_re_s{stage}_b{block}_k{k}"));
                let l_im = Expr::var(format!("l_im_s{stage}_b{block}_k{k}"));
                body.push(Node::let_bind(
                    format!("t_re_s{stage}_b{block}_k{k}"),
                    Expr::sub(
                        Expr::mul(Expr::f32(w_re), l_re.clone()),
                        Expr::mul(Expr::f32(w_im), l_im.clone()),
                    ),
                ));
                body.push(Node::let_bind(
                    format!("t_im_s{stage}_b{block}_k{k}"),
                    Expr::add(
                        Expr::mul(Expr::f32(w_re), l_im),
                        Expr::mul(Expr::f32(w_im), l_re),
                    ),
                ));
                let u_re = Expr::var(format!("u_re_s{stage}_b{block}_k{k}"));
                let u_im = Expr::var(format!("u_im_s{stage}_b{block}_k{k}"));
                let t_re = Expr::var(format!("t_re_s{stage}_b{block}_k{k}"));
                let t_im = Expr::var(format!("t_im_s{stage}_b{block}_k{k}"));
                // Store: upper = u + t; lower = u - t
                body.push(Node::store(
                    output,
                    Expr::u32(upper_re_idx),
                    Expr::add(u_re.clone(), t_re.clone()),
                ));
                body.push(Node::store(
                    output,
                    Expr::u32(upper_im_idx),
                    Expr::add(u_im.clone(), t_im.clone()),
                ));
                body.push(Node::store(
                    output,
                    Expr::u32(lower_re_idx),
                    Expr::sub(u_re, t_re),
                ));
                body.push(Node::store(
                    output,
                    Expr::u32(lower_im_idx),
                    Expr::sub(u_im, t_im),
                ));
            }
        }
    }
    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32)
                .with_count(elements),
            BufferDecl::output(output, 1, DataType::F32).with_count(elements),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
    .with_entry_op_id(OP_ID))
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || fft_radix2_complex("input", "output", 4)
            .unwrap_or_else(|_| unreachable!("Fix: catalog fixture uses a valid radix-2 FFT size.")),
        test_inputs: Some(|| {
            vec![vec![
                crate::test_support::byte_pack::f32_bytes(&[1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
            ]]
        }),
        expected_output: Some(|| {
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

    fn naive_dft(input: &[f32], n: usize) -> Vec<f32> {
        let mut out = vec![0.0_f32; 2 * n];
        for k in 0..n {
            let mut re = 0.0_f32;
            let mut im = 0.0_f32;
            for nn in 0..n {
                let xr = input[2 * nn];
                let xi = input[2 * nn + 1];
                let theta = -2.0_f32 * std::f32::consts::PI * (nn as f32) * (k as f32) / (n as f32);
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

    fn run(n: u32, input: &[f32]) -> Vec<f32> {
        let prog = fft_radix2_complex("input", "output", n).expect("Fix: build");
        let outputs = vyre_reference::reference_eval(
            &prog,
            &[
                Value::from(f32_bytes(input)),
                Value::from(vec![0u8; (2 * n as usize) * 4]),
            ],
        )
        .expect("Fix: fft_radix2_complex must execute in the reference interpreter.");
        decode(&outputs[0].to_bytes())
    }

    /// N=2 FFT of [a, b] (both real) is [a+b, a-b].
    #[test]
    fn fft_radix2_n2_basic() {
        let input = [3.0, 0.0, 5.0, 0.0];
        let actual = run(2, &input);
        assert!((actual[0] - 8.0).abs() <= 1.0e-5); // X[0] re
        assert!((actual[1] - 0.0).abs() <= 1.0e-5);
        assert!((actual[2] + 2.0).abs() <= 1.0e-5); // X[1] re = 3 - 5 = -2
        assert!((actual[3] - 0.0).abs() <= 1.0e-5);
    }

    /// N=4 FFT of impulse [1, 0, 0, 0] is uniform [1, 1, 1, 1].
    #[test]
    fn fft_radix2_n4_impulse() {
        let input = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let actual = run(4, &input);
        let expected = [1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0];
        for (a, e) in actual.iter().zip(expected.iter()) {
            assert!((a - e).abs() <= 1.0e-5, "{a} != {e}");
        }
    }

    /// N=8 FFT impulse → uniform.
    #[test]
    fn fft_radix2_n8_impulse() {
        let mut input = vec![0.0_f32; 16];
        input[0] = 1.0;
        let actual = run(8, &input);
        for k in 0..8 {
            assert!((actual[2 * k] - 1.0).abs() <= 1.0e-5);
            assert!(actual[2 * k + 1].abs() <= 1.0e-5);
        }
    }

    /// N=8 fuzz vs naive DFT (1.0e-3 tolerance  -  log2(8)=3 stages
    /// of f32 rounding accumulate).
    #[test]
    fn fft_radix2_n8_matches_naive_on_random_fuzz() {
        let mut state = 0xC001CAFE_u64;
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 33) as f32 / (u32::MAX as f32 / 2.0)) - 1.0
        };
        for _ in 0..30 {
            let input: Vec<f32> = (0..16).map(|_| next()).collect();
            let actual = run(8, &input);
            let expected = naive_dft(&input, 8);
            for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
                assert!(
                    (a - e).abs() <= 1.0e-3,
                    "lane {i}: fft={a} naive={e} diff={}",
                    (a - e).abs()
                );
            }
        }
    }

    /// Rejects non-power-of-two N.
    #[test]
    fn fft_radix2_rejects_non_power_of_two() {
        let err = fft_radix2_complex("input", "output", 6).expect_err("non-pow2 n must error");
        assert!(err.contains("power of two"));
    }

    /// Rejects N < 2.
    #[test]
    fn fft_radix2_rejects_tiny_n() {
        let err0 = fft_radix2_complex("input", "output", 0).expect_err("n=0 must error");
        let err1 = fft_radix2_complex("input", "output", 1).expect_err("n=1 must error");
        assert!(
            err0.contains("power of two") || err0.contains("Fix:"),
            "n=0 fft error: {err0}"
        );
        assert!(
            err1.contains("power of two") || err1.contains("Fix:"),
            "n=1 fft error: {err1}"
        );
    }

    /// Bit-reverse helper sanity check.
    #[test]
    fn bit_reverse_helper() {
        // bit_reverse(1, 3) = 100_b = 4
        assert_eq!(bit_reverse(1, 3), 4);
        // bit_reverse(3, 3) = 110_b = 6
        assert_eq!(bit_reverse(3, 3), 6);
        // bit_reverse(0, 3) = 0
        assert_eq!(bit_reverse(0, 3), 0);
        // bit_reverse(7, 3) = 7
        assert_eq!(bit_reverse(7, 3), 7);
    }

    // ------------------------------------------------------------------
    // Adversarial fixtures exposing real gaps
    // ------------------------------------------------------------------

    /// NaN in the real part of input[0] propagates to the real part of every
    /// output bin (imaginary parts remain finite because input[1]=0).
    #[test]
    fn fft_radix2_nan_input_propagates_to_real_parts() {
        let mut input = vec![0.0_f32; 16];
        input[0] = f32::NAN;
        let actual = run(8, &input);
        for k in 0..8 {
            assert!(
                actual[2 * k].is_nan(),
                "radix-2 FFT bin {k} real part must be NaN when input[0] is NaN"
            );
        }
    }

    /// NaN in both re and im of input[0] must make every output component NaN.
    #[test]
    fn fft_radix2_nan_both_components_propagates_everywhere() {
        let mut input = vec![0.0_f32; 16];
        input[0] = f32::NAN;
        input[1] = f32::NAN;
        let actual = run(8, &input);
        for (i, &v) in actual.iter().enumerate() {
            assert!(
                v.is_nan(),
                "radix-2 FFT lane {i} must be NaN when both re/im inputs are NaN"
            );
        }
    }

    /// Inf in the real part of input[0] propagates to the real part of every
    /// output bin.
    #[test]
    fn fft_radix2_inf_input_propagates_to_real_parts() {
        let mut input = vec![0.0_f32; 16];
        input[0] = f32::INFINITY;
        let actual = run(8, &input);
        for k in 0..8 {
            assert!(
                actual[2 * k].is_infinite(),
                "radix-2 FFT bin {k} real part must be Inf when input[0] is Inf"
            );
        }
    }
}
