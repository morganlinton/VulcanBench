//! Cat-A `fnv1a64`  -  FNV-1a 64-bit hash.
//!
//! Reference spec:
//! ```text
//! hash = FNV_OFFSET_BASIS_64;       // 0xCBF29CE484222325
//! for byte in data:
//!     hash = (hash XOR byte) * FNV_PRIME_64;  // 0x00000100000001B3
//! ```
//!
//! The IR lacks native u64 arithmetic. We emulate u64 state as a
//! `(lo, hi)` u32 pair and perform the widening multiply by pieces.
//! Because the FNV prime is `(p_hi << 32) | p_lo` with
//! `p_lo = 0x01B3 < 2^16` and `p_hi = 0x0100 < 2^16`, every
//! sub-product `(u32) × (u32<2^16)` fits back in a u32 after
//! appropriate shifts. The widening multiply decomposes:
//!
//! ```text
//! result_lo = (h_lo * p_lo) mod 2^32
//! carry     = high-32-bits of the true (h_lo * p_lo)  // < 2^16
//! result_hi = (h_hi * p_lo + h_lo * p_hi + carry) mod 2^32
//! ```
//!
//! `carry` is computed by splitting `h_lo` into 16-bit halves.
//!
//! Output: two u32 slots, `out[0] = result_lo`, `out[1] = result_hi`.

use vyre::ir::Program;
use vyre_primitives::hash::fnv1a::{fnv1a64_program, fnv1a64_program_n, FNV1A64_OP_ID};

#[cfg(test)]
use crate::buffer_names::fixed_name;
#[cfg(test)]
use vyre_primitives::hash::fnv1a::fnv1a64 as cpu_ref_u64;

use super::wrap::HashWrapperSpec;

const OP_ID: &str = "vyre-libs::hash::fnv1a64";
const FAMILY_PREFIX: &str = "hash_fnv1a64";
const SPEC: HashWrapperSpec = HashWrapperSpec::new(OP_ID, FNV1A64_OP_ID, FAMILY_PREFIX, 2);

/// Build a Program that writes FNV-1a-64(input[0..]) as two u32
/// halves (low, high) to `out[0]` and `out[1]`.
#[must_use]
pub fn fnv1a64(input: &str, out: &str) -> Program {
    let (input, out) = SPEC.scoped_standard_buffers(input, out);
    let primitive = fnv1a64_program(&input, &out);
    SPEC.wrap_dynamic_count(&input, &out, primitive)
}

/// Build a Program that computes FNV-1a 64-bit over exactly `n` input slots.
#[must_use]
pub fn fnv1a64_n(input: &str, out: &str, n: u32) -> Program {
    let (input, out) = SPEC.scoped_standard_buffers(input, out);
    let primitive = fnv1a64_program_n(&input, &out, n);
    SPEC.wrap_static_count(&input, &out, n, primitive)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || fnv1a64_n("input", "out", 3),
        test_inputs: Some(|| {
            let bytes = vyre_primitives::wire::pack_bytes_as_u32_slice(b"abc");
            vec![vec![bytes]]
        }),
        // FNV-1a 64("abc") = 0xe71fa2190541574b (canonical test vector).
        // Written LE as [lo, hi] pair of u32s.
        expected_output: Some(|| {
            let hash: u64 = 0xe71f_a219_0541_574bu64;
            let bytes = hash.to_le_bytes().to_vec();
            vec![vec![bytes]]
        }),
        category: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::pack_bytes_as_u32;
    use vyre_reference::value::Value;

    fn run(bytes: &[u8]) -> u64 {
        let program = fnv1a64("input", "out");
        let inputs = vec![Value::Bytes(pack_bytes_as_u32(bytes).into())];
        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: fnv1a64 must run; restore this invariant before continuing.");
        let raw = outputs[0].to_bytes();
        let lo = vyre_primitives::wire::read_u32_le_word(&raw, 0, "fnv1a64 low output")
            .expect("Fix: fnv1a64 output must contain low u32.");
        let hi = vyre_primitives::wire::read_u32_le_word(&raw, 1, "fnv1a64 high output")
            .expect("Fix: fnv1a64 output must contain high u32.");
        (u64::from(hi) << 32) | u64::from(lo)
    }

    fn run_words(words: &[u32]) -> u64 {
        let program = fnv1a64_n("input", "out", words.len().max(1) as u32);
        let input = vyre_primitives::wire::pack_u32_slice(words);
        let outputs = vyre_reference::reference_eval(&program, &[Value::Bytes(input.into())])
            .expect("Fix: fnv1a64 must run on u32 byte slots.");
        let raw = outputs[0].to_bytes();
        let lo = vyre_primitives::wire::read_u32_le_word(&raw, 0, "fnv1a64 low word output")
            .expect("Fix: fnv1a64 word output must contain low u32.");
        let hi = vyre_primitives::wire::read_u32_le_word(&raw, 1, "fnv1a64 high word output")
            .expect("Fix: fnv1a64 word output must contain high u32.");
        (u64::from(hi) << 32) | u64::from(lo)
    }

    #[test]
    fn abc_matches_ref() {
        assert_eq!(run(b"abc"), cpu_ref_u64(b"abc"));
    }

    #[test]
    fn foobar_matches_known_vector() {
        assert_eq!(run(b"foobar"), 0x8594_4171_F739_67E8);
    }

    #[test]
    fn random_64_bytes_match_ref() {
        let bytes: Vec<u8> = (0u8..64).collect();
        assert_eq!(run(&bytes), cpu_ref_u64(&bytes));
    }

    #[test]
    fn random_512_bytes_match_ref() {
        // Stress the widening-multiply carry logic.
        let mut x: u32 = 0xDEAD_BEEF;
        let bytes: Vec<u8> = (0..512)
            .map(|_| {
                x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (x >> 24) as u8
            })
            .collect();
        assert_eq!(run(&bytes), cpu_ref_u64(&bytes));
    }

    #[test]
    fn high_input_bits_are_ignored() {
        let words = [0xFFFF_FF61, 0xCAFE_0062, 0x8000_0063];
        assert_eq!(run_words(&words), cpu_ref_u64(b"abc"));
    }

    #[test]
    fn wrapper_delegates_to_primitive_fnv1a64_region() {
        let program = fnv1a64_n("input", "out", 3);
        let [vyre::ir::Node::Region { body, .. }] = program.entry() else {
            panic!("expected one top-level FNV-1a64 wrapper region");
        };
        let [vyre::ir::Node::Region { generator, .. }] = body.as_ref().as_slice() else {
            panic!("expected FNV-1a64 wrapper to contain one primitive child region");
        };
        assert_eq!(generator.as_str(), FNV1A64_OP_ID);
    }

    #[test]
    fn generic_default_names_are_family_scoped() {
        let program = fnv1a64("input", "out");
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
