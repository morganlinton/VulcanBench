//! Cat-A `multi_hash`  -  CRC-32 + FNV-1a 32-bit + Adler-32 in one pass.
//!
//! Single lane-0 guarded walk over `input[0..]`.  Each iteration updates
//! all three hash states using the same loaded byte, so the buffer is
//! walked exactly once.

use vyre::ir::Program;
use vyre_primitives::hash::multi_hash::{multi_hash_program, MULTI_HASH_OP_ID};

#[cfg(test)]
use crate::buffer_names::fixed_name;

use super::wrap::{scoped_input_buffer, HashWrapperSpec};

const OP_ID: &str = "vyre-libs::hash::multi_hash";
const FAMILY_PREFIX: &str = "hash_multi";
const SPEC: HashWrapperSpec = HashWrapperSpec::new(OP_ID, MULTI_HASH_OP_ID, FAMILY_PREFIX, 3);

/// Build a Program that computes CRC-32, FNV-1a 32-bit, and Adler-32 over
/// `input[0..n]` in a single walk.
///
/// `input[i]` packs one byte per u32 slot. The three results are packed into
/// one ABI-legal output buffer:
/// `out[0] = crc32`, `out[1] = fnv1a32`, `out[2] = adler32`.
#[must_use]
pub fn multi_hash(
    input: &str,
    out_crc32: &str,
    out_fnv1a32: &str,
    out_adler32: &str,
    n: u32,
) -> Program {
    let input = scoped_input_buffer(FAMILY_PREFIX, input);
    let out_crc32 = SPEC.scoped_output_buffer_with_aliases(
        out_crc32,
        &[
            "out",
            "output",
            "crc32",
            "out_crc32",
            "multi_hash",
            "out_multi_hash",
        ],
    );
    let _legacy_output_names = (out_fnv1a32, out_adler32);
    let primitive = multi_hash_program(&input, &out_crc32, n);
    SPEC.wrap_static_count(&input, &out_crc32, n, primitive)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || multi_hash("input", "out_crc32", "out_fnv1a32", "out_adler32", 3),
        test_inputs: Some(|| {
            let bytes = vyre_primitives::wire::pack_bytes_as_u32_slice(b"abc");
            vec![vec![bytes]]
        }),
        expected_output: Some(|| vec![vec![vyre_primitives::wire::pack_u32_slice(&[
            0x3524_41c2,
            0x1a47_e90b,
            0x024D_0127,
        ])]]),
        category: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::{adler32, crc32, fnv1a32, pack_bytes_as_u32};
    use vyre_reference::value::Value;

    fn run_multi(bytes: &[u8]) -> (u32, u32, u32) {
        let n = bytes.len().max(1) as u32;
        let program = multi_hash("input", "out_crc32", "out_fnv1a32", "out_adler32", n);
        let mut input_bytes = pack_bytes_as_u32(bytes);
        input_bytes.resize(n as usize * 4, 0);
        let inputs = vec![Value::Bytes(input_bytes.into())];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: multi_hash must run; restore this invariant before continuing.");
        assert_eq!(
            outputs.len(),
            1,
            "multi_hash must expose one packed output buffer"
        );
        let raw = outputs[0].to_bytes();
        let crc = vyre_primitives::wire::read_u32_le_word(&raw, 0, "multi-hash crc32 output")
            .expect("Fix: multi_hash output must contain crc32 word.");
        let fnv = vyre_primitives::wire::read_u32_le_word(&raw, 1, "multi-hash fnv1a32 output")
            .expect("Fix: multi_hash output must contain fnv1a32 word.");
        let adler = vyre_primitives::wire::read_u32_le_word(&raw, 2, "multi-hash adler32 output")
            .expect("Fix: multi_hash output must contain adler32 word.");
        (crc, fnv, adler)
    }

    fn run_crc32(bytes: &[u8]) -> u32 {
        let n = bytes.len().max(1) as u32;
        let program = crc32("input", "out", n);
        let mut input_bytes = pack_bytes_as_u32(bytes);
        input_bytes.resize(n as usize * 4, 0);
        let inputs = vec![
            Value::Bytes(input_bytes.into()),
            Value::Bytes(vec![0u8; 4].into()),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: crc32 must run; restore this invariant before continuing.");
        let raw = outputs[0].to_bytes();
        vyre_primitives::wire::read_u32_le_word(&raw, 0, "multi-hash crc32 output")
            .expect("Fix: crc32 output must contain one u32.")
    }

    fn run_fnv1a32(bytes: &[u8]) -> u32 {
        let program = fnv1a32("input", "out");
        let mut input_bytes = pack_bytes_as_u32(bytes);
        input_bytes.resize(input_bytes.len().max(4), 0);
        let inputs = vec![
            Value::Bytes(input_bytes.into()),
            Value::Bytes(vec![0u8; 4].into()),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: fnv1a32 must run; restore this invariant before continuing.");
        let raw = outputs[0].to_bytes();
        vyre_primitives::wire::read_u32_le_word(&raw, 0, "multi-hash fnv1a32 output")
            .expect("Fix: fnv1a32 output must contain one u32.")
    }

    fn run_adler32(bytes: &[u8]) -> u32 {
        let n = bytes.len().max(1) as u32;
        let program = adler32("input", "out", n);
        let mut input_bytes = pack_bytes_as_u32(bytes);
        input_bytes.resize(n as usize * 4, 0);
        let inputs = vec![
            Value::Bytes(input_bytes.into()),
            Value::Bytes(vec![0u8; 4].into()),
        ];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: adler32 must run; restore this invariant before continuing.");
        let raw = outputs[0].to_bytes();
        vyre_primitives::wire::read_u32_le_word(&raw, 0, "multi-hash adler32 output")
            .expect("Fix: adler32 output must contain one u32.")
    }

    #[test]
    fn abc_matches_expected() {
        let (crc, fnv, adler) = run_multi(b"abc");
        assert_eq!(crc, 0x3524_41c2);
        assert_eq!(fnv, 0x1a47_e90b);
        assert_eq!(adler, 0x024D_0127);
    }

    #[test]
    fn parity_with_individual_hashes_empty() {
        let bytes: Vec<u8> = vec![];
        let (crc, fnv, adler) = run_multi(&bytes);
        assert_eq!(crc, run_crc32(&bytes));
        assert_eq!(fnv, run_fnv1a32(&bytes));
        assert_eq!(adler, run_adler32(&bytes));
    }

    #[test]
    fn parity_with_individual_hashes_random() {
        for len in [1, 7, 64, 255, 1024] {
            let bytes: Vec<u8> = (0..len)
                .map(|i| (i as u8).wrapping_mul(7).wrapping_add(13))
                .collect();
            let (crc, fnv, adler) = run_multi(&bytes);
            assert_eq!(crc, run_crc32(&bytes), "crc32 mismatch at len {}", len);
            assert_eq!(fnv, run_fnv1a32(&bytes), "fnv1a32 mismatch at len {}", len);
            assert_eq!(
                adler,
                run_adler32(&bytes),
                "adler32 mismatch at len {}",
                len
            );
        }
    }

    #[test]
    fn crc_lane_ignores_high_input_bits_like_primitive_crc32() {
        let program = multi_hash("input", "out_crc32", "out_fnv1a32", "out_adler32", 1);
        let inputs = vec![Value::Bytes(0xFFFF_FF61u32.to_le_bytes().to_vec().into())];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: multi_hash must run on high-bit-polluted u32 byte slots.");
        let raw = outputs[0].to_bytes();
        let crc = vyre_primitives::wire::read_u32_le_word(&raw, 0, "multi-hash crc output")
            .expect("Fix: multi_hash crc output must contain one u32.");

        assert_eq!(crc, run_crc32(b"a"));
    }

    #[test]
    fn generated_crc_lane_masks_high_bits_for_polluted_u32_slots() {
        let program = multi_hash("input", "out_crc32", "out_fnv1a32", "out_adler32", 4);
        for seed in 0u32..64 {
            let logical = [
                seed as u8,
                seed.wrapping_mul(3) as u8,
                seed.wrapping_add(17) as u8,
                seed.wrapping_mul(11).wrapping_add(9) as u8,
            ];
            let polluted = logical
                .iter()
                .enumerate()
                .flat_map(|(index, byte)| {
                    (0xA5A5_0000u32 | ((index as u32) << 12) | u32::from(*byte)).to_le_bytes()
                })
                .collect::<Vec<_>>();
            let inputs = vec![Value::Bytes(polluted.into())];
            let outputs = vyre_reference::reference_eval(&program, &inputs).unwrap_or_else(|error| {
                panic!("Fix: multi_hash must run for generated polluted u32 byte slots at seed={seed}: {error}")
            });
            let raw = outputs[0].to_bytes();
            let crc = vyre_primitives::wire::read_u32_le_word(&raw, 0, "multi-hash crc32 output")
                .expect("Fix: generated multi_hash output must contain crc32 word.");
            let fnv = vyre_primitives::wire::read_u32_le_word(&raw, 1, "multi-hash fnv1a32 output")
                .expect("Fix: generated multi_hash output must contain fnv1a32 word.");
            let adler =
                vyre_primitives::wire::read_u32_le_word(&raw, 2, "multi-hash adler32 output")
                    .expect("Fix: generated multi_hash output must contain adler32 word.");

            assert_eq!(
                crc,
                run_crc32(&logical),
                "crc32 high-bit masking mismatch at seed {seed}"
            );
            assert_eq!(
                fnv,
                run_fnv1a32(&logical),
                "fnv1a32 high-bit masking mismatch at seed {seed}"
            );
            assert_eq!(
                adler,
                run_adler32(&logical),
                "adler32 high-bit masking mismatch at seed {seed}"
            );
        }
    }

    #[test]
    fn generic_default_names_are_family_scoped() {
        let program = multi_hash("input", "out_crc32", "out_fnv1a32", "out_adler32", 4);
        assert_eq!(
            program.buffers()[0].name(),
            fixed_name(FAMILY_PREFIX, "input")
        );
        assert_eq!(
            program.buffers()[1].name(),
            fixed_name(FAMILY_PREFIX, "out")
        );
    }

    #[test]
    fn declares_single_packed_output_buffer_for_backend_abi() {
        let program = multi_hash("input", "out_crc32", "out_fnv1a32", "out_adler32", 4);
        assert_eq!(
            program.buffers().len(),
            2,
            "multi_hash must expose input plus one packed output buffer"
        );
        assert_eq!(program.buffers()[1].count(), 3);
    }
}
