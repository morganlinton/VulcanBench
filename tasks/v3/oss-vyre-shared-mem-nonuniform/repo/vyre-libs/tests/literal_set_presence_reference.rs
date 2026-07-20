//! High-volume soundness gate for the literal-set PRESENCE-bitmap program,
//! evaluated on the CPU REFERENCE backend (no GPU required — runs everywhere).
//!
//! For thousands of (literal set, haystack) cases the presence bitmap produced by
//! the suffix3-prefiltered presence program must mark EXACTLY the set of pattern
//! ids the bounded-ranges AC oracle reports: every present pattern set (recall),
//! and no absent pattern set (precision). This is the per-output-mode equivalent
//! of `bounded_ranges_suffix3_prefilter_reference_eval_matches_cpu_oracle`.

use std::collections::BTreeSet;

use vyre_libs::scan::classic_ac::{
    classic_ac_bounded_ranges_scan, classic_ac_candidate_end_byte_mask_words,
    classic_ac_candidate_suffix2_mask_words, classic_ac_candidate_suffix3_bloom_words,
    classic_ac_compile, presence_bitmap_words,
    try_build_ac_bounded_ranges_suffix3_presence_program,
};
use vyre_libs::scan::{pack_haystack_u32, pack_u32_slice};

struct Lcg(u64);
impl Lcg {
    fn next_u32(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            0
        } else {
            self.next_u32() % n
        }
    }
}

/// Small alphabet so literals collide and the DFA / prefilter actually exercise
/// shared prefixes, suffix2/suffix3 candidate gating, and overlapping matches.
const ALPHABET: &[u8] = b"abcAB_0/-";

fn random_literals(rng: &mut Lcg) -> Vec<Vec<u8>> {
    let count = 1 + rng.below(8); // 1..=8 patterns
    let mut set: BTreeSet<Vec<u8>> = BTreeSet::new();
    for _ in 0..count {
        let len = 1 + rng.below(6); // 1..=6 bytes
        let mut lit = Vec::with_capacity(len as usize);
        for _ in 0..len {
            lit.push(ALPHABET[rng.below(ALPHABET.len() as u32) as usize]);
        }
        set.insert(lit);
    }
    set.into_iter().collect()
}

fn random_haystack(rng: &mut Lcg) -> Vec<u8> {
    let len = rng.below(160); // 0..=159 bytes, includes empty + sub-pattern lengths
    (0..len)
        .map(|_| ALPHABET[rng.below(ALPHABET.len() as u32) as usize])
        .collect()
}

fn decode_u32(bytes: &[u8]) -> Vec<u32> {
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

fn presence_bit(bitmap: &[u32], pattern_id: u32) -> bool {
    let w = (pattern_id >> 5) as usize;
    let b = pattern_id & 31;
    bitmap.get(w).is_some_and(|word| (word >> b) & 1 == 1)
}

#[test]
fn presence_program_reference_eval_matches_cpu_oracle_high_volume() {
    let cases: usize = std::env::var("VYRE_PRESENCE_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5000);

    let mut rng = Lcg(0x7265_7365_6e63_65u64);
    let mut checked = 0usize;
    let mut nonempty_presence = 0usize;

    for case in 0..cases {
        let literals = random_literals(&mut rng);
        let pattern_refs: Vec<&[u8]> = literals.iter().map(Vec::as_slice).collect();
        let haystack = random_haystack(&mut rng);

        let ac = classic_ac_compile(&pattern_refs);
        let lengths: Vec<u32> = literals.iter().map(|l| l.len() as u32).collect();
        let pattern_count = literals.len() as u32;

        // CPU oracle: the set of pattern ids that occur in `haystack`.
        let expected: BTreeSet<u32> = classic_ac_bounded_ranges_scan(&ac, &lengths, &haystack)
            .into_iter()
            .map(|(pid, _start, _end)| pid)
            .collect();

        let program = try_build_ac_bounded_ranges_suffix3_presence_program(&ac.dfa, pattern_count)
            .expect("presence program builds for a non-degenerate DFA");

        let presence_words = presence_bitmap_words(pattern_count) as usize;
        let inputs = vec![
            vyre_reference::value::Value::from(pack_haystack_u32(&haystack)),
            vyre_reference::value::Value::from(pack_u32_slice(&ac.dfa.transitions)),
            vyre_reference::value::Value::from(pack_u32_slice(&ac.dfa.output_offsets)),
            vyre_reference::value::Value::from(pack_u32_slice(&ac.dfa.output_records)),
            vyre_reference::value::Value::from(pack_u32_slice(&lengths)),
            vyre_reference::value::Value::from(pack_u32_slice(&[haystack.len() as u32])),
            vyre_reference::value::Value::from(pack_u32_slice(&vec![0u32; presence_words])),
            vyre_reference::value::Value::from(pack_u32_slice(
                &classic_ac_candidate_end_byte_mask_words(&ac.dfa),
            )),
            vyre_reference::value::Value::from(pack_u32_slice(
                &classic_ac_candidate_suffix2_mask_words(&ac.dfa),
            )),
            vyre_reference::value::Value::from(pack_u32_slice(
                &classic_ac_candidate_suffix3_bloom_words(&pattern_refs),
            )),
        ];

        let outputs = vyre_reference::reference_eval(&program, &inputs)
            .expect("Fix: suffix3 presence program must evaluate in the reference backend");
        let bitmap = decode_u32(&outputs[0].to_bytes());

        for pid in 0..pattern_count {
            let got = presence_bit(&bitmap, pid);
            let want = expected.contains(&pid);
            assert_eq!(
                got,
                want,
                "case {case}: presence mismatch for pattern {pid} ({:?}) in haystack {:?} \
                 (literals={:?}): presence={got} oracle={want}",
                String::from_utf8_lossy(&literals[pid as usize]),
                String::from_utf8_lossy(&haystack),
                literals
                    .iter()
                    .map(|l| String::from_utf8_lossy(l).into_owned())
                    .collect::<Vec<_>>(),
            );
        }
        if !expected.is_empty() {
            nonempty_presence += 1;
        }
        checked += 1;
    }

    assert_eq!(checked, cases);
    // The corpus must actually exercise the present-pattern path, or the test is
    // only checking the empty case.
    assert!(
        nonempty_presence * 4 > cases,
        "only {nonempty_presence}/{cases} cases had any present pattern; corpus is too sparse to be a real gate"
    );
    eprintln!(
        "presence reference parity: {checked} cases, {nonempty_presence} with ≥1 present pattern"
    );
}
