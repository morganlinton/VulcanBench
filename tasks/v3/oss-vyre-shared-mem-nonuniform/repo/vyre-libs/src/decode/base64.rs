//! GPU base64 decode compositions.

use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Program};

#[cfg(test)]
use crate::buffer_names::fixed_name;
use crate::decode::buffers::{scoped_decode_input_buffer, scoped_decoded_output_buffer};
use crate::decode::scan::linear_aho_scan_body;
use crate::region::wrap_anonymous;
#[cfg(test)]
use vyre_primitives::decode::base64::decode_standard_packed_reference;
use vyre_primitives::decode::base64::{
    base64_decode_child, decoded_capacity, standard_decode_table_ref, BASE64_DECODE_TABLE_WORDS,
    BASE64_WORKGROUP_SIZE,
};

const OP_ID: &str = "vyre-libs::decode::base64";
const FUSED_SCAN_OP_ID: &str = "vyre-libs::decode::base64_then_aho_corasick";
const FAMILY_PREFIX: &str = "decode_base64";

/// Fixed buffer name carrying the base64 decode lookup table.
///
/// The buffer contains 256 `u32` entries; each entry is the six-bit value for
/// the corresponding ASCII byte, or `0xFF` for invalid input.
///
/// ```ignore
/// use vyre_libs::decode::{base64_decode, BASE64_DECODE_TABLE_BUFFER};
///
/// let program = base64_decode("encoded", "decoded", 8);
/// assert_eq!(program.buffers()[1].name(), BASE64_DECODE_TABLE_BUFFER);
/// ```
pub const BASE64_DECODE_TABLE_BUFFER: &str = "__vyre_decode_base64_table";
const DECODED_LEN_BUFFER: &str = "__vyre_decode_base64_decoded_len";

use crate::scan::dispatch_io::pack_u32_slice as pack_words;

