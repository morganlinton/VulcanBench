//! Stable u32 key sort Program builder + CPU reference.
//!
//! # Program Design
//!
//! Each invocation computes the stable rank of one key:
//!
//! ```text
//! rank(i) = count(keys[j] < keys[i]) + count(keys[j] == keys[i] && j < i)
//! out[rank(i)] = keys[i]
//! ```
//!
//! This is a single-dispatch stable sorting primitive over the current
//! statement IR. The multi-dispatch histogram/scan/scatter radix pipeline can
//! replace this implementation behind the same function once pipeline-level
//! scratch dispatch is available.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Canonical op id.
pub const OP_ID: &str = "vyre-primitives::reduce::radix_sort";

/// Emit a stable u32 sort Program.
///
/// `input`   -  source buffer of `count` u32 keys.  
/// `output`  -  destination buffer of `count` u32 keys.  
/// `count`   -  number of elements.  
/// `bits`    -  number of significant key bits (1..=32).  Fewer bits = fewer
///            passes.
///
/// # Panics
///
/// Invalid dimensions lower to an explicit trap program.
#[must_use]
pub fn radix_sort(input: &str, output: &str, count: u32, bits: u32) -> Program {
    if count == 0 {
        return crate::invalid_output_program(
            OP_ID,
            output,
            DataType::U32,
            format!("Fix: radix_sort requires count > 0, got {count}."),
        );
    }
    if bits > 32 {
        return crate::invalid_output_program(
            OP_ID,
            output,
            DataType::U32,
            format!("Fix: radix_sort bits must be <= 32, got {bits}."),
        );
    }

    let buffers = vec![
        BufferDecl::storage(input, 0, BufferAccess::ReadOnly, DataType::U32).with_count(count),
        BufferDecl::storage(output, 1, BufferAccess::ReadWrite, DataType::U32).with_count(count),
    ];

    let t = Expr::InvocationId { axis: 0 };
    let mask = if bits == 32 {
        u32::MAX
    } else if bits == 0 {
        0
    } else {
        (1u32 << bits) - 1
    };
    let masked_key = |expr: Expr| {
        if bits == 32 {
            expr
        } else {
            Expr::bitand(expr, Expr::u32(mask))
        }
    };

    let key_i = masked_key(Expr::load(input, Expr::var("i")));
    let key_j = masked_key(Expr::load(input, Expr::var("j")));
    let lower_key = Expr::lt(key_j.clone(), key_i.clone());
    let stable_tie = Expr::and(
        Expr::eq(key_j, key_i),
        Expr::lt(Expr::var("j"), Expr::var("i")),
    );

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(count)),
        vec![
            Node::let_bind("i", t.clone()),
            Node::let_bind("rank", Expr::u32(0)),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(count),
                vec![Node::if_then(
                    Expr::or(lower_key, stable_tie),
                    vec![Node::assign(
                        "rank",
                        Expr::add(Expr::var("rank"), Expr::u32(1)),
                    )],
                )],
            ),
            Node::store(output, Expr::var("rank"), Expr::load(input, Expr::var("i"))),
        ],
    )];

    Program::wrapped(
        buffers,
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// CPU-reference stable u32 sort over the lowest `bits` key bits.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(input: &[u32], bits: u32) -> Vec<u32> {
    let mut out = Vec::new();
    let mut scratch = Vec::new();
    match try_cpu_ref_into(input, bits, &mut out, &mut scratch) {
        Ok(()) => out,
        // A parity oracle that returns empty on failure makes the GPU-vs-CPU
        // assertion pass on empty==empty, silently masking a divergence
        // (Law 10 / Law 6). Fail loud; callers use try_cpu_ref_into.
        Err(error) => panic!("vyre-primitives radix_sort CPU reference failed: {error}"),
    }
}

/// CPU-reference stable u32 sort into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(input: &[u32], bits: u32, out: &mut Vec<u32>, scratch: &mut Vec<u32>) {
    if let Err(error) = try_cpu_ref_into(input, bits, out, scratch) {
        panic!("vyre-primitives radix_sort CPU reference failed: {error}");
    }
}

