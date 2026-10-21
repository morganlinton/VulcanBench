//! Shared u32 bit-count unary builders.

use vyre::ir::{Expr, Program};

/// Bit-count operation over each u32 lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BitCountKind {
    /// Count leading zero bits.
    LeadingZeros,
    /// Count trailing zero bits.
    TrailingZeros,
}

impl BitCountKind {
    fn expr(self, value: Expr) -> Expr {
        match self {
            Self::LeadingZeros => Expr::clz(value),
            Self::TrailingZeros => Expr::ctz(value),
        }
    }

    #[cfg(test)]
    fn cpu(self, value: u32) -> u32 {
        match self {
            Self::LeadingZeros => value.leading_zeros(),
            Self::TrailingZeros => value.trailing_zeros(),
        }
    }
}

/// Build `out[i] = bit_count(input[i])`.
#[must_use]
pub(crate) fn bit_count_u32_program(
    op_id: &'static str,
    input: &str,
    out: &str,
    size: u32,
    kind: BitCountKind,
) -> Program {
    super::elementwise::u32_elementwise_unary(op_id, input, out, size, |value| kind.expr(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_reference::value::Value;

    fn eval(kind: BitCountKind, input: &[u32]) -> Vec<u32> {
        let program = bit_count_u32_program(
            "vyre-libs::math::bit_count_test",
            "input",
            "out",
            input.len() as u32,
            kind,
        );
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(vyre_primitives::wire::pack_u32_slice(input)),
                Value::from(vec![0_u8; input.len() * core::mem::size_of::<u32>()]),
            ],
        )
        .expect("Fix: bit-count unary program must execute in the reference interpreter.");
        vyre_primitives::wire::decode_u32_le_bytes_all(&outputs[0].to_bytes())
    }

    #[test]
    fn generated_bit_counts_match_rust_reference() {
        let mut state = 0xB17C_0001_u32;
        for case in 0..1024_u32 {
            state = state.wrapping_mul(22_695_477).wrapping_add(1);
            let len = (state as usize % 65) + 1;
            let mut input = Vec::with_capacity(len);
            for index in 0..len {
                state = state.rotate_left(3) ^ (index as u32).wrapping_mul(0x9E37_79B9);
                input.push(match index % 8 {
                    0 => 0,
                    1 => 1,
                    2 => u32::MAX,
                    3 => 0x8000_0000,
                    4 => 0x0000_0001,
                    5 => 1_u32 << (case & 31),
                    6 => !(1_u32 << ((case + index as u32) & 31)),
                    _ => state,
                });
            }

            for kind in [BitCountKind::LeadingZeros, BitCountKind::TrailingZeros] {
                let expected: Vec<u32> =
                    input.iter().copied().map(|value| kind.cpu(value)).collect();
                assert_eq!(eval(kind, &input), expected, "case {case} kind {kind:?}");
            }
        }
    }
}
