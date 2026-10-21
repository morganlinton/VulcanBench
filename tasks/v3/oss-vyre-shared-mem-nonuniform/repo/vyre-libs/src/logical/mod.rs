//! Elementwise logical operations (and, or, xor, nand, nor).
//! Ported from the legacy target-text implementations in vyre-ops.

macro_rules! define_wrapped_bitset_binary {
    ($module:ident, $function:ident, $op_id:literal, $primitive:path, $primitive_op_id:path, $expected:expr, $doc:literal) => {
        #[doc = $doc]
        pub mod $module {
            use super::wrap::wrap_bitset_binary;
            use vyre::ir::Program;

            const OP_ID: &str = $op_id;

            /// Build the logical wrapper around the canonical bitset primitive.
            #[must_use]
            pub fn $function(a: &str, b: &str, out: &str, size: u32) -> Program {
                let primitive = $primitive(a, b, out, size);
                wrap_bitset_binary(OP_ID, $primitive_op_id, a, b, out, size, primitive)
            }

            inventory::submit! {
                crate::harness::OpEntry {
                    id: OP_ID,
                    build: || $function("a", "b", "out", 4),
                    test_inputs: Some(|| {
                        let a = [0xFF00_FF00u32, 0x00FF_00FF, 0xFFFF_FFFF, 0x0000_0000];
                        let b = [0xF0F0_F0F0u32, 0x0F0F_0F0F, 0xFFFF_FFFF, 0x0000_0000];
                        let to_bytes = vyre_primitives::wire::pack_u32_slice;
                        vec![vec![to_bytes(&a), to_bytes(&b), vec![0u8; 16]]]
                    }),
                    expected_output: Some(|| {
                        let to_bytes = vyre_primitives::wire::pack_u32_slice;
                        vec![vec![to_bytes($expected)]]
                    }),
                    category: None,
                }
            }
        }
    };
}

macro_rules! define_synthesized_logical_binary {
    ($module:ident, $function:ident, $op_id:literal, $expr:expr, $expected:expr, $doc:literal) => {
        #[doc = $doc]
        pub mod $module {
            use super::wrap::build_logical_binary;
            use vyre::ir::Program;

            const OP_ID: &str = $op_id;

            /// Build the synthesized logical binary operation.
            #[must_use]
            pub fn $function(a: &str, b: &str, out: &str, size: u32) -> Program {
                build_logical_binary(OP_ID, a, b, out, size, $expr)
            }

            inventory::submit! {
                crate::harness::OpEntry {
                    id: OP_ID,
                    build: || $function("a", "b", "out", 4),
                    test_inputs: Some(|| {
                        let a = [0xFF00_FF00u32, 0x00FF_00FF, 0xFFFF_FFFF, 0x0000_0000];
                        let b = [0xF0F0_F0F0u32, 0x0F0F_0F0F, 0xFFFF_FFFF, 0x0000_0000];
                        let to_bytes = vyre_primitives::wire::pack_u32_slice;
                        vec![vec![to_bytes(&a), to_bytes(&b), vec![0u8; 16]]]
                    }),
                    expected_output: Some(|| {
                        let to_bytes = vyre_primitives::wire::pack_u32_slice;
                        vec![vec![to_bytes($expected)]]
                    }),
                    category: None,
                }
            }
        }
    };
}

define_wrapped_bitset_binary!(
    and,
    and,
    "vyre-libs::logical::and",
    vyre_primitives::bitset::and::bitset_and,
    vyre_primitives::bitset::and::OP_ID,
    &[0xF000_F000, 0x000F_000F, 0xFFFF_FFFF, 0x0000_0000],
    "Bitwise AND."
);
define_synthesized_logical_binary!(
    nand,
    nand,
    "vyre-libs::logical::nand",
    |left, right| vyre::ir::Expr::bitnot(vyre::ir::Expr::bitand(left, right)),
    &[0x0FFF_0FFF, 0xFFF0_FFF0, 0x0000_0000, 0xFFFF_FFFF],
    "Bitwise NAND."
);
define_synthesized_logical_binary!(
    nor,
    nor,
    "vyre-libs::logical::nor",
    |left, right| vyre::ir::Expr::bitnot(vyre::ir::Expr::bitor(left, right)),
    &[0x000F_000F, 0xF000_F000, 0x0000_0000, 0xFFFF_FFFF],
    "Bitwise NOR."
);
define_wrapped_bitset_binary!(
    or,
    or,
    "vyre-libs::logical::or",
    vyre_primitives::bitset::or::bitset_or,
    vyre_primitives::bitset::or::OP_ID,
    &[0xFFF0_FFF0, 0x0FFF_0FFF, 0xFFFF_FFFF, 0x0000_0000],
    "Bitwise OR."
);
define_wrapped_bitset_binary!(
    xor,
    xor,
    "vyre-libs::logical::xor",
    vyre_primitives::bitset::xor::bitset_xor,
    vyre_primitives::bitset::xor::OP_ID,
    &[0x0FF0_0FF0, 0x0FF0_0FF0, 0x0000_0000, 0x0000_0000],
    "Bitwise XOR."
);
mod wrap;