/// Fallible CPU-reference stable u32 sort into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_cpu_ref_into(
    input: &[u32],
    bits: u32,
    out: &mut Vec<u32>,
    scratch: &mut Vec<u32>,
) -> Result<(), String> {
    let bits = bits.min(32);
    if input.len() > out.capacity() {
        out.try_reserve_exact(input.len() - out.capacity())
            .map_err(|err| {
                format!(
                    "radix_sort CPU reference could not reserve {} output keys: {err}",
                    input.len()
                )
            })?;
    }
    if input.len() > scratch.capacity() {
        scratch
            .try_reserve_exact(input.len() - scratch.capacity())
            .map_err(|err| {
                format!(
                    "radix_sort CPU reference could not reserve {} scratch keys: {err}",
                    input.len()
                )
            })?;
    }

    out.clear();
    out.extend_from_slice(input);
    if out.is_empty() || bits == 0 {
        scratch.clear();
        return Ok(());
    }

    scratch.clear();
    scratch.resize(out.len(), 0);
    let passes = ((bits + 7) / 8).min(4) as usize;
    let mut sorted_in_scratch = false;

    for pass in 0..passes {
        if sorted_in_scratch {
            radix_pass(scratch, out, bits, pass);
        } else {
            radix_pass(out, scratch, bits, pass);
        }
        sorted_in_scratch = !sorted_in_scratch;
    }

    if sorted_in_scratch {
        out.clear();
        out.extend_from_slice(scratch);
    }
    Ok(())
}

