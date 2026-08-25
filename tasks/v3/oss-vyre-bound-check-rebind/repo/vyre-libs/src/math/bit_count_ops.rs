use super::bit_count_u32::{bit_count_u32_program, BitCountKind};
use vyre::ir::Program;

macro_rules! define_bit_count_u32_op {
    (
        module = $module:ident,
        function = $function:ident,
        op_id = $op_id:expr,
        kind = $kind:expr,
        reference = $reference:ident,
        expected = $expected:expr,
        doc = $doc:expr
    ) => {
        #[doc = $doc]
        pub mod $module {
            use super::*;

            const OP_ID: &str = $op_id;

            #[doc = $doc]
            #[must_use]
            pub fn $function(input: &str, out: &str, size: u32) -> Program {
                bit_count_u32_program(OP_ID, input, out, size, $kind)
            }

            inventory::submit! {
                crate::harness::OpEntry {
                    id: OP_ID,
                    build: || $function("input", "out", 4),
                    test_inputs: Some(|| {
                        let input = [0u32, 1, 0x8000_0000, 0x00F0_0000];
                        let to_bytes = vyre_primitives::wire::pack_u32_slice;
                        vec![vec![to_bytes(&input)]]
                    }),
                    expected_output: Some(|| {
                        let bytes = vyre_primitives::wire::pack_u32_slice(&$expected);
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
                    let program = $function("input", "out", n.max(1));
                    let to_bytes = vyre_primitives::wire::pack_u32_slice;
                    let inputs = vec![
                        Value::Bytes(to_bytes(input).into()),
                        Value::Bytes(vec![0u8; (n.max(1) * 4) as usize].into()),
                    ];
                    let outputs = vyre_reference::reference_eval(&program, &inputs)
                        .expect("Fix: bit-count u32 op must run in the reference interpreter.");
                    vyre_primitives::wire::decode_u32_le_bytes_all(&outputs[0].to_bytes())
                }

                #[test]
                fn matches_rust_reference_samples() {
                    let input = [0u32, 1, 2, 3, 0x8000_0000, 0x00F0_0000, u32::MAX];
                    let got = run(&input);
                    let expected: Vec<u32> = input.iter().map(|value| value.$reference()).collect();
                    assert_eq!(got, expected);
                }
            }
        }
    };
}

define_bit_count_u32_op!(
    module = lzcnt_u32,
    function = lzcnt_u32,
    op_id = "vyre-libs::math::lzcnt_u32",
    kind = BitCountKind::LeadingZeros,
    reference = leading_zeros,
    expected = [32u32, 31, 0, 8],
    doc = "Map `input[i] -> input[i].leading_zeros()` into `out[i]`."
);

define_bit_count_u32_op!(
    module = tzcnt_u32,
    function = tzcnt_u32,
    op_id = "vyre-libs::math::tzcnt_u32",
    kind = BitCountKind::TrailingZeros,
    reference = trailing_zeros,
    expected = [32u32, 0, 31, 20],
    doc = "Map `input[i] -> input[i].trailing_zeros()` into `out[i]`."
);
