//! Generated semantic coverage for the public literal-set count path.
//!
//! The fast count API is a performance surface, not just an ABI shape. These
//! cases drive `GpuLiteralSet::count` through the official CPU reference backend
//! and compare against an independent brute-force literal counter.

use vyre_libs::scan::GpuLiteralSet;

const GENERATED_CASES: usize = 2_048;
const ALPHABET: &[u8] = b"\0\x01abAB_:/-\xff";

#[test]
fn public_literal_set_count_matches_bruteforce_generated_matrix() {
    for case_id in 0..GENERATED_CASES {
        let (patterns, haystack) = generated_case(case_id as u64);
        let pattern_refs = patterns.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let engine = GpuLiteralSet::try_compile(&pattern_refs)
            .expect("Fix: generated non-empty literal set must compile");
        let expected = brute_force_literal_count(&patterns, &haystack);
        let reference_matches = engine.reference_scan(&haystack);
        let got = engine
            .count(&vyre_driver_reference::CpuRefBackend, &haystack)
            .expect("Fix: public literal-set count must dispatch through cpu-ref");

        assert_eq!(
            reference_matches.len(),
            expected as usize,
            "DFA reference_scan count diverged from brute-force oracle for case {case_id}: patterns={patterns:?} haystack={haystack:?}"
        );
        assert_eq!(
            got, expected,
            "public GpuLiteralSet::count diverged from brute-force oracle for case {case_id}: patterns={patterns:?} haystack={haystack:?}"
        );
    }
}

#[test]
fn public_literal_set_count_handles_binary_overlap_seeds() {
    let cases: &[(&[&[u8]], &[u8])] = &[
        (&[b"a", b"aa", b"aaa"], b"aaaaaa"),
        (&[b"\0", b"\0\0", b"a\0"], b"a\0\0a\0"),
        (&[b"BEGIN", b"GIN", b"N"], b"BEGINBEGIN"),
        (&[b"::", b":/-", b"\xff"], b":::/-\xff::"),
        (&[b"token", b"ken", b"n"], b"token-token"),
        (&[b"A", b"AB", b"B_"], b"AB_AB_"),
        (&[b"B", b"B", b"AB"], b"B AB"),
        (&[b"\xff\xff", b"\xff"], b"\xff\xff\xff"),
        (&[b"/", b"//", b"://"], b"http://x//y"),
    ];
    for (case_id, &(patterns, haystack)) in cases.iter().enumerate() {
        let engine =
            GpuLiteralSet::try_compile(patterns).expect("Fix: seeded literal set must compile");
        let owned_patterns = patterns
            .iter()
            .map(|pattern| pattern.to_vec())
            .collect::<Vec<_>>();
        let expected = brute_force_literal_count(&owned_patterns, haystack);
        let got = engine
            .count(&vyre_driver_reference::CpuRefBackend, haystack)
            .expect("Fix: public literal-set count must dispatch seeded cases through cpu-ref");

        assert_eq!(
            got, expected,
            "seeded public GpuLiteralSet::count case {case_id} diverged: patterns={patterns:?} haystack={haystack:?}"
        );
    }
}

fn generated_case(case_id: u64) -> (Vec<Vec<u8>>, Vec<u8>) {
    let mut rng = SplitMix64::new(case_id ^ 0x9E37_79B9_7F4A_7C15);
    let pattern_count = 1 + rng.usize(5);
    let mut patterns = Vec::with_capacity(pattern_count);
    for pattern_index in 0..pattern_count {
        let len = 1 + rng.usize(5);
        let mut pattern = Vec::with_capacity(len);
        for byte_index in 0..len {
            pattern.push(generated_byte(&mut rng, pattern_index, byte_index));
        }
        patterns.push(pattern);
    }

    let haystack_len = rng.usize(80);
    let mut haystack = Vec::with_capacity(haystack_len);
    for byte_index in 0..haystack_len {
        haystack.push(generated_byte(&mut rng, case_id as usize, byte_index));
    }

    if !haystack.is_empty() {
        for pattern in &patterns {
            let start = rng.usize(haystack.len());
            let copy_len = pattern.len().min(haystack.len() - start);
            haystack[start..start + copy_len].copy_from_slice(&pattern[..copy_len]);
        }
    }
    (patterns, haystack)
}

fn generated_byte(rng: &mut SplitMix64, salt_a: usize, salt_b: usize) -> u8 {
    match rng.usize(8) {
        0 => ALPHABET[(salt_a + salt_b) % ALPHABET.len()],
        1 => ALPHABET[rng.usize(ALPHABET.len())],
        2 => b'a' + (rng.usize(4) as u8),
        3 => b'A' + (rng.usize(4) as u8),
        _ => rng.next_u64() as u8,
    }
}

fn brute_force_literal_count(patterns: &[Vec<u8>], haystack: &[u8]) -> u32 {
    let mut count = 0u32;
    for pattern in patterns {
        if pattern.is_empty() || pattern.len() > haystack.len() {
            continue;
        }
        for start in 0..=haystack.len() - pattern.len() {
            if &haystack[start..start + pattern.len()] == pattern.as_slice() {
                count = count
                    .checked_add(1)
                    .expect("Fix: generated literal count should fit u32");
            }
        }
    }
    count
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn usize(&mut self, upper_exclusive: usize) -> usize {
        assert!(upper_exclusive > 0);
        (self.next_u64() as usize) % upper_exclusive
    }
}
