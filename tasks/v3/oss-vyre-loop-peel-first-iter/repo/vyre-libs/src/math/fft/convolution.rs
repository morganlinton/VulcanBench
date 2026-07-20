//! FFT-based circular convolution for complex F32 vectors.
//!
//! This is the H2 composition layer over the verified radix-2 FFT:
//! `FFT(a)`, `FFT(b)`, pointwise complex multiply, then inverse FFT
//! via `conj(FFT(conj(x))) / n`. Inputs and output are interleaved
//! complex buffers `[re0, im0, re1, im1, ...]` with length `2 * n`.

use std::sync::Arc;

use vyre::ir::model::expr::GeneratorRef;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Ident, Node, Program};

use super::common::validate_complex_len;
use super::fft_radix2_complex;
use crate::region::wrap_anonymous;

const OP_ID: &str = "vyre-libs::math::fft::fft_convolve_circular_complex";
const FFT_OP_ID: &str = "vyre-libs::math::fft::fft_radix2";
const MULTIPLY_OP_ID: &str = "vyre-libs::math::fft::pointwise_complex_multiply_conjugate";
const SCALE_OP_ID: &str = "vyre-libs::math::fft::scale_conjugate_inverse";

/// Build a Program that computes the length-`n` circular convolution
/// of two interleaved complex F32 buffers using FFTs.
///
/// `signal_freq`, `kernel_freq`, and `product_freq` are explicit
/// workgroup scratch buffers of length `2 * n`. Keeping scratch in
/// the Program's buffer table makes the memory footprint visible to
/// validation and the megakernel planner.
///
/// # Errors
///
/// Returns `Err` when `n` is not a power of two, `n < 2`, `2 * n`
/// overflows `u32`, or any buffer names alias.
pub fn fft_convolve_circular_complex(
    signal: &str,
    kernel: &str,
    signal_freq: &str,
    kernel_freq: &str,
    product_freq: &str,
    output: &str,
    n: u32,
) -> Result<Program, String> {
    validate_names(&[
        signal,
        kernel,
        signal_freq,
        kernel_freq,
        product_freq,
        output,
    ])?;
    let elements = validate_complex_len(n, "fft_convolve_circular_complex")?;

    // Each FFT body is a registered composition boundary. The Region keeps
    // its Let bindings scoped while letting Gate 1 and print-composition see
    // this as reuse of `fft_radix2`, not inline FFT reinvention.
    let mut entry = Vec::new();
    entry.push(fft_region(signal, signal_freq, n)?);
    entry.push(fft_region(kernel, kernel_freq, n)?);
    entry.push(Node::Region {
        generator: Ident::from(MULTIPLY_OP_ID),
        // source_region: Some(...) marks this as a child composition so
        // the structural-budget gate stops counting nodes once it
        // descends below this Region. The `multiply + conjugate` step
        // is a self-contained sub-op of the FFT-convolve composition.
        source_region: Some(GeneratorRef {
            name: MULTIPLY_OP_ID.to_string(),
        }),
        body: Arc::new(multiply_and_conjugate_body(
            signal_freq,
            kernel_freq,
            product_freq,
            n,
        )),
    });
    entry.push(fft_region(product_freq, output, n)?);
    entry.push(Node::Region {
        generator: Ident::from(SCALE_OP_ID),
        source_region: Some(GeneratorRef {
            name: SCALE_OP_ID.to_string(),
        }),
        body: Arc::new(scale_conjugate_body(output, n)),
    });

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(signal, 0, BufferAccess::ReadOnly, DataType::F32)
                .with_count(elements),
            BufferDecl::storage(kernel, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(elements),
            BufferDecl::workgroup(signal_freq, elements, DataType::F32),
            BufferDecl::workgroup(kernel_freq, elements, DataType::F32),
            BufferDecl::workgroup(product_freq, elements, DataType::F32),
            BufferDecl::output(output, 2, DataType::F32).with_count(elements),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(OP_ID, entry)],
    )
    .with_entry_op_id(OP_ID))
}

fn validate_names(names: &[&str]) -> Result<(), String> {
    for (idx, name) in names.iter().enumerate() {
        if name.is_empty() {
            return Err(format!(
                "Fix: buffer name at position {idx} must not be empty."
            ));
        }
        if names[..idx].iter().any(|seen| seen == name) {
            return Err(format!(
                "Fix: fft_convolve_circular_complex requires distinct buffer names; `{name}` is reused."
            ));
        }
    }
    Ok(())
}