pub use and::and;
pub use nand::nand;
pub use nor::nor;
pub use or::or;
pub use xor::xor;

#[cfg(test)]
mod tests {
    use super::*;
    use vyre::ir::Node;
    use vyre_reference::value::Value;

    fn assert_delegates_to_primitive(program: vyre::ir::Program, expected_primitive: &str) {
        let [Node::Region { body, .. }] = program.entry() else {
            panic!("expected one top-level logical wrapper region");
        };
        let [Node::Region { generator, .. }] = body.as_ref().as_slice() else {
            panic!("expected logical wrapper to contain one primitive child region");
        };
        assert_eq!(generator.as_str(), expected_primitive);
    }

    #[test]
    fn and_delegates_to_bitset_primitive() {
        assert_delegates_to_primitive(and("a", "b", "out", 4), vyre_primitives::bitset::and::OP_ID);
    }

    #[test]
    fn or_delegates_to_bitset_primitive() {
        assert_delegates_to_primitive(or("a", "b", "out", 4), vyre_primitives::bitset::or::OP_ID);
    }

    #[test]
    fn xor_delegates_to_bitset_primitive() {
        assert_delegates_to_primitive(xor("a", "b", "out", 4), vyre_primitives::bitset::xor::OP_ID);
    }

    fn eval_u32_binary(program: &vyre::ir::Program, a: &[u32], b: &[u32]) -> Vec<u32> {
        let outputs = vyre_reference::reference_eval(
            program,
            &[
                Value::from(vyre_primitives::wire::pack_u32_slice(a)),
                Value::from(vyre_primitives::wire::pack_u32_slice(b)),
                Value::from(vec![0_u8; a.len() * core::mem::size_of::<u32>()]),
            ],
        )
        .expect("Fix: logical elementwise program must execute in the reference interpreter.");
        vyre_primitives::wire::decode_u32_le_bytes_all(&outputs[0].to_bytes())
    }

    #[test]
    fn generated_nand_nor_match_scalar_reference() {
        let mut state = 0x10CC_A11E_u32;
        for case in 0..1024_u32 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let len = (state as usize % 33) + 1;
            let mut a = Vec::with_capacity(len);
            let mut b = Vec::with_capacity(len);
            for index in 0..len {
                state = state.rotate_left(5) ^ (index as u32).wrapping_mul(0x9E37_79B9);
                a.push(match index % 4 {
                    0 => state,
                    1 => !state,
                    2 => 0,
                    _ => u32::MAX,
                });
                state = state.rotate_left(9) ^ (case.wrapping_mul(0x85EB_CA6B));
                b.push(match index % 5 {
                    0 => state,
                    1 => !state,
                    2 => 0xAAAA_AAAA,
                    3 => 0x5555_5555,
                    _ => u32::MAX,
                });
            }

            let nand_program = nand("a", "b", "out", len as u32);
            let nor_program = nor("a", "b", "out", len as u32);
            let expected_nand: Vec<u32> = a
                .iter()
                .zip(&b)
                .map(|(left, right)| !(left & right))
                .collect();
            let expected_nor: Vec<u32> = a
                .iter()
                .zip(&b)
                .map(|(left, right)| !(left | right))
                .collect();

            assert_eq!(
                eval_u32_binary(&nand_program, &a, &b),
                expected_nand,
                "case {case}"
            );
            assert_eq!(
                eval_u32_binary(&nor_program, &a, &b),
                expected_nor,
                "case {case}"
            );
        }
    }
}
