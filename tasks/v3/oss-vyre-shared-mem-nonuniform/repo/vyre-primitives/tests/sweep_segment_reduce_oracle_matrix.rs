//! Handwritten oracle matrix for `reduce::segment_reduce` (per-segment sum).
//!
//! Compares production `cpu_ref` / `cpu_ref_into` against an independent
//! wrapping-sum oracle across hostile input lengths, edge values, and LCG seeds.

#![forbid(unsafe_code)]
#![cfg(feature = "cpu-parity")]

type SegmentReduce = fn(&[u32], &[u32]) -> Vec<u32>;
type SegmentReduceInto = fn(&[u32], &[u32], &mut Vec<u32>);

#[test]
fn segment_reduce_sum_matches_independent_oracle_matrix() {
    assert_segment_reduce(
        "segment_reduce_sum",
        vyre_primitives::reduce::segment_reduce::cpu_ref,
        vyre_primitives::reduce::segment_reduce::cpu_ref_into,
        oracle_segment_sum,
    );
}

fn assert_segment_reduce(
    name: &str,
    actual: SegmentReduce,
    actual_into: SegmentReduceInto,
    expected: SegmentReduce,
) {
    for (case_idx, (input, offsets)) in segment_cases().enumerate() {
        let expected_out = expected(&input, &offsets);
        assert_eq!(
            actual(&input, &offsets),
            expected_out,
            "Fix: {name} adversarial case {case_idx} input_len={} segments={} must match the independent oracle.",
            input.len(),
            offsets.len().saturating_sub(1)
        );

        let mut reused = vec![0xBADC0FFE; expected_out.len().saturating_add(17)];
        actual_into(&input, &offsets, &mut reused);
        assert_eq!(
            reused, expected_out,
            "Fix: {name} cpu_ref_into adversarial case {case_idx} must clear stale output capacity before writing."
        );
    }
}

fn oracle_segment_sum(input: &[u32], segment_offsets: &[u32]) -> Vec<u32> {
    let num_segments = segment_offsets
        .len()
        .checked_sub(1)
        .expect("oracle segment_offsets must be CSR-style with at least one boundary");
    let mut out = Vec::with_capacity(num_segments);
    for seg in 0..num_segments {
        let start = segment_offsets[seg] as usize;
        let end = segment_offsets[seg + 1] as usize;
        assert!(
            start <= end && end <= input.len(),
            "oracle segment {seg}: start={start}, end={end}, input_len={}",
            input.len()
        );
        let sum = input[start..end]
            .iter()
            .copied()
            .fold(0u32, u32::wrapping_add);
        out.push(sum);
    }
    out
}

fn segment_cases() -> impl Iterator<Item = (Vec<u32>, Vec<u32>)> {
    let lengths = [0usize, 1, 32, 257];
    let fills = [0u32, 1, u32::MAX, 0x8000_0000, 0xDEAD_BEEF];
    let fixed = lengths.into_iter().flat_map(move |len| {
        fills.into_iter().flat_map(move |fill| {
            [
                (vec![fill; len], single_segment_offsets(len)),
                (vec![fill; len], alternating_empty_segments(len)),
                (ramp(len, fill), uniform_segment_offsets(len, 4)),
            ]
            .into_iter()
        })
    });
    let generated = (0..8192usize).map(|case| {
        let len = match case % 16 {
            0 => 0,
            1 => 1,
            2 => 32,
            3 => 257,
            _ => case % 129,
        };
        let seg_count = 1 + (case % 17);
        let input = lcg(case as u32 ^ 0x9E37_79B9, len);
        let offsets = random_valid_offsets(case as u64 ^ 0xD00D_F00D, len, seg_count);
        (input, offsets)
    });
    fixed.chain(generated)
}

fn single_segment_offsets(len: usize) -> Vec<u32> {
    vec![0, len as u32]
}

fn alternating_empty_segments(len: usize) -> Vec<u32> {
    let mut offsets = vec![0];
    let mut pos = 0usize;
    while pos < len {
        let step = if pos % 3 == 0 { 0 } else { 1 + (pos % 5) };
        if step == 0 {
            pos += 1;
        } else {
            pos = (pos + step).min(len);
        }
        offsets.push(pos as u32);
    }
    if offsets.last().copied() != Some(len as u32) {
        offsets.push(len as u32);
    }
    offsets
}

fn uniform_segment_offsets(len: usize, width: usize) -> Vec<u32> {
    let width = width.max(1);
    let mut offsets = vec![0];
    let mut pos = 0usize;
    while pos < len {
        pos = (pos + width).min(len);
        offsets.push(pos as u32);
    }
    if offsets.last().copied() != Some(len as u32) {
        offsets.push(len as u32);
    }
    offsets
}

fn random_valid_offsets(seed: u64, len: usize, seg_count: usize) -> Vec<u32> {
    let seg_count = seg_count.max(1);
    let mut rng = seed;
    let mut cuts = vec![0usize];
    for _ in 1..seg_count {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1);
        cuts.push(if len == 0 {
            0
        } else {
            (rng as usize) % (len + 1)
        });
    }
    cuts.push(len);
    cuts.sort_unstable();
    cuts.dedup();
    if cuts.last().copied() != Some(len) {
        cuts.push(len);
    }
    cuts.into_iter().map(|v| v as u32).collect()
}

fn ramp(len: usize, start: u32) -> Vec<u32> {
    (0..len)
        .map(|idx| start.wrapping_add((idx as u32).wrapping_mul(0x9E37_79B9)))
        .collect()
}

fn lcg(seed: u32, len: usize) -> Vec<u32> {
    let mut state = seed;
    (0..len)
        .map(|idx| {
            state = state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223)
                .rotate_left((idx % 31) as u32);
            state ^ (idx as u32).wrapping_mul(0x85EB_CA6B)
        })
        .collect()
}