fn fft_region(input: &str, output: &str, n: u32) -> Result<Node, String> {
    Ok(Node::Region {
        generator: Ident::from(FFT_OP_ID),
        source_region: Some(GeneratorRef {
            name: FFT_OP_ID.to_string(),
        }),
        body: Arc::new(fft_radix2_complex(input, output, n)?.into_entry_vec()),
    })
}

fn multiply_and_conjugate_body(
    signal_freq: &str,
    kernel_freq: &str,
    product_freq: &str,
    n: u32,
) -> Vec<Node> {
    let mut body = Vec::with_capacity(n as usize * 10);
    for k in 0..n {
        let base = 2 * k;
        let ar_name = format!("a_re_{k}");
        let ai_name = format!("a_im_{k}");
        let br_name = format!("b_re_{k}");
        let bi_name = format!("b_im_{k}");
        let prod_re_name = format!("prod_re_{k}");
        let prod_im_name = format!("prod_im_{k}");
        body.push(Node::let_bind(
            ar_name.clone(),
            Expr::load(signal_freq, Expr::u32(base)),
        ));
        body.push(Node::let_bind(
            ai_name.clone(),
            Expr::load(signal_freq, Expr::u32(base + 1)),
        ));
        body.push(Node::let_bind(
            br_name.clone(),
            Expr::load(kernel_freq, Expr::u32(base)),
        ));
        body.push(Node::let_bind(
            bi_name.clone(),
            Expr::load(kernel_freq, Expr::u32(base + 1)),
        ));
        let ar = Expr::var(ar_name);
        let ai = Expr::var(ai_name);
        let br = Expr::var(br_name);
        let bi = Expr::var(bi_name);
        body.push(Node::let_bind(
            prod_re_name.clone(),
            Expr::sub(
                Expr::mul(ar.clone(), br.clone()),
                Expr::mul(ai.clone(), bi.clone()),
            ),
        ));
        body.push(Node::let_bind(
            prod_im_name.clone(),
            Expr::add(Expr::mul(ar, bi), Expr::mul(ai, br)),
        ));
        body.push(Node::store(
            product_freq,
            Expr::u32(base),
            Expr::var(prod_re_name),
        ));
        body.push(Node::store(
            product_freq,
            Expr::u32(base + 1),
            Expr::negate(Expr::var(prod_im_name)),
        ));
    }
    body
}

fn scale_conjugate_body(output: &str, n: u32) -> Vec<Node> {
    scale_conjugate_body_from(output, output, n)
}

fn scale_conjugate_body_from(input: &str, output: &str, n: u32) -> Vec<Node> {
    let inv_n = Expr::f32(1.0 / n as f32);
    let zero_epsilon = Expr::f32(1.0e-6);
    let mut body = Vec::with_capacity(n as usize * 6);
    for k in 0..n {
        let base = 2 * k;
        let re_name = format!("ifft_re_{k}");
        let im_name = format!("ifft_im_{k}");
        let scaled_re_name = format!("ifft_scaled_re_{k}");
        let scaled_im_name = format!("ifft_scaled_im_{k}");
        body.push(Node::let_bind(
            re_name.clone(),
            Expr::load(input, Expr::u32(base)),
        ));
        body.push(Node::let_bind(
            im_name.clone(),
            Expr::load(input, Expr::u32(base + 1)),
        ));
        body.push(Node::let_bind(
            scaled_re_name.clone(),
            Expr::mul(Expr::var(re_name), inv_n.clone()),
        ));
        body.push(Node::let_bind(
            scaled_im_name.clone(),
            Expr::mul(Expr::negate(Expr::var(im_name)), inv_n.clone()),
        ));
        body.push(Node::store(
            output,
            Expr::u32(base),
            Expr::select(
                Expr::lt(
                    Expr::abs(Expr::var(scaled_re_name.clone())),
                    zero_epsilon.clone(),
                ),
                Expr::f32(0.0),
                Expr::var(scaled_re_name),
            ),
        ));
        body.push(Node::store(
            output,
            Expr::u32(base + 1),
            Expr::select(
                Expr::lt(
                    Expr::abs(Expr::var(scaled_im_name.clone())),
                    zero_epsilon.clone(),
                ),
                Expr::f32(0.0),
                Expr::var(scaled_im_name),
            ),
        ));
    }
    body
}

