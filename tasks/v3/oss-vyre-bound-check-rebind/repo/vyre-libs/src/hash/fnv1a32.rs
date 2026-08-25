//! FNV-1a 32-bit non-cryptographic hash.
//!
//! Category A composition  -  the kernel body is
//! [`vyre_primitives::hash::fnv1a::fnv1a32_program_dyn`]; the Tier-3
//! wrapper stamps the `vyre-libs::hash::fnv1a32` op id, carries the
//! `OpEntry` fixtures, and exposes the universal `(input, out)`
//! signature the harness uses.

use vyre::ir::Program;
use vyre_primitives::hash::fnv1a::{fnv1a32_program, fnv1a32_program_dyn, FNV1A32_OP_ID};

#[cfg(test)]
use crate::buffer_names::fixed_name;
#[cfg(test)]
use vyre_primitives::hash::fnv1a::{
    fnv1a32 as cpu_ref_u32, fnv1a32_packed_u32_low8 as cpu_ref_words,
};

use super::wrap::HashWrapperSpec;

const OP_ID: &str = "vyre-libs::hash::fnv1a32";
const FAMILY_PREFIX: &str = "hash_fnv1a32";
const SPEC: HashWrapperSpec = HashWrapperSpec::new(OP_ID, FNV1A32_OP_ID, FAMILY_PREFIX, 1);

/// Build a Program that computes FNV-1a 32-bit over `input` bytes,
/// writing the result to `out[0]`.
///
/// `input` is a u32 buffer with one byte per slot (upper 24 bits zero).
/// `out` is a single-slot u32 buffer.
#[must_use]
pub fn fnv1a32(input: &str, out: &str) -> Program {
    let (input, out) = SPEC.scoped_standard_buffers(input, out);
    let primitive = fnv1a32_program_dyn(&input, &out);
    SPEC.wrap_dynamic_count(&input, &out, primitive)
}

/// Build a Program that computes FNV-1a 32-bit over exactly `n` input slots.
#[must_use]
pub fn fnv1a32_n(input: &str, out: &str, n: u32) -> Program {
    let (input, out) = SPEC.scoped_standard_buffers(input, out);
    let primitive = fnv1a32_program(&input, &out, n);
    SPEC.wrap_static_count(&input, &out, n, primitive)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || fnv1a32_n("input", "out", 3),
        test_inputs: Some(|| vec![vec![
            vec![0x61, 0, 0, 0, 0x62, 0, 0, 0, 0x63, 0, 0, 0],
        ]]),
        expected_output: Some(|| vec![{
            let hash = 0x1a47_e90bu32;
            vec![hash.to_le_bytes().to_vec()]
        }]),
        category: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::pack_bytes_as_u32;
    use vyre_reference::value::Value;

    fn run(bytes: &[u8]) -> u32 {
        let program = fnv1a32("input", "out");
        let inputs = vec![Value::Bytes(pack_bytes_as_u32(bytes).into())];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: fnv1a32 must run; restore this invariant before continuing.");
        let raw = outputs[0].to_bytes();
        vyre_primitives::wire::read_u32_le_word(&raw, 0, "fnv1a32 output")
            .expect("Fix: fnv1a32 output must contain one u32.")
    }

    fn run_words(words: &[u32]) -> u32 {
        let program = fnv1a32_n("input", "out", words.len().max(1) as u32);
        let input = vyre_primitives::wire::pack_u32_slice(words);
        let outputs = vyre_reference::reference_eval(&program, &[Value::Bytes(input.into())])
            .expect("Fix: fnv1a32 must run on u32 byte slots.");
        let raw = outputs[0].to_bytes();
        vyre_primitives::wire::read_u32_le_word(&raw, 0, "fnv1a32 word output")
            .expect("Fix: fnv1a32 word output must contain one u32.")
    }

    #[test]
    fn abc_matches_canonical_vector_and_cpu_ref() {
        assert_eq!(run(b"abc"), 0x1a47_e90b);
        assert_eq!(run(b"abc"), cpu_ref_u32(b"abc"));
    }

    #[test]
    fn random_512_bytes_match_ref() {
        let mut x: u32 = 0xA5A5_1357;
        let bytes: Vec<u8> = (0..512)
            .map(|_| {
                x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (x >> 24) as u8
            })
            .collect();
        assert_eq!(run(&bytes), cpu_ref_u32(&bytes));
    }

    #[test]
    fn high_input_bits_are_ignored() {
        let words = [0xFFFF_FF61, 0xCAFE_0062, 0x8000_0063];
        assert_eq!(run_words(&words), cpu_ref_words(&words));
        assert_eq!(run_words(&words), cpu_ref_u32(b"abc"));
    }

    #[test]
    fn wrapper_delegates_to_primitive_fnv1a32_region() {
        let program = fnv1a32_n("input", "out", 3);
        let [vyre::ir::Node::Region { body, .. }] = program.entry() else {
            panic!("expected one top-level FNV-1a32 wrapper region");
        };
        let [vyre::ir::Node::Region { generator, .. }] = body.as_ref().as_slice() else {
            panic!("expected FNV-1a32 wrapper to contain one primitive child region");
        };
        assert_eq!(generator.as_str(), FNV1A32_OP_ID);
    }

    #[test]
    fn generic_default_names_are_family_scoped() {
        let program = fnv1a32("input", "out");
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
