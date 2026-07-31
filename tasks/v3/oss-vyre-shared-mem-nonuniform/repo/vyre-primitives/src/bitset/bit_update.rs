//! Shared scalar bitset update builder.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};

/// Scalar bit update operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BitUpdateKind {
    /// Set `target[bit_idx]`.
    Set,
    /// Clear `target[bit_idx]`.
    Clear,
}

/// Build an in-place scalar bit update.
#[must_use]
pub(crate) fn bit_update_program(
    op_id: &'static str,
    kind: BitUpdateKind,
    target: &str,
    bit_idx: u32,
    words: u32,
) -> Program {
    let word = bit_idx / 32;
    let bit = bit_idx % 32;
    let mask = Expr::shl(Expr::u32(1), Expr::u32(bit));
    let old = Expr::load(target, Expr::u32(word));
    let value = match kind {
        BitUpdateKind::Set => Expr::bitor(old, mask),
        BitUpdateKind::Clear => Expr::bitand(
            old,
            Expr::UnOp {
                op: UnOp::BitNot,
                operand: Box::new(mask),
            },
        ),
    };
    let body = vec![Node::if_then(
        Expr::lt(Expr::u32(word), Expr::u32(words)),
        vec![Node::store(target, Expr::u32(word), value)],
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage(target, 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU scalar bit update.
#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) fn cpu_ref(target: &mut [u32], bit_idx: u32, kind: BitUpdateKind) {
    let word = (bit_idx / 32) as usize;
    let bit = bit_idx % 32;
    if let Some(slot) = target.get_mut(word) {
        match kind {
            BitUpdateKind::Set => *slot |= 1u32 << bit,
            BitUpdateKind::Clear => *slot &= !(1u32 << bit),
        }
    }
}

macro_rules! define_bit_update_op {
    (
        op_id: $op_id:expr,
        fn_name: $fn_name:ident,
        kind: $kind:ident,
        inventory_input: $inventory_input:expr,
        inventory_expected: $inventory_expected:expr
    ) => {
        /// Canonical op id.
        pub const OP_ID: &str = $op_id;

        /// Build a scalar in-place bit-update Program.
        #[must_use]
        pub fn $fn_name(target: &str, bit_idx: u32, words: u32) -> vyre_foundation::ir::Program {
            crate::bitset::bit_update::bit_update_program(
                OP_ID,
                crate::bitset::bit_update::BitUpdateKind::$kind,
                target,
                bit_idx,
                words,
            )
        }

        /// CPU reference. Mutates `target` in place.
        #[cfg(any(test, feature = "cpu-parity"))]
        pub fn cpu_ref(target: &mut [u32], bit_idx: u32) {
            crate::bitset::bit_update::cpu_ref(
                target,
                bit_idx,
                crate::bitset::bit_update::BitUpdateKind::$kind,
            );
        }

        #[cfg(feature = "inventory-registry")]
        inventory::submit! {
            crate::harness::OpEntry::new(
                OP_ID,
                || $fn_name("target", 0, 2),
                Some(|| {
                    let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
                    vec![vec![to_bytes(&$inventory_input)]]
                }),
                Some(|| {
                    let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
                    vec![vec![to_bytes(&$inventory_expected)]]
                }),
            )
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn inventory_case_matches_cpu_reference() {
                let mut buf = $inventory_input.to_vec();
                cpu_ref(&mut buf, 0);
                assert_eq!(buf, $inventory_expected.to_vec());
            }

            #[test]
            fn out_of_range_is_noop() {
                let mut buf = $inventory_input.to_vec();
                let before = buf.clone();
                cpu_ref(&mut buf, u32::MAX);
                assert_eq!(buf, before);
            }

            #[test]
            fn wrapper_program_uses_requested_shape() {
                let program = $fn_name("target", 0, 2);
                assert_eq!(program.buffers().len(), 1);
                assert_eq!(program.workgroup_size(), [1, 1, 1]);
            }
        }
    };
}

pub(crate) use define_bit_update_op;

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar(mut input: Vec<u32>, bit_idx: u32, kind: BitUpdateKind) -> Vec<u32> {
        let word = (bit_idx / 32) as usize;
        let bit = bit_idx % 32;
        if word < input.len() {
            match kind {
                BitUpdateKind::Set => input[word] |= 1u32 << bit,
                BitUpdateKind::Clear => input[word] &= !(1u32 << bit),
            }
        }
        input
    }

    #[test]
    fn generated_bit_updates_match_scalar_reference() {
        let mut state = 0xB175_E7A5_u32;
        for case in 0..4096u32 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let words = (state % 9) as usize;
            let mut input = Vec::with_capacity(words);
            for lane in 0..words {
                state = state.rotate_left(7) ^ (lane as u32).wrapping_mul(0x9E37_79B9);
                input.push(state);
            }
            let bit_idx = match case % 7 {
                0 => 0,
                1 => 31,
                2 => 32,
                3 => words as u32 * 32,
                _ => state % 320,
            };

            for kind in [BitUpdateKind::Set, BitUpdateKind::Clear] {
                let mut actual = input.clone();
                cpu_ref(&mut actual, bit_idx, kind);
                assert_eq!(
                    actual,
                    scalar(input.clone(), bit_idx, kind),
                    "generated bit update case {case} kind {kind:?}"
                );
            }
        }
    }
}
