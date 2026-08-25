//! Cat-A `adler32` wrapper  -  Adler-32 (RFC 1950) checksum.
//!
//! Serial single-invocation walk. A init 1, B init 0, both mod 65521
//! per byte. Output `(B << 16) | A`.
//!
//! Delegates the executable checksum body to `vyre-primitives`; this Tier-3
//! module owns only scoped buffer naming, provenance, and harness registration.

use vyre::ir::Program;
use vyre_primitives::hash::adler32::{adler32_program, ADLER32_OP_ID};

#[cfg(test)]
use crate::buffer_names::fixed_name;
#[cfg(test)]
use vyre_primitives::hash::adler32::adler32 as adler32_cpu_reference;

use super::wrap::HashWrapperSpec;

const OP_ID: &str = "vyre-libs::hash::adler32";
const FAMILY_PREFIX: &str = "hash_adler32";
const SPEC: HashWrapperSpec = HashWrapperSpec::new(OP_ID, ADLER32_OP_ID, FAMILY_PREFIX, 1);

/// Build a Program that writes Adler-32(input[0..n]) to `out[0]`.
#[must_use]
pub fn adler32(input: &str, out: &str, n: u32) -> Program {
    let (input, out) = SPEC.scoped_standard_buffers(input, out);
    let primitive = adler32_program(&input, &out, n);
    SPEC.wrap_static_count(&input, &out, n, primitive)
}

#[cfg(test)]
fn cpu_ref(input: &[u8]) -> u32 {
    adler32_cpu_reference(input)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || adler32("input", "out", 3),
        test_inputs: Some(|| {
            let bytes = vyre_primitives::wire::pack_bytes_as_u32_slice(b"abc");
            vec![vec![bytes]]
        }),
        // Adler-32("abc") = 0x024D0127 (a = 295, b = 589).
        expected_output: Some(|| vec![vec![0x024D_0127u32.to_le_bytes().to_vec()]]),
        category: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::pack_bytes_as_u32;
    use vyre_reference::value::Value;

    fn run(bytes: &[u8]) -> u32 {
        let n = bytes.len().max(1) as u32;
        let program = adler32("input", "out", n);
        let inputs = vec![Value::Bytes(pack_bytes_as_u32(bytes).into())];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: adler32 must run; restore this invariant before continuing.");
        let raw = outputs[0].to_bytes();
        vyre_primitives::wire::read_u32_le_word(&raw, 0, "adler32 output")
            .expect("Fix: adler32 output must contain one u32.")
    }

    fn run_words(words: &[u32]) -> u32 {
        let n = words.len().max(1) as u32;
        let program = adler32("input", "out", n);
        let input = vyre_primitives::wire::pack_u32_slice(words);
        let outputs = vyre_reference::reference_eval(&program, &[Value::Bytes(input.into())])
            .expect("Fix: adler32 must run on u32 byte slots.");
        let raw = outputs[0].to_bytes();
        vyre_primitives::wire::read_u32_le_word(&raw, 0, "adler32 word output")
            .expect("Fix: adler32 word output must contain one u32.")
    }

    #[test]
    fn abc_matches_rfc1950_example() {
        assert_eq!(run(b"abc"), 0x024D_0127);
        assert_eq!(run(b"abc"), cpu_ref(b"abc"));
    }

    #[test]
    fn wikipedia_string() {
        assert_eq!(run(b"Wikipedia"), 0x11E6_0398);
    }

    #[test]
    fn random_64_bytes_match_ref() {
        let bytes: Vec<u8> = (0u8..64).collect();
        assert_eq!(run(&bytes), cpu_ref(&bytes));
    }

    #[test]
    fn high_input_bits_are_ignored() {
        assert_eq!(run_words(&[0xFFFF_FF61, 0xA5A5_0062]), cpu_ref(b"ab"));
    }

    #[test]
    fn generic_default_names_are_family_scoped() {
        let program = adler32("input", "out", 4);
        assert_eq!(
            program.buffers()[0].name(),
            fixed_name(FAMILY_PREFIX, "input")
        );
        assert_eq!(
            program.buffers()[1].name(),
            fixed_name(FAMILY_PREFIX, "out")
        );
    }
}