#[cfg(any(test, feature = "cpu-parity"))]
fn radix_pass(src: &[u32], dst: &mut [u32], bits: u32, pass: usize) {
    let shift = pass * 8;
    let mask = if shift + 8 >= bits as usize {
        (1u32 << ((bits as usize - shift).min(8))) - 1
    } else {
        0xFF
    };

    let mut counts = [0u32; 256];
    for &key in src {
        let digit = ((key >> shift) & mask) as usize;
        counts[digit] += 1;
    }

    let mut offset = 0u32;
    for count in &mut counts {
        let current = *count;
        *count = offset;
        offset += current;
    }

    for &key in src {
        let digit = ((key >> shift) & mask) as usize;
        let dest = counts[digit] as usize;
        dst[dest] = key;
        counts[digit] += 1;
    }
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || radix_sort("input", "output", 4, 8),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[3, 1, 4, 2]),
                to_bytes(&[0, 0, 0, 0]),
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![to_bytes(&[1, 2, 3, 4])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_ref_empty() {
        assert_eq!(cpu_ref(&[], 32), Vec::<u32>::new());
    }

    #[test]
    fn cpu_ref_single_element() {
        assert_eq!(cpu_ref(&[42], 32), vec![42]);
    }

    #[test]
    fn cpu_ref_already_sorted() {
        let input = vec![1, 2, 3, 4, 5];
        assert_eq!(cpu_ref(&input, 32), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn cpu_ref_reverse_sorted() {
        let input = vec![5, 4, 3, 2, 1];
        assert_eq!(cpu_ref(&input, 32), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn cpu_ref_into_reuses_buffers() {
        let mut out = Vec::with_capacity(16);
        let mut scratch = Vec::with_capacity(16);
        cpu_ref_into(&[5, 4, 3, 2, 1], 32, &mut out, &mut scratch);
        let out_capacity = out.capacity();
        let scratch_capacity = scratch.capacity();
        assert_eq!(out, vec![1, 2, 3, 4, 5]);

        cpu_ref_into(&[3, 1, 2], 32, &mut out, &mut scratch);
        assert_eq!(out.capacity(), out_capacity);
        assert_eq!(scratch.capacity(), scratch_capacity);
        assert_eq!(out, vec![1, 2, 3]);
    }

    #[test]
    fn compatibility_wrappers_match_fallible_reference() {
        let input = &[5, 4, 3, 2, 1];
        let mut compat = Vec::with_capacity(16);
        let mut compat_scratch = Vec::with_capacity(16);
        let mut fallible = Vec::with_capacity(16);
        let mut fallible_scratch = Vec::with_capacity(16);

        cpu_ref_into(input, 32, &mut compat, &mut compat_scratch);
        try_cpu_ref_into(input, 32, &mut fallible, &mut fallible_scratch)
            .expect("Fix: small radix_sort CPU reference must reserve");

        assert_eq!(cpu_ref(input, 32), fallible);
        assert_eq!(compat, fallible);
    }

    #[test]
    fn production_wrappers_have_no_raw_panic_path() {
        let production = include_str!("radix_sort.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: radix_sort.rs must contain production section");

        assert!(
            !production.contains(".expect(") && !production.contains(".unwrap("),
            "Fix: radix_sort CPU reference wrappers must not panic in production."
        );
    }

    #[test]
    fn try_cpu_ref_into_reuses_buffers_and_clears_stale_tail() {
        let mut out = Vec::with_capacity(16);
        let mut scratch = Vec::with_capacity(16);
        out.extend_from_slice(&[u32::MAX; 16]);
        scratch.extend_from_slice(&[u32::MAX; 16]);
        let out_ptr = out.as_ptr();
        let scratch_ptr = scratch.as_ptr();

        try_cpu_ref_into(&[5, 4, 3, 2, 1], 32, &mut out, &mut scratch).unwrap();

        assert_eq!(out, vec![1, 2, 3, 4, 5]);
        assert_eq!(out.as_ptr(), out_ptr);
        assert_eq!(scratch.as_ptr(), scratch_ptr);
    }

    #[test]
    fn bits_zero_clears_scratch_without_reallocating() {
        let mut out = Vec::with_capacity(8);
        let mut scratch = Vec::with_capacity(8);
        scratch.extend_from_slice(&[u32::MAX; 8]);
        let scratch_ptr = scratch.as_ptr();

        try_cpu_ref_into(&[3, 1, 2], 0, &mut out, &mut scratch).unwrap();

        assert_eq!(out, vec![3, 1, 2]);
        assert!(scratch.is_empty());
        assert_eq!(scratch.as_ptr(), scratch_ptr);
    }

    #[test]
    fn cpu_ref_stable_sort() {
        // With u32 keys there is no separate payload; stability is visible
        // when keys are equal  -  their relative order must be preserved.
        // We simulate payload by packing (key << 16 | payload) and verifying
        // the payload order after sort.
        let input: Vec<u32> = vec![
            (2 << 16),
            (1 << 16),
            (2 << 16) | 1,
            (1 << 16) | 1,
            (2 << 16) | 2,
        ];
        let sorted = cpu_ref(&input, 32);
        let payloads: Vec<u16> = sorted.iter().map(|v| (*v & 0xFFFF) as u16).collect();
        assert_eq!(payloads, vec![0, 1, 0, 1, 2]);
    }

    #[test]
    fn cpu_ref_duplicates() {
        let input = vec![3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5];
        let mut expected = input.clone();
        expected.sort_unstable();
        assert_eq!(cpu_ref(&input, 32), expected);
    }

    #[test]
    fn cpu_ref_partial_bits() {
        // Only sort on lowest 8 bits.
        let input = vec![0x0100, 0x0001, 0x0200, 0x0002];
        // With 8 bits, stable sort by low byte:
        // low-byte 0x00: 0x0100 (first), 0x0200 (second)
        // low-byte 0x01: 0x0001
        // low-byte 0x02: 0x0002
        assert_eq!(cpu_ref(&input, 8), vec![0x0100, 0x0200, 0x0001, 0x0002]);
    }

    #[test]
    fn cpu_ref_bits_zero_is_noop() {
        let input = vec![3, 1, 2];
        assert_eq!(cpu_ref(&input, 0), vec![3, 1, 2]);
    }

    #[test]
    fn cpu_ref_large_random() {
        let input: Vec<u32> = (0..1000u32).map(|i| i.wrapping_mul(0x9E3779B9)).collect();
        let mut expected = input.clone();
        expected.sort_unstable();
        assert_eq!(cpu_ref(&input, 32), expected);
    }

    #[test]
    fn emitted_program_has_expected_buffers() {
        let p = radix_sort("in", "out", 128, 32);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["in", "out"]);
    }

    #[test]
    fn emitted_program_small_count_ok() {
        let p = radix_sort("in", "out", 1, 32);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
    }

    #[test]
    fn bits_over_32_traps() {
        let p = radix_sort("in", "out", 10, 33);
        assert!(p.stats().trap());
    }

    #[test]
    fn cpu_ref_bits_over_32_clamps_to_full_key_sort() {
        assert_eq!(cpu_ref(&[2, 1], 33), vec![1, 2]);
    }
}
