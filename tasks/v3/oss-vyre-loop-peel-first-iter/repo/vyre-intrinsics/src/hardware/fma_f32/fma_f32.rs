//! Cat-C `fma_f32`  -  fused multiply-add per f32 lane.
//! CPU reference: `f32::mul_add` BYTE-IDENTICAL (never multiply-then-add).
//!
//! Round-mode guarantee: this op promises IEEE-754 single-round fused
//! semantics, matching `f32::mul_add` bit-for-bit. A backend that cannot emit
//! a true fused instruction must report `UnsupportedByBackend`; it must NOT
//! silently degrade to `a * b + c`, which double-rounds and changes results.
//! Callers that explicitly want multiply-then-add semantics must build that as
//! a different Program and accept the different rounding contract.

use vyre_foundation::ir::Program;

use crate::hardware::{pack_f32, ternary_f32_program};

/// Map `out[i] = fma(a[i], b[i], c[i])` over n elements.
///
/// # FMA capability and round-mode guarantee
///
/// This op requires the backend to advertise the `FMA` capability.  If the
/// backend reports `FMA` as absent, lowering **must** emit a clear
/// `BackendError::Unsupported`  -  it must
/// **never** silently fall back to `a * b + c`, because IEEE-754 multiply-then-add
/// double-rounds and produces a different result from single-round fused
/// multiply-add.  Callers that want the weaker `a * b + c` contract must build
/// that expression explicitly and accept the rounding divergence.
#[must_use]
pub fn fma_f32(a: &str, b: &str, c: &str, out: &str, n: u32) -> Program {
    ternary_f32_program(a, b, c, out, n)
}

fn cpu_ref(a: &[f32], b: &[f32], c: &[f32]) -> Vec<u8> {
    pack_f32(
        &a.iter()
            .zip(b.iter())
            .zip(c.iter())
            .map(|((&x, &y), &z)| x.mul_add(y, z))
            .collect::<Vec<_>>(),
    )
}

fn test_inputs() -> Vec<Vec<Vec<u8>>> {
    let a = vec![0.0f32, 1.0, -2.5, f32::MAX];
    let b = vec![1.0f32, -3.0, 4.0, 0.5];
    let c = vec![0.0f32, 0.25, -1.0, 2.0];
    let len = a.len() * 4;
    vec![vec![
        pack_f32(&a),
        pack_f32(&b),
        pack_f32(&c),
        vec![0u8; len],
    ]]
}

fn expected_output() -> Vec<Vec<Vec<u8>>> {
    let a = vec![0.0f32, 1.0, -2.5, f32::MAX];
    let b = vec![1.0f32, -3.0, 4.0, 0.5];
    let c = vec![0.0f32, 0.25, -1.0, 2.0];
    vec![vec![cpu_ref(&a, &b, &c)]]
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-intrinsics::hardware::fma_f32",
        build: || fma_f32("a", "b", "c", "out", 4),
        test_inputs: Some(test_inputs),
        expected_output: Some(expected_output),
        category: Some("hardware"),
        shape: Some(crate::harness::OpShape::new(
            3,
            1,
            4,
            crate::harness::HardwareSemantic::FmaF32,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::{lcg_f32, run_program};

    fn assert_case(a: &[f32], b: &[f32], c: &[f32]) {
        let n = a.len() as u32;
        let program = fma_f32("a", "b", "c", "out", n.max(1));
        let outputs = run_program(
            &program,
            vec![
                pack_f32(a),
                pack_f32(b),
                pack_f32(c),
                vec![0u8; (n.max(1) * 4) as usize],
            ],
        );
        assert_eq!(outputs, vec![cpu_ref(a, b, c)]);
    }

    #[test]
    fn one_element() {
        assert_case(&[1.5], &[2.0], &[0.25]);
    }

    #[test]
    fn max_value() {
        assert_case(&[f32::MAX], &[1.0], &[0.0]);
    }

    #[test]
    fn random_sixty_four() {
        let a = lcg_f32(0x0F1A_A001, 64);
        let b = lcg_f32(0x0F1A_A002, 64);
        let c = lcg_f32(0x0F1A_A003, 64);
        assert_case(&a, &b, &c);
    }
}
