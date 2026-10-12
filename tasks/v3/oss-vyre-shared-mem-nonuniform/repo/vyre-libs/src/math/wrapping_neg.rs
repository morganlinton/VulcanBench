use vyre::ir::{Expr, Program};

const OP_ID: &str = "vyre-libs::math::wrapping_neg";

/// Computes wrapping negation.
#[must_use]
pub fn wrapping_neg(a: &str, out: &str, size: u32) -> Program {
    super::elementwise::u32_elementwise_unary(OP_ID, a, out, size, |value| {
        Expr::sub(Expr::u32(0), value)
    })
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || wrapping_neg("a", "out", 4),
        test_inputs: Some(|| {
            let a = [0u32, 1, u32::MAX, 42];
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![to_bytes(&a)]]
        }),
        expected_output: Some(|| {
            let expected = [
                0u32.wrapping_neg(),
                1u32.wrapping_neg(),
                u32::MAX.wrapping_neg(),
                42u32.wrapping_neg(),
            ];
            let bytes = vyre_primitives::wire::pack_u32_slice(&expected);
            vec![vec![bytes]]
        }),
        category: Some("math"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_reference::value::Value;

    fn run(input: &[u32]) -> Vec<u32> {
        let n = input.len() as u32;
        let program = wrapping_neg("input", "out", n.max(1));
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(vyre_primitives::wire::pack_u32_slice(input)),
                Value::from(vec![0u8; (n.max(1) * 4) as usize]),
            ],
        )
        .expect("Fix: wrapping_neg must execute in the reference interpreter.");
        vyre_primitives::wire::decode_u32_le_bytes_all(&outputs[0].to_bytes())
    }

    #[test]
    fn generated_wrapping_neg_matches_rust_reference() {
        let mut state = 0x6E67_A71E_u32;
        for case in 0..2048u32 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let len = (state % 65 + 1) as usize;
            let mut input = Vec::with_capacity(len);
            for lane in 0..len {
                state = state.rotate_left(5) ^ (lane as u32).wrapping_mul(0x9E37_79B9);
                input.push(match lane % 8 {
                    0 => 0,
                    1 => 1,
                    2 => u32::MAX,
                    3 => i32::MIN as u32,
                    4 => i32::MAX as u32,
                    _ => state,
                });
            }

            let expected = input
                .iter()
                .copied()
                .map(u32::wrapping_neg)
                .collect::<Vec<_>>();
            assert_eq!(run(&input), expected, "generated wrapping-neg case {case}");
        }
    }
}
