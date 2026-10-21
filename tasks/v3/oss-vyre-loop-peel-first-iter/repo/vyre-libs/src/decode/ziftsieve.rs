//! GPU LZ4 literal-extraction composition.
//!
//! `vyre-primitives::decode::ziftsieve` owns the reusable indexed literal-copy
//! kernel and host oracle. This module keeps the libs-level composition API:
//! scoped buffer names, fixture registration, and stable public exports for
//! decode-to-scan pipelines.

use vyre::ir::Program;
use vyre_primitives::decode::ziftsieve::ziftsieve_literal_copy_with_op_id;

// CPU parity oracle: re-exported only for parity tests / the `cpu-parity`
// feature, never as a production decode surface (matches the vyre-primitives
// gating of the underlying helper).
#[cfg(any(test, feature = "cpu-parity"))]
pub use vyre_primitives::decode::ziftsieve::ziftsieve_reference_extract_literals;

use crate::decode::buffers::{scoped_decode_input_buffer, scoped_decode_output_buffer};
#[cfg(test)]
use crate::scan::dispatch_io::pack_u32_slice as pack_words;

const OP_ID: &str = "vyre-libs::decode::ziftsieve";
const FAMILY_PREFIX: &str = "decode_ziftsieve";

/// Canonical pointer to the primitive-owned GPU-port design.
pub const NOTE_ZIFTSIEVE_GPU_DESIGN: &str = "docs: primitive GPU-port design is in \
     libs/performance/matching/vyre/vyre-primitives/src/decode/ziftsieve.rs";

/// Build a Program that copies LZ4 literals in parallel given a pre-built
/// sequence index.
///
/// The reusable IR and CPU oracle live in `vyre-primitives`; this wrapper only
/// applies libs-level buffer scoping and preserves the public composition id.
#[must_use]
pub fn ziftsieve_gpu(
    input: &str,
    output: &str,
    seq_literal_start: &str,
    seq_literal_len: &str,
    seq_literal_offset: &str,
    input_len: u32,
    seq_count: u32,
    max_output: u32,
) -> Program {
    let input = scoped_decode_input_buffer(FAMILY_PREFIX, input);
    let output =
        scoped_decode_output_buffer(FAMILY_PREFIX, "output", output, &["output", "decoded"]);
    ziftsieve_literal_copy_with_op_id(
        OP_ID,
        &input,
        &output,
        seq_literal_start,
        seq_literal_len,
        seq_literal_offset,
        input_len,
        seq_count,
        max_output,
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use vyre_reference::value::Value;

    fn run(input: &[u8], seq_starts: &[u32], seq_lens: &[u32], seq_offsets: &[u32]) -> Vec<u32> {
        let seq_count = seq_starts.len() as u32;
        let max_output = seq_lens.iter().copied().sum::<u32>();
        let input_words = input.iter().map(|&b| u32::from(b)).collect::<Vec<_>>();
        let program = ziftsieve_gpu(
            "input",
            "output",
            "seq_start",
            "seq_len",
            "seq_off",
            input.len() as u32,
            seq_count,
            max_output,
        );
        let inputs = vec![
            Value::from(pack_words(&input_words)),
            Value::from(pack_words(seq_starts)),
            Value::from(pack_words(seq_lens)),
            Value::from(pack_words(seq_offsets)),
            Value::from(vec![0u8; (max_output.max(1) as usize) * 4]),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: ziftsieve_gpu wrapper must run.");
        let words = vyre_primitives::wire::decode_u32_le_bytes_all(&outputs[0].to_bytes());
        words.into_iter().take(max_output as usize).collect()
    }

    #[test]
    fn single_literal() {
        assert_eq!(run(&[0x10, b'A'], &[1], &[1], &[0]), vec![b'A' as u32]);
    }

    #[test]
    fn two_sequences() {
        assert_eq!(
            run(&[0x10, b'A', 0x20, b'B', b'C'], &[1, 3], &[1, 2], &[0, 1]),
            vec![b'A' as u32, b'B' as u32, b'C' as u32]
        );
    }

    #[test]
    fn zero_literal_sequence_is_nop() {
        assert_eq!(
            run(&[0x00, 0x10, b'A'], &[0], &[0], &[0]),
            Vec::<u32>::new()
        );
    }

    #[test]
    fn wrapper_reexports_primitive_reference() {
        let result = ziftsieve_reference_extract_literals(&[0x10, b'A'], 1024).unwrap();
        assert_eq!(result, b"A");
    }
}
