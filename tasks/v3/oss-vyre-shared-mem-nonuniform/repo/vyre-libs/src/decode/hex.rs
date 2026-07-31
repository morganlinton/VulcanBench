//! GPU hex decode composition.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
#[cfg(test)]
use vyre_primitives::decode::hex::hex_decode_reference_packed;
use vyre_primitives::decode::hex::{
    hex_decode_child, hex_decode_pair_expr, hex_decoded_capacity, HEX_DECODE_TABLE_WORDS,
    HEX_WORKGROUP_SIZE,
};
pub use vyre_primitives::decode::hex::{hex_decode_table, hex_decode_table_ref};

#[cfg(test)]
use crate::buffer_names::fixed_name;
use crate::decode::buffers::{scoped_decode_input_buffer, scoped_decoded_output_buffer};
use crate::decode::scan::tiled_decode_aho_scan_body;
use crate::region::wrap_anonymous;

const OP_ID: &str = "vyre-libs::decode::hex";
const FUSED_SCAN_OP_ID: &str = "vyre-libs::decode::hex_then_aho_corasick";
const FAMILY_PREFIX: &str = "decode_hex";
/// Fixed buffer name carrying the ASCII hex decode lookup table.
pub const HEX_DECODE_TABLE_BUFFER: &str = "__vyre_decode_hex_table";

use crate::scan::dispatch_io::pack_u32_slice as pack_words;