fn pointwise_complex_multiply_conjugate_program() -> Program {
    let n = 4;
    let elements = n * 2;
    Program::wrapped(
        vec![
            BufferDecl::storage("signal_freq", 0, BufferAccess::ReadOnly, DataType::F32)
                .with_count(elements),
            BufferDecl::storage("kernel_freq", 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(elements),
            BufferDecl::output("product_freq", 2, DataType::F32).with_count(elements),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(
            MULTIPLY_OP_ID,
            multiply_and_conjugate_body("signal_freq", "kernel_freq", "product_freq", n),
        )],
    )
    .with_entry_op_id(MULTIPLY_OP_ID)
}

fn scale_conjugate_inverse_program() -> Program {
    let n = 4;
    let elements = n * 2;
    Program::wrapped(
        vec![
            BufferDecl::storage("product_freq", 0, BufferAccess::ReadOnly, DataType::F32)
                .with_count(elements),
            BufferDecl::output("output", 1, DataType::F32).with_count(elements),
        ],
        [1, 1, 1],
        vec![wrap_anonymous(
            SCALE_OP_ID,
            scale_conjugate_body_from("product_freq", "output", n),
        )],
    )
    .with_entry_op_id(SCALE_OP_ID)
}

fn fixture_f32_bytes(values: &[f32]) -> Vec<u8> {
    vyre_primitives::wire::pack_f32_slice(values)
}

fn pointwise_complex_multiply_conjugate_inputs() -> Vec<Vec<Vec<u8>>> {
    vec![vec![
        fixture_f32_bytes(&[1.0, 2.0, 3.0, 4.0, -1.0, 0.5, 0.0, -2.0]),
        fixture_f32_bytes(&[5.0, 6.0, -2.0, 1.0, 0.5, -1.0, 3.0, 0.0]),
    ]]
}

fn pointwise_complex_multiply_conjugate_expected() -> Vec<Vec<Vec<u8>>> {
    vec![vec![fixture_f32_bytes(&[
        -7.0, -16.0, -10.0, 5.0, 0.0, -1.25, 0.0, 6.0,
    ])]]
}

fn scale_conjugate_inverse_inputs() -> Vec<Vec<Vec<u8>>> {
    vec![vec![fixture_f32_bytes(&[
        4.0, -8.0, -12.0, 16.0, 0.0, 0.0, 2.0, -4.0,
    ])]]
}

fn scale_conjugate_inverse_expected() -> Vec<Vec<Vec<u8>>> {
    vec![vec![fixture_f32_bytes(&[
        1.0, 2.0, -3.0, -4.0, 0.0, 0.0, 0.5, 1.0,
    ])]]
}

inventory::submit! {
    crate::harness::OpEntry {
        id: MULTIPLY_OP_ID,
        build: pointwise_complex_multiply_conjugate_program,
        test_inputs: Some(pointwise_complex_multiply_conjugate_inputs),
        expected_output: Some(pointwise_complex_multiply_conjugate_expected),
        category: Some("math"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: SCALE_OP_ID,
        build: scale_conjugate_inverse_program,
        test_inputs: Some(scale_conjugate_inverse_inputs),
        expected_output: Some(scale_conjugate_inverse_expected),
        category: Some("math"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || fft_convolve_circular_complex(
            "signal",
            "kernel",
            "signal_freq",
            "kernel_freq",
            "product_freq",
            "output",
            4,
        ).unwrap_or_else(|_| unreachable!("Fix: catalog fixture uses valid power-of-two buffers.")),
        test_inputs: Some(|| {
            vec![vec![
                crate::test_support::byte_pack::f32_bytes(&[1.0, 0.0, 2.0, 0.0, 3.0, 0.0, 4.0, 0.0]),
                crate::test_support::byte_pack::f32_bytes(&[1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
            ]]
        }),
        expected_output: Some(|| {
            vec![vec![crate::test_support::byte_pack::f32_bytes(&[5.0, 0.0, 3.0, 0.0, 5.0, 0.0, 7.0, 0.0])]]
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

    fn run(signal: &[f32], kernel: &[f32], n: u32) -> Vec<f32> {
        let prog = fft_convolve_circular_complex(
            "signal",
            "kernel",
            "signal_freq",
            "kernel_freq",
            "product_freq",
            "output",
            n,
        )
        .expect("Fix: build");
        let byte_len = (2 * n as usize) * 4;
        let outputs = vyre_reference::reference_eval(
            &prog,
            &[
                Value::from(f32_bytes(signal)),
                Value::from(f32_bytes(kernel)),
                Value::from(vec![0u8; byte_len]),
            ],
        )
        .expect("Fix: fft_convolve_circular_complex must execute in the reference interpreter.");
        decode(
            &outputs
                .last()
                .expect("Fix: output buffer must be returned after scratch buffers.")
                .to_bytes(),
        )
    }

    fn naive_circular_complex(signal: &[f32], kernel: &[f32], n: usize) -> Vec<f32> {
        let mut out = vec![0.0_f32; 2 * n];
        for k in 0..n {
            let mut re = 0.0_f32;
            let mut im = 0.0_f32;
            for j in 0..n {
                let rhs = (k + n - j) % n;
                let ar = signal[2 * j];
                let ai = signal[2 * j + 1];
                let br = kernel[2 * rhs];
                let bi = kernel[2 * rhs + 1];
                re += ar * br - ai * bi;
                im += ar * bi + ai * br;
            }
            out[2 * k] = re;
            out[2 * k + 1] = im;
        }
        out
    }

    #[test]
    fn fft_convolve_circular_real_fixture_matches_reference() {
        let signal = [1.0, 0.0, 2.0, 0.0, 3.0, 0.0, 4.0, 0.0];
        let kernel = [1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let actual = run(&signal, &kernel, 4);
        let expected = naive_circular_complex(&signal, &kernel, 4);
        for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
            assert!((a - e).abs() <= 1.0e-4, "lane {i}: {a} != {e}");
        }
    }

    #[test]
    fn fft_convolve_circular_complex_fixture_matches_reference() {
        let signal = [1.0, 1.0, 0.0, -1.0, 2.0, 0.5, -3.0, 0.25];
        let kernel = [0.5, -1.0, 2.0, 0.0, -1.0, 0.5, 0.25, 1.0];
        let actual = run(&signal, &kernel, 4);
        let expected = naive_circular_complex(&signal, &kernel, 4);
        for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
            assert!((a - e).abs() <= 1.0e-4, "lane {i}: {a} != {e}");
        }
    }

    #[test]
    fn fft_convolve_rejects_non_power_of_two() {
        let err = fft_convolve_circular_complex("a", "b", "af", "bf", "pf", "out", 6)
            .expect_err("non-power-of-two must fail");
        assert!(err.contains("power of two"));
    }

    #[test]
    fn fft_convolve_rejects_aliasing_buffers() {
        let err = fft_convolve_circular_complex("a", "b", "af", "bf", "af", "out", 4)
            .expect_err("duplicate scratch name must fail");
        assert!(err.contains("distinct buffer names"));
    }

    // ------------------------------------------------------------------
    // Adversarial fixtures exposing real gaps
    // ------------------------------------------------------------------

    /// NaN in the signal must propagate to the convolution output.
    #[test]
    fn fft_convolve_nan_input_propagates() {
        let mut signal = vec![0.0_f32; 8];
        signal[0] = f32::NAN;
        let kernel = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let actual = run(&signal, &kernel, 4);
        assert!(
            actual.iter().any(|&v| v.is_nan()),
            "convolution with NaN signal must produce at least one NaN component"
        );
    }

    /// Inf in the signal propagates, but because the complex multiply
    /// includes `Inf * 0 = NaN` in the imaginary part, the output becomes
    /// NaN rather than pure Inf. This is IEEE-754 correct but adversarial.
    #[test]
    fn fft_convolve_inf_input_produces_nan_or_inf() {
        let mut signal = vec![0.0_f32; 8];
        signal[0] = f32::INFINITY;
        let kernel = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let actual = run(&signal, &kernel, 4);
        assert!(
            actual.iter().any(|&v| v.is_nan() || v.is_infinite()),
            "convolution with Inf signal must produce NaN or Inf; got {:?}",
            actual
        );
    }
}