/// Build a Program that decodes base64-encoded ASCII bytes from `input` into
/// `output`, storing one decoded byte per `u32` slot.
///
/// The input buffer carries one ASCII byte per `u32` element so the decode
/// output can chain directly into Aho-Corasick transition-table programs.
///
/// ```ignore
/// use vyre_libs::decode::base64::base64_decode;
///
/// let program = base64_decode("encoded", "decoded", 8);
/// assert_eq!(program.workgroup_size(), [64, 1, 1]);
/// ```
///
#[must_use]
pub fn base64_decode(input: &str, output: &str, input_len: u32) -> Program {
    let input = scoped_decode_input_buffer(FAMILY_PREFIX, input);
    let output = scoped_decoded_output_buffer(FAMILY_PREFIX, output);
    let body = vec![base64_decode_child(
        OP_ID,
        &input,
        BASE64_DECODE_TABLE_BUFFER,
        &output,
        DECODED_LEN_BUFFER,
        input_len,
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage(&input, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(input_len),
            BufferDecl::storage(
                BASE64_DECODE_TABLE_BUFFER,
                1,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(BASE64_DECODE_TABLE_WORDS),
            BufferDecl::output(&output, 2, DataType::U32).with_count(decoded_capacity(input_len)),
            // Length is aux state  -  `read_write` only (V022: at most one `::output`).
            BufferDecl::read_write(DECODED_LEN_BUFFER, 3, DataType::U32).with_count(1),
        ],
        BASE64_WORKGROUP_SIZE,
        vec![wrap_anonymous(OP_ID, body)],
    )
}

/// Build one GPU program that base64-decodes and then scans the decoded bytes
/// with the Aho-Corasick transition table, without a host readback between
/// stages.
///
/// ```ignore
/// use vyre_libs::decode::base64::base64_decode_then_aho_corasick;
///
/// let program = base64_decode_then_aho_corasick(
///     "encoded",
///     "decoded",
///     "transitions",
///     "accept",
///     "matches",
///     8,
///     4,
/// );
/// assert_eq!(program.output_buffer_indices().len(), 2);
/// ```
#[must_use]
pub fn base64_decode_then_aho_corasick(
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
    let decoded_capacity = decoded_capacity(input_len);
    let mut entry = vec![base64_decode_child(
        FUSED_SCAN_OP_ID,
        &input,
        BASE64_DECODE_TABLE_BUFFER,
        &decoded,
        DECODED_LEN_BUFFER,
        input_len,
    )];
    entry.extend(linear_aho_scan_body(
        &decoded,
        transitions,
        accept,
        matches,
        Expr::load(DECODED_LEN_BUFFER, Expr::u32(0)),
    ));
    Program::wrapped(
        vec![
            BufferDecl::storage(&input, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(input_len),
            BufferDecl::storage(
                BASE64_DECODE_TABLE_BUFFER,
                1,
                BufferAccess::ReadOnly,
                DataType::U32,
            )
            .with_count(BASE64_DECODE_TABLE_WORDS),
            BufferDecl::read_write(&decoded, 2, DataType::U32).with_count(decoded_capacity),
            BufferDecl::storage(transitions, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(state_count.saturating_mul(256)),
            BufferDecl::storage(accept, 4, BufferAccess::ReadOnly, DataType::U32)
                .with_count(state_count),
            BufferDecl::output(matches, 5, DataType::U32).with_count(decoded_capacity),
            BufferDecl::read_write(DECODED_LEN_BUFFER, 6, DataType::U32).with_count(1),
        ],
        BASE64_WORKGROUP_SIZE,
        vec![wrap_anonymous(FUSED_SCAN_OP_ID, entry)],
    )
}

#[cfg(test)]
fn cpu_ref(input: &[u8]) -> (Vec<u32>, u32) {
    decode_standard_packed_reference(input)
}

fn fixture_inputs() -> Vec<Vec<Vec<u8>>> {
    vec![
        vec![
            pack_words(&[
                u32::from(b'T'),
                u32::from(b'W'),
                u32::from(b'F'),
                u32::from(b'u'),
                u32::from(b'T'),
                u32::from(b'W'),
                u32::from(b'F'),
                u32::from(b'u'),
            ]),
            pack_words(standard_decode_table_ref()),
            vec![0u8; 4],
        ],
        vec![
            pack_words(&[
                u32::from(b'T'),
                u32::from(b'W'),
                u32::from(b'E'),
                u32::from(b'='),
                u32::from(b'T'),
                u32::from(b'W'),
                u32::from(b'E'),
                u32::from(b'='),
            ]),
            pack_words(standard_decode_table_ref()),
            vec![0u8; 4],
        ],
        vec![
            pack_words(&[
                u32::from(b'S'),
                u32::from(b'G'),
                u32::from(b'V'),
                u32::from(b's'),
                u32::from(b'b'),
                u32::from(b'G'),
                u32::from(b'8'),
                u32::from(b'*'),
            ]),
            pack_words(standard_decode_table_ref()),
            vec![0u8; 4],
        ],
    ]
}

fn fixture_outputs() -> Vec<Vec<Vec<u8>>> {
    vec![
        vec![pack_words(&[77, 97, 110, 77, 97, 110]), pack_words(&[6])],
        vec![pack_words(&[77, 97, 0, 77, 97, 0]), pack_words(&[5])],
        vec![pack_words(&[72, 101, 108, 108, 111, 0]), pack_words(&[6])],
    ]
}

inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || base64_decode("input", "output", 8),
        Some(fixture_inputs),
        Some(fixture_outputs),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_primitives::matching::CompiledDfa;
    use vyre_reference::value::Value;

    fn run(input: &[u8]) -> (Vec<u32>, u32) {
        let program = base64_decode("input", "output", input.len() as u32);
        let decoded_capacity = decoded_capacity(input.len() as u32);
        let inputs = vec![
            Value::from(pack_words(
                &input
                    .iter()
                    .map(|&byte| u32::from(byte))
                    .collect::<Vec<_>>(),
            )),
            Value::from(pack_words(standard_decode_table_ref())),
            Value::from(vec![0u8; decoded_capacity as usize * 4]),
            Value::from(vec![0u8; 4]),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: base64 decode must run; restore this invariant before continuing.");
        let decoded = vyre_primitives::wire::decode_u32_le_bytes_all(&outputs[0].to_bytes());
        let len_bytes = outputs[1].to_bytes();
        let decoded_len =
            u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]);
        (decoded, decoded_len)
    }

    #[test]
    fn aligned_input_decodes_three_bytes() {
        let (decoded, decoded_len) = run(b"TWFu");
        assert_eq!(&decoded[..3], &[77, 97, 110]);
        assert_eq!(decoded_len, 3);
    }

    #[test]
    fn padded_input_reports_real_length() {
        let (decoded, decoded_len) = run(b"TQ==");
        assert_eq!(decoded[0], 77);
        assert_eq!(decoded_len, 1);
    }

    #[test]
    fn invalid_character_clamps_without_panicking() {
        let (decoded, decoded_len) = run(b"SGVsbG8*");
        assert_eq!(&decoded[..6], &[72, 101, 108, 108, 111, 0]);
        assert_eq!(decoded_len, 6);
    }

    #[test]
    fn malformed_length_lowers_to_ir_trap_not_host_panic() {
        let program = base64_decode("input", "output", 3);
        assert!(program.stats().trap());
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
        let program = base64_decode_then_aho_corasick(
            "input",
            "decoded",
            "transitions",
            "accept",
            "matches",
            8,
            dfa.state_count,
        );
        assert_eq!(
            program.buffers()[2].name(),
            fixed_name(FAMILY_PREFIX, "decoded")
        );
        assert_eq!(program.buffers()[5].name(), "matches");
        assert_eq!(program.buffers()[6].name(), DECODED_LEN_BUFFER);
    }

    #[test]
    fn generic_default_names_are_family_scoped() {
        let program = base64_decode("input", "decoded", 8);
        assert_eq!(
            program.buffers()[0].name(),
            fixed_name(FAMILY_PREFIX, "input")
        );
        assert_eq!(
            program.buffers()[2].name(),
            fixed_name(FAMILY_PREFIX, "decoded")
        );
        assert_eq!(program.buffers()[3].name(), DECODED_LEN_BUFFER);
    }

    #[test]
    fn twelve_byte_input_decodes_nine_bytes_in_linear_time() {
        let (decoded, decoded_len) = run(b"TWFuTWFuTWFu");
        assert_eq!(&decoded[..9], &[77, 97, 110, 77, 97, 110, 77, 97, 110]);
        assert_eq!(decoded_len, 9);
    }

    #[test]
    fn generated_quads_match_cpu_reference_for_invalid_padding_and_symbols() {
        const ALPHABET: &[u8] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=*#\n";

        for seed in 0u32..4096 {
            let quads = 1 + (seed % 6);
            let mut state = seed ^ 0xB64D_EC0D;
            let mut input = Vec::with_capacity(quads as usize * 4);
            for _ in 0..(quads * 4) {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                input.push(ALPHABET[(state as usize) % ALPHABET.len()]);
            }

            let (actual, actual_len) = run(&input);
            let (expected, expected_len) = cpu_ref(&input);
            assert_eq!(actual_len, expected_len, "decoded length seed {seed}");
            assert_eq!(actual, expected, "decoded bytes seed {seed}");
        }
    }
}