/// Build a Program that decodes ASCII hex bytes from `input` into `output`,
/// storing one decoded byte per `u32` slot.
///
/// ```ignore
/// use vyre_libs::decode::hex_decode;
///
/// let program = hex_decode("encoded", "decoded", 8);
/// assert_eq!(program.buffers().len(), 2);
/// ```
#[must_use]
pub fn hex_decode(input: &str, output: &str, input_len: u32) -> Program {
    let input = scoped_decode_input_buffer(FAMILY_PREFIX, input);
    let output = scoped_decoded_output_buffer(FAMILY_PREFIX, output);
    let body = vec![hex_decode_child(
        OP_ID,
        &input,
        &output,
        HEX_DECODE_TABLE_BUFFER,
        input_len,
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage(&input, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(input_len),
            BufferDecl::output(&output, 1, DataType::U32)
                .with_count(hex_decoded_capacity(input_len)),
            BufferDecl::storage(
                HEX_DECODE_TABLE_BUFFER,
                2,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(HEX_DECODE_TABLE_WORDS),
        ],
        HEX_WORKGROUP_SIZE,
        vec![wrap_anonymous(OP_ID, body)],
    )
}

/// Build one GPU program that hex-decodes and then scans the decoded bytes
/// with the Aho-Corasick transition table, without a host readback between
/// stages.
///
/// ```ignore
/// use vyre_libs::decode::hex::hex_decode_then_aho_corasick;
///
/// let program = hex_decode_then_aho_corasick(
///     "encoded",
///     "decoded",
///     "transitions",
///     "accept",
///     "matches",
///     8,
///     4,
/// );
/// assert_eq!(program.output_buffer_indices().len(), 1);
/// ```
#[must_use]
pub fn hex_decode_then_aho_corasick(
    input: &str,
    decoded: &str,
    transitions: &str,
    accept: &str,
    matches: &str,
    input_len: u32,
    state_count: u32,
) -> Program {
    let input = scoped_decode_input_buffer(FAMILY_PREFIX, input);
    let decoded = scoped_decoded_output_buffer(FAMILY_PREFIX, decoded);
    let decoded_len = hex_decoded_capacity(input_len);
    let body = tiled_decode_aho_scan_body(
        transitions,
        accept,
        matches,
        Expr::u32(decoded_len),
        64,
        |pair| hex_decode_pair_expr(&input, HEX_DECODE_TABLE_BUFFER, pair),
        |pair, byte| Some(Node::store(&decoded, pair, byte)),
    );
    Program::wrapped(
        vec![
            BufferDecl::storage(&input, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(input_len),
            BufferDecl::read_write(&decoded, 1, DataType::U32).with_count(decoded_len),
            BufferDecl::storage(transitions, 2, BufferAccess::ReadOnly, DataType::U32)
                .with_count(state_count.saturating_mul(256)),
            BufferDecl::storage(accept, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(state_count),
            BufferDecl::output(matches, 4, DataType::U32).with_count(decoded_len),
            BufferDecl::storage(
                HEX_DECODE_TABLE_BUFFER,
                5,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(HEX_DECODE_TABLE_WORDS),
        ],
        HEX_WORKGROUP_SIZE,
        vec![wrap_anonymous(FUSED_SCAN_OP_ID, body)],
    )
}

#[cfg(test)]
fn cpu_ref(input: &[u8]) -> Vec<u32> {
    hex_decode_reference_packed(input)
}

fn fixture_inputs() -> Vec<Vec<Vec<u8>>> {
    vec![
        vec![
            pack_words(&[
                u32::from(b'4'),
                u32::from(b'D'),
                u32::from(b'6'),
                u32::from(b'1'),
                u32::from(b'6'),
                u32::from(b'E'),
            ]),
            pack_words(&[0, 0, 0]),
            pack_words(hex_decode_table_ref()),
        ],
        vec![
            pack_words(&[
                u32::from(b'6'),
                u32::from(b'8'),
                u32::from(b'4'),
                u32::from(b'9'),
                u32::from(b'4'),
                u32::from(b'A'),
            ]),
            pack_words(&[0, 0, 0]),
            pack_words(hex_decode_table_ref()),
        ],
        vec![
            pack_words(&[
                u32::from(b'7'),
                u32::from(b'a'),
                u32::from(b'Z'),
                u32::from(b'1'),
                u32::from(b'0'),
                u32::from(b'0'),
            ]),
            pack_words(&[0, 0, 0]),
            pack_words(hex_decode_table_ref()),
        ],
    ]
}

fn fixture_outputs() -> Vec<Vec<Vec<u8>>> {
    vec![
        vec![pack_words(&[0x4D, 0x61, 0x6E])],
        vec![pack_words(&[0x68, 0x49, 0x4A])],
        vec![pack_words(&[0x7A, 0x01, 0x00])],
    ]
}

inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || hex_decode("input", "output", 6),
        Some(fixture_inputs),
        Some(fixture_outputs),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_primitives::matching::CompiledDfa;
    use vyre_reference::value::Value;

    fn run(input: &[u8]) -> Vec<u32> {
        let program = hex_decode("input", "output", input.len() as u32);
        let inputs = vec![
            Value::from(pack_words(
                &input
                    .iter()
                    .map(|&byte| u32::from(byte))
                    .collect::<Vec<_>>(),
            )),
            Value::from(vec![0u8; (input.len() / 2) * 4]),
            Value::from(pack_words(hex_decode_table_ref())),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: hex decode must run; restore this invariant before continuing.");
        vyre_primitives::wire::decode_u32_le_bytes_all(&outputs[0].to_bytes())
    }

    #[test]
    fn decodes_uppercase_hex() {
        assert_eq!(run(b"4D616E"), vec![77, 97, 110]);
    }

    #[test]
    fn decodes_lowercase_hex() {
        assert_eq!(run(b"68494a"), vec![104, 73, 74]);
    }

    #[test]
    fn decodes_sixteen_char_hex() {
        // 16-character input → 8 output bytes. Regression guard against
        // any O(n²) path that re-walks the input per output byte.
        assert_eq!(
            run(b"4D616E6973657321"),
            vec![77, 97, 110, 105, 115, 101, 115, 33]
        );
    }

    #[test]
    fn invalid_nibble_clamps_to_zero() {
        assert_eq!(run(b"7aZ100"), vec![122, 1, 0]);
    }

    #[test]
    fn generated_pairs_match_primitive_reference_for_invalid_and_mixed_case() {
        const ALPHABET: &[u8] = b"0123456789abcdefABCDEFXz*#\n";

        for seed in 0u32..4096 {
            let pairs = 1 + (seed % 16);
            let mut state = seed ^ 0x48EC_DECD;
            let mut input = Vec::with_capacity(pairs as usize * 2);
            for _ in 0..(pairs * 2) {
                state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                input.push(ALPHABET[(state as usize) % ALPHABET.len()]);
            }

            assert_eq!(run(&input), cpu_ref(&input), "hex wrapper seed {seed}");
        }
    }

    #[test]
    fn generic_default_names_are_family_scoped() {
        let program = hex_decode("input", "decoded", 6);
        assert_eq!(
            program.buffers()[0].name(),
            fixed_name(FAMILY_PREFIX, "input")
        );
        assert_eq!(
            program.buffers()[1].name(),
            fixed_name(FAMILY_PREFIX, "decoded")
        );
        assert_eq!(program.buffers()[2].name(), HEX_DECODE_TABLE_BUFFER);
    }

    #[test]
    fn fused_program_reuses_decoded_buffer_for_scan() {
        let dfa = CompiledDfa {
            transitions: vec![0; 256],
            accept: vec![0],
            state_count: 1,
            max_pattern_len: 0,
            output_offsets: vec![0, 0],
            output_records: vec![],
        };
        let program = hex_decode_then_aho_corasick(
            "input",
            "decoded",
            "transitions",
            "accept",
            "matches",
            8,
            dfa.state_count,
        );
        assert_eq!(
            program.buffers()[1].name(),
            fixed_name(FAMILY_PREFIX, "decoded")
        );
        assert_eq!(program.buffers()[4].name(), "matches");
        assert_eq!(program.buffers()[5].name(), HEX_DECODE_TABLE_BUFFER);
    }
}
