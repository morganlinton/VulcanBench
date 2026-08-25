//! Adversarial oracle tests for `reduce::segment_reduce`.

#![allow(unused_imports, dead_code, clippy::identity_op)]

use vyre_primitives::reduce::segment_reduce::*;

fn cpu_ref(input: &[u32], segment_offsets: &[u32]) -> Vec<u32> {
    let num_segments = segment_offsets
        .len()
        .checked_sub(1)
        .expect("segment_reduce_sum CPU oracle received empty segment_offsets. Fix: pass at least one CSR-style offset.");
    let mut out = Vec::with_capacity(num_segments);
    for segment in 0..num_segments {
        let start = segment_offsets[segment] as usize;
        let end = segment_offsets[segment + 1] as usize;
        assert!(
            start <= end && end <= input.len(),
            "segment_reduce_sum CPU oracle received malformed segment {segment}: start={start}, end={end}, input_len={}. Fix: rebuild monotonic in-bounds segment offsets before parity comparison.",
            input.len()
        );
        out.push(input[start..end].iter().copied().fold(0, u32::wrapping_add));
    }
    out
}

#[test]
fn segment_reduce_hostile_corpus() {
    let cases: &[(&[u32], &[u32], &[u32])] = &[
        (&[], &[0], &[]),
        (&[1, 2, 3], &[0, 3], &[6]),
        (&[10, 20, 30, 40], &[0, 2, 4], &[30, 70]),
        (&[0xffff_ffff, 1], &[0, 1, 2], &[0xffff_ffff, 1]),
    ];
    for (idx, (input, offsets, expected)) in cases.iter().enumerate() {
        assert_eq!(
            cpu_ref(input, offsets),
            *expected,
            "Fix: segment_reduce oracle mismatch on case {idx}"
        );
    }
}

#[test]
#[should_panic(expected = "malformed segment")]
fn segment_reduce_rejects_non_monotonic_offsets() {
    let _ = cpu_ref(&[1, 2, 3], &[0, 3, 2]);
}
