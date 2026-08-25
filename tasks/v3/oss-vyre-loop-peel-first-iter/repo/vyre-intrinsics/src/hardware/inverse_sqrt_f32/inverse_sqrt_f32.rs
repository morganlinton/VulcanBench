//! Cat-C `inverse_sqrt_f32`  -  finite-domain `1 / sqrt(x)` per f32 lane.
//! Inputs that are non-finite, negative, zero, or subnormal are clamped to
//! `f32::MIN_POSITIVE` before the reciprocal square root.

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::hardware::{pack_f32, MAP_WORKGROUP};

/// Build a Program that computes finite-domain `out[i] = 1.0 / sqrt(input[i])`.
#[must_use]
pub fn inverse_sqrt_f32(input: &str, out: &str, n: u32) -> Program {
    let body = vec![crate::region::wrap_anonymous(
        "vyre-intrinsics::hardware::inverse_sqrt_f32",
        vec![
            Node::let_bind("idx", Expr::InvocationId { axis: 0 }),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::buf_len(out)),
                vec![
                    Node::let_bind("x", Expr::load(input, Expr::var("idx"))),
                    Node::let_bind(
                        "safe_x",
                        Expr::select(
                            Expr::and(
                                Expr::is_finite(Expr::var("x")),
                                Expr::gt(Expr::var("x"), Expr::f32(f32::MIN_POSITIVE)),
                            ),
                            Expr::var("x"),
                            Expr::f32(f32::MIN_POSITIVE),
                        ),
                    ),
                    Node::store(
                        out,
                        Expr::var("idx"),
                        Expr::inverse_sqrt(Expr::var("safe_x")),
                    ),
                ],
            ),
        ],
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::F32).with_count(n),
            BufferDecl::output(out, 1, DataType::F32).with_count(n),
        ],
        MAP_WORKGROUP,
        body,
    )
}

fn cpu_ref(input: &[f32]) -> Vec<u8> {
    pack_f32(
        &input
            .iter()
            .map(|&x| {
                let safe_x = if x.is_finite() && x > f32::MIN_POSITIVE {
                    x
                } else {
                    f32::MIN_POSITIVE
                };
                1.0 / safe_x.sqrt()
            })
            .collect::<Vec<_>>(),
    )
}

fn test_inputs() -> Vec<Vec<Vec<u8>>> {
    let input = vec![1.0f32, 4.0, 9.0, 16.0];
    let len = input.len() * 4;
    vec![vec![pack_f32(&input), vec![0u8; len]]]
}

fn expected_output() -> Vec<Vec<Vec<u8>>> {
    let input = vec![1.0f32, 4.0, 9.0, 16.0];
    vec![vec![cpu_ref(&input)]]
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-intrinsics::hardware::inverse_sqrt_f32",
        build: || inverse_sqrt_f32("input", "out", 4),
        test_inputs: Some(test_inputs),
        expected_output: Some(expected_output),
        category: Some("hardware"),
        shape: Some(crate::harness::OpShape::new(
            1,
            1,
            4,
            crate::harness::HardwareSemantic::InverseSqrtF32,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::{lcg_f32, run_program};

    fn assert_case(input: &[f32]) {
        let n = input.len() as u32;
        let program = inverse_sqrt_f32("input", "out", n.max(1));
        let outputs = run_program(
            &program,
            vec![pack_f32(input), vec![0u8; (n.max(1) * 4) as usize]],
        );
        assert_eq!(outputs, vec![cpu_ref(input)]);
    }

    #[test]
    fn one_element() {
        assert_case(&[4.0]);
    }

    #[test]
    fn known_values() {
        assert_case(&[1.0, 4.0, 9.0, 16.0, 25.0, 100.0]);
    }

    #[test]
    fn random_sixty_four() {
        let input: Vec<f32> = lcg_f32(0x0F1A_A005, 64)
            .into_iter()
            .map(|v| v.abs() + 0.01)
            .collect();
        assert_case(&input);
    }

    #[test]
    fn clamps_non_finite_and_tiny_inputs() {
        assert_case(&[
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
            -1.0,
            0.0,
            f32::from_bits(1),
            f32::MIN_POSITIVE,
        ]);
    }
}
