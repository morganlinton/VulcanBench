//! Handwritten oracle matrix for `reduce::radix_sort`.
//!
//! Compares production radix sort against an independent stable masked-key
//! sort oracle across hostile lengths, bit widths, and LCG seeds.

#![forbid(unsafe_code)]
#![cfg(feature = "cpu-parity")]

type RadixSort = fn(&[u32], u32) -> Vec<u32>;
type RadixSortInto = fn(&[u32], u32, &mut Vec<u32>, &mut Vec<u32>);

#[test]
fn radix_sort_matches_stable_masked_sort_oracle_matrix() {
    assert_radix_sort(
        "radix_sort",
        vyre_primitives::reduce::radix_sort::cpu_ref,
        vyre_primitives::reduce::radix_sort::cpu_ref_into,
        oracle_stable_masked_sort,
    );
}

fn assert_radix_sort(
    name: &str,
    actual: RadixSort,
    actual_into: RadixSortInto,
    expected: RadixSort,
) {
    let cases = radix_cases();
    for (case_idx, (input, bits)) in cases.iter().enumerate() {
        let expected_out = expected(input, *bits);
        assert_eq!(
            actual(input, *bits),
            expected_out,
            "Fix: {name} adversarial case {case_idx} len={} bits={bits} must match the independent oracle.",
            input.len()
        );

        let mut out = vec![0xDEAD_BEEF; input.len().saturating_add(13)];
        let mut scratch = vec![0xCAFE_BABE; input.len().saturating_add(13)];
        actual_into(input, *bits, &mut out, &mut scratch);
        assert_eq!(
            out, expected_out,
            "Fix: {name} cpu_ref_into adversarial case {case_idx} must clear stale output before writing."
        );
    }
}

fn oracle_stable_masked_sort(input: &[u32], bits: u32) -> Vec<u32> {
    let bits = bits.min(32);
    if input.is_empty() || bits == 0 {
        return input.to_vec();
    }
    let mask = if bits == 32 {
        u32::MAX
    } else {
        (1u32 << bits) - 1
    };
    let mut out = vec![0; input.len()];
    for (index, &key) in input.iter().enumerate() {
        let masked = key & mask;
        let rank = input
            .iter()
            .enumerate()
            .filter(|(other_index, &other_key)| {
                let other_masked = other_key & mask;
                other_masked < masked || (other_masked == masked && *other_index < index)
            })
            .count();
        out[rank] = key;
    }
    out
}

fn radix_cases() -> Vec<(Vec<u32>, u32)> {
    let mut cases = Vec::new();
    let lengths = [0usize, 1, 32, 257, 1024];
    let fills = [0u32, 1, u32::MAX, 0xDEAD_BEEF];
    let bit_widths = [0u32, 1, 8, 16, 32, 33];

    for len in lengths {
        for fill in fills {
            for bits in bit_widths {
                cases.push((vec![fill; len], bits));
            }
        }
        for bits in bit_widths {
            cases.push((ramp(len, 0), bits));
            cases.push((ramp(len, u32::MAX), bits));
            cases.push((alternating(len, 0, u32::MAX), bits));
            cases.push((high_low_byte_pattern(len), bits));
        }
    }

    for seed in [0x0000_0001, 0xDEAD_BEEF, 0xFFFF_FFFE] {
        for len in lengths {
            let input = lcg(seed, len);
            for bits in bit_widths {
                cases.push((input.clone(), bits));
            }
        }
    }

    for case in 0..4096usize {
        let len = case % 129;
        let bits = match case % 8 {
            0 => 0,
            1 => 1,
            2 => 4,
            3 => 8,
            4 => 16,
            5 => 24,
            6 => 32,
            _ => 33,
        };
        let input = lcg(case as u32 ^ 0x51AF_0D00, len);
        cases.push((input, bits));
    }

    cases
}

fn high_low_byte_pattern(len: usize) -> Vec<u32> {
    (0..len)
        .map(|idx| {
            let lo = (idx % 256) as u32;
            let hi = ((idx / 256) % 256) as u32;
            (hi << 16) | lo
        })
        .collect()
}

fn ramp(len: usize, start: u32) -> Vec<u32> {
    (0..len)
        .map(|idx| start.wrapping_add((idx as u32).wrapping_mul(0x9E37_79B9)))
        .collect()
}

fn alternating(len: usize, even: u32, odd: u32) -> Vec<u32> {
    (0..len)
        .map(|idx| if idx % 2 == 0 { even } else { odd })
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
