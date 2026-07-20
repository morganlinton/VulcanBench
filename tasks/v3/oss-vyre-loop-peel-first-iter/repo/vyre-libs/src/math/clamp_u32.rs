//! Cat-A `clamp_u32`  -  element-wise `x.clamp(lo, hi)`.
//!
//! Migration target per `docs/migration-vyre-ops-to-intrinsics.md`:
//! pure composition of `Expr::min` and `Expr::max` (both are existing
//! `BinOp` primitives with no dedicated target builder arm required at the op
//! level). Library, not intrinsic.
//!
//! Signature takes three buffers + one output  -  the binary helper
//! doesn't fit, so the Program is constructed inline (still wrapped in
//! a `Node::Region` per the Region chain invariant).
//!
//! CPU reference: `u32::clamp` bit-exact.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

const OP_ID: &str = "vyre-libs::math::clamp_u32";

/// Map `out[i] = input[i].clamp(lo[i], hi[i])` over n elements.
#[must_use]
pub fn clamp_u32(input: &str, lo: &str, hi: &str, out: &str, n: u32) -> Program {
    let body = vec![crate::region::wrap_anonymous(
        OP_ID,
        vec![
            Node::let_bind("idx", Expr::InvocationId { axis: 0 }),
            Node::if_then(
                Expr::lt(Expr::var("idx"), Expr::u32(n)),
                vec![Node::store(
                    out,
                    Expr::var("idx"),
                    Expr::min(
                        Expr::max(
                            Expr::load(input, Expr::var("idx")),
                            Expr::load(lo, Expr::var("idx")),
                        ),
                        Expr::load(hi, Expr::var("idx")),
                    ),
                )],
            ),
        ],
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(lo, 1, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(hi, 2, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::output(out, 3, DataType::U32).with_count(n),
        ],
        [64, 1, 1],
        body,
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || clamp_u32("input", "lo", "hi", "out", 4),
        test_inputs: Some(|| {
            let input = [0u32, 5, 10, u32::MAX];
            let lo = [3u32, 3, 3, 100];
            let hi = [8u32, 8, 8, 200];
            let to_bytes = vyre_primitives::wire::pack_u32_slice;
            vec![vec![to_bytes(&input), to_bytes(&lo), to_bytes(&hi)]]
        }),
        expected_output: Some(|| {
            // u32::clamp per-element. The 4th lane (u32::MAX) clamps
            // down to hi=200; the first three clamp up to lo=3 or
            // pass through unchanged.
            let expected = [3u32, 5, 8, 200];
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

    fn run(input: &[u32], lo: &[u32], hi: &[u32]) -> Vec<u32> {
        let n = input.len() as u32;
        let program = clamp_u32("input", "lo", "hi", "out", n.max(1));
        let to_bytes = vyre_primitives::wire::pack_u32_slice;
        let inputs = vec![
            Value::Bytes(to_bytes(input).into()),
            Value::Bytes(to_bytes(lo).into()),
            Value::Bytes(to_bytes(hi).into()),
            Value::Bytes(vec![0u8; (n.max(1) * 4) as usize].into()),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: clamp_u32 must run; restore this invariant before continuing.");
        let raw = outputs[0].to_bytes();
        vyre_primitives::wire::decode_u32_le_bytes_all(&raw)
    }

    #[test]
    fn matches_rust_ref_small() {
        let input = [0u32, 5, 10, u32::MAX];
        let lo = [3u32, 3, 3, 100];
        let hi = [8u32, 8, 8, 200];
        let got = run(&input, &lo, &hi);
        let expected: Vec<u32> = input
            .iter()
            .zip(lo.iter())
            .zip(hi.iter())
            .map(|((&x, &l), &h)| x.clamp(l, h))
            .collect();
        assert_eq!(got, expected);
    }

    #[test]
    fn passthrough_when_in_range() {
        let input = [5u32];
        let lo = [0u32];
        let hi = [10u32];
        assert_eq!(run(&input, &lo, &hi), vec![5]);
    }
}
