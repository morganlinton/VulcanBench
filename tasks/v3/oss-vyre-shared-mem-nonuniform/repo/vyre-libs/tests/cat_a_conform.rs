//! Category-A conformance harness.
//!
//! Every Cat-A composition in `vyre-libs` must byte-match a handwritten
//! Reference oracle on a witness set. If the composition drifts, the op's
//! declared contract is broken  -  the conformance test fails.
//!
//! Adding a new Cat-A op to this harness = one `assert_cat_a_match!`
//! invocation + one witness fn. No IR rewriting required; the
//! reference is what the op *claims* to compute.

#![allow(deprecated)]
#![cfg(all(
    feature = "math-linalg",
    feature = "math-scan",
    feature = "matching-substring",
    feature = "matching-dfa",
))]

mod common;
use common::{decode_u32_words, u32_bytes};
use vyre::ir::Program;
use vyre_reference::value::Value;

/// Run `program` on `inputs` and return the read-write buffer outputs.
fn run_program(program: &Program, inputs: Vec<Value>) -> Vec<Vec<u8>> {
    let outputs =
        vyre_reference::reference_eval(program, &inputs).expect("Cat-A program must execute");
    outputs.into_iter().map(|v| v.to_bytes()).collect()
}

/// Canonical u32-per-byte encoding used by every Cat-A matching op.
#[test]
fn cat_a_substring_edge_cases() {
    // Needle longer than haystack → every match slot is 0.
    use vyre_libs::scan::substring_search;
    let program = substring_search("haystack", "needle", "matches", 3, 10);
    let haystack_bytes: Vec<u8> = "abc"
        .bytes()
        .flat_map(|b| u32::from(b).to_le_bytes())
        .collect();
    let needle_bytes = vec![0u8; 10 * 4];
    let matches_bytes = vec![0u8; 3 * 4];
    let outputs = run_program(
        &program,
        vec![
            Value::from(haystack_bytes),
            Value::from(needle_bytes),
            Value::from(matches_bytes),
        ],
    );
    let got = decode_u32_words(&outputs[0]);
    // The guarded predicate `needle_len <= haystack_len` short-circuits
    // and no position is checked. matches stays zero.
    assert_eq!(got, vec![0, 0, 0]);
}

#[test]
fn cat_a_substring_search_matches_cpu_reference() {
    use vyre_libs::scan::substring_search;

    // Witness set: (haystack, needle, expected match bitmap).
    let witnesses: &[(&str, &str, Vec<u32>)] = &[
        ("hello", "lo", vec![0, 0, 0, 1, 0]),
        ("abcabc", "abc", vec![1, 0, 0, 1, 0, 0]),
        ("aaaa", "aa", vec![1, 1, 1, 0]),
        ("x", "xy", vec![0]),
        ("abc", "", vec![1, 1, 1]),
    ];

    for (haystack, needle, expected) in witnesses {
        let needle_len = needle.len() as u32;
        let haystack_len = haystack.len() as u32;
        let program = substring_search("haystack", "needle", "matches", haystack_len, needle_len);

        let haystack_bytes = u32_bytes(&haystack.bytes().map(u32::from).collect::<Vec<_>>());
        let needle_bytes = u32_bytes(&needle.bytes().map(u32::from).collect::<Vec<_>>());
        let matches_bytes = vec![0u8; haystack.len() * 4];

        let outputs = run_program(
            &program,
            vec![
                Value::from(haystack_bytes),
                Value::from(needle_bytes),
                Value::from(matches_bytes),
            ],
        );
        assert_eq!(outputs.len(), 1, "Cat-A substring_search has one RW buffer");
        let got = decode_u32_words(&outputs[0]);
        assert_eq!(
            got, *expected,
            "Cat-A substring_search diverged on haystack={haystack:?} needle={needle:?}: got {got:?} expected {expected:?}"
        );
    }
}

#[test]
fn cat_a_dot_matches_cpu_reference() {
    use vyre_libs::math::dot;

    let witnesses: &[(Vec<u32>, Vec<u32>, u32)] = &[
        (
            vec![1, 2, 3, 4],
            vec![5, 6, 7, 8],
            5 + 2 * 6 + 3 * 7 + 4 * 8,
        ),
        (vec![0, 0, 0], vec![1, 2, 3], 0),
        (vec![7], vec![11], 77),
    ];

    for (lhs, rhs, expected) in witnesses {
        let program = dot("lhs", "rhs", "out", lhs.len() as u32).unwrap();
        let outputs = run_program(
            &program,
            vec![
                Value::from(u32_bytes(lhs)),
                Value::from(u32_bytes(rhs)),
                Value::from(vec![0u8; 4]),
            ],
        );
        let got = decode_u32_words(&outputs[0])[0];
        assert_eq!(
            got, *expected,
            "Cat-A dot diverged on lhs={lhs:?} rhs={rhs:?}: got {got} expected {expected}"
        );
    }
}

#[test]
fn cat_a_aho_corasick_matches_cpu_reference() {
    use vyre_libs::scan::{aho_corasick, dfa_compile};

    let patterns: [&[u8]; 4] = [b"he", b"she", b"his", b"hers"];
    let compiled = dfa_compile(&patterns);
    let haystack = b"ushers";

    // Reference oracle: walk the automaton; emit accept values at each
    // byte offset.
    let mut expected = Vec::with_capacity(haystack.len());
    let mut state = 0u32;
    for &byte in haystack {
        state = compiled.transitions[(state as usize) * 256 + byte as usize];
        expected.push(compiled.accept[state as usize]);
    }

    let program = aho_corasick(
        "haystack",
        "transitions",
        "accept",
        "matches",
        u32::try_from(haystack.len()).unwrap(),
        u32::try_from(compiled.accept.len()).unwrap(),
    );
    let inputs = vec![
        Value::from(u32_bytes(
            &haystack.iter().map(|&b| u32::from(b)).collect::<Vec<_>>(),
        )),
        Value::from(u32_bytes(&compiled.transitions)),
        Value::from(u32_bytes(&compiled.accept)),
        Value::from(vec![0u8; haystack.len() * 4]),
    ];
    let outputs = run_program(&program, inputs);
    assert_eq!(
        outputs.len(),
        1,
        "aho_corasick returns only the matches buffer"
    );
    let got = decode_u32_words(&outputs[0]);
    assert_eq!(
        got, expected,
        "Cat-A aho_corasick diverged on haystack=ushers patterns={patterns:?}"
    );
}

#[test]
fn cat_a_scan_prefix_sum_matches_cpu_reference() {
    use vyre_libs::math::scan_prefix_sum;

    let witnesses: &[(Vec<u32>, Vec<u32>)] = &[
        (vec![1, 2, 3, 4], vec![1, 3, 6, 10]),
        (vec![0, 0, 0, 0], vec![0, 0, 0, 0]),
        (vec![5, 1, 1, 1], vec![5, 6, 7, 8]),
    ];

    for (input, expected) in witnesses {
        let program = scan_prefix_sum("input", "output", input.len() as u32);
        let outputs = run_program(
            &program,
            vec![
                Value::from(u32_bytes(input)),
                Value::from(vec![0u8; input.len() * 4]),
            ],
        );
        let got = decode_u32_words(&outputs[0]);
        assert_eq!(
            got, *expected,
            "Cat-A scan_prefix_sum diverged on input={input:?}"
        );
    }
}
