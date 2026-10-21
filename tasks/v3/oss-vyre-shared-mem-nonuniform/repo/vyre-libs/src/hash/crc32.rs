//! Cat-A `crc32`  -  CRC-32 (ISO 3309 / ITU-T V.42) checksum.
//!
//! Serial single-invocation walk. Standard CRC-32 polynomial
//! 0xEDB88320 (reflected), init 0xFFFFFFFF, final XOR 0xFFFFFFFF,
//! delegated to the Tier-2.5 primitive so this wrapper owns only naming,
//! provenance, and harness registration.
//!
//! `input[i]` packs one byte per u32 slot (low 8 bits). `out[0]`
//! receives the final CRC-32.

use vyre::ir::Program;
use vyre_primitives::hash::crc32::{crc32_program, CRC32_OP_ID};

#[cfg(test)]
use crate::buffer_names::fixed_name;
#[cfg(test)]
use vyre_primitives::hash::crc32::crc32 as crc32_cpu_reference;

use super::wrap::HashWrapperSpec;

const OP_ID: &str = "vyre-libs::hash::crc32";
const FAMILY_PREFIX: &str = "hash_crc32";
const SPEC: HashWrapperSpec = HashWrapperSpec::new(OP_ID, CRC32_OP_ID, FAMILY_PREFIX, 1);

/// Build a Program that writes CRC-32(input[0..n]) to `out[0]`.
#[must_use]
pub fn crc32(input: &str, out: &str, n: u32) -> Program {
    let (input, out) = SPEC.scoped_standard_buffers(input, out);
    let primitive = crc32_program(&input, &out, n);
    SPEC.wrap_static_count(&input, &out, n, primitive)
}

#[cfg(test)]
fn cpu_ref(input: &[u8]) -> u32 {
    crc32_cpu_reference(input)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || crc32("input", "out", 3),
        test_inputs: Some(|| {
            let bytes = vyre_primitives::wire::pack_bytes_as_u32_slice(b"abc");
            vec![vec![bytes]]
        }),
        // Canonical CRC-32 of "abc" (reflected poly 0xEDB88320) = 0x352441c2.
        expected_output: Some(|| vec![vec![0x352441c2u32.to_le_bytes().to_vec()]]),
        category: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_reference::value::Value;

    fn run(bytes: &[u8]) -> u32 {
        let words = bytes
            .iter()
            .map(|&byte| u32::from(byte))
            .collect::<Vec<_>>();
        run_words(&words)
    }

    fn run_words(words: &[u32]) -> u32 {
        let n = words.len().max(1) as u32;
        let program = crc32("input", "out", n);
        let input = vyre_primitives::wire::pack_u32_slice(words);
        let inputs = vec![Value::Bytes(input.into())];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: crc32 must run; restore this invariant before continuing.");
        let raw = outputs[0].to_bytes();
        vyre_primitives::wire::read_u32_le_word(&raw, 0, "crc32 output")
            .expect("Fix: crc32 output must contain one u32.")
    }

    #[test]
    fn abc_matches_ref() {
        assert_eq!(run(b"abc"), 0x352441c2);
        assert_eq!(run(b"abc"), cpu_ref(b"abc"));
    }

    #[test]
    fn canonical_check_value() {
        assert_eq!(run(b"123456789"), 0xcbf43926);
    }

    #[test]
    fn random_64_bytes_match_ref() {
        let bytes: Vec<u8> = (0u8..64).collect();
        assert_eq!(run(&bytes), cpu_ref(&bytes));
    }

    #[test]
    fn high_bits_in_packed_slots_are_ignored() {
        let words = [0xFFFF_FF61, 0xCAFE_0062, 0x8000_0063];
        assert_eq!(run_words(&words), cpu_ref(b"abc"));
    }

    #[test]
    fn generated_crc32_wrapper_matches_primitive_for_4096_packed_cases() {
        let mut assertions = 0usize;
        for seed in 0u32..4096 {
            let len = ((seed.wrapping_mul(37) ^ seed.rotate_left(11)) % 96 + 1) as usize;
            let mut words = Vec::with_capacity(len);
            let mut bytes = Vec::with_capacity(len);
            let mut state = seed ^ 0xA5A5_5A5A;
            for index in 0..len {
                state = state
                    .wrapping_mul(1_664_525)
                    .wrapping_add(1_013_904_223)
                    .rotate_left((index as u32) & 15);
                let byte = (state ^ (seed << (index & 7))) as u8;
                let hostile_high_bits = state & 0xFFFF_FF00;
                words.push(hostile_high_bits | u32::from(byte));
                bytes.push(byte);
            }

            assert_eq!(
                run_words(&words),
                cpu_ref(&bytes),
                "Fix: generated CRC32 wrapper case {seed} must ignore packed-slot high bits and match the primitive CPU authority."
            );
            assertions += 1;
        }
        assert_eq!(assertions, 4096);
    }

    #[test]
    fn wrapper_delegates_to_primitive_crc32_region() {
        let program = crc32("input", "out", 3);
        let [vyre::ir::Node::Region { body, .. }] = program.entry() else {
            panic!("expected one top-level CRC32 wrapper region");
        };
        let [vyre::ir::Node::Region { generator, .. }] = body.as_ref().as_slice() else {
            panic!("expected CRC32 wrapper to contain one primitive child region");
        };
        assert_eq!(generator.as_str(), CRC32_OP_ID);
    }

    #[test]
    fn wrapper_source_does_not_fork_crc32_algorithm() {
        let source = include_str!("crc32.rs");

        assert!(source.contains("crc32_program"));
        assert!(source.contains("crc32_cpu_reference"));
        assert!(!source.contains(concat!("CRC32", "_POLY")));
        assert!(!source.contains(concat!("build", "_table")));
        assert!(!source.contains(concat!("crc32_update", "_byte_state")));
        assert!(!source.contains(concat!("loop", "_for(")));
    }

    #[test]
    fn generic_default_names_are_family_scoped() {
        let program = crc32("input", "out", 4);
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
