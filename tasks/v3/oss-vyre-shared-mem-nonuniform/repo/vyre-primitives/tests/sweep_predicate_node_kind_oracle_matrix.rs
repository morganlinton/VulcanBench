//! Handwritten oracle matrix for `predicate::node_kind_eq`.
//!
//! Compares production nodeset filters against an independent bitset oracle
//! across hostile node counts, kind constants, and LCG seeds.

#![forbid(unsafe_code)]
#![cfg(feature = "cpu-parity")]

use vyre_primitives::predicate::node_kind;

type NodeKindFilter = fn(&[u32], u32) -> Vec<u32>;
type NodeKindFilterInto = fn(&[u32], u32, &mut Vec<u32>);

#[test]
fn node_kind_eq_matches_independent_oracle_matrix() {
    assert_node_kind(
        "node_kind_eq",
        vyre_primitives::predicate::node_kind_eq::cpu_ref,
        vyre_primitives::predicate::node_kind_eq::cpu_ref_into,
        oracle_node_kind_eq,
    );
}

fn assert_node_kind(
    name: &str,
    actual: NodeKindFilter,
    actual_into: NodeKindFilterInto,
    expected: NodeKindFilter,
) {
    let cases = node_kind_cases();
    for (case_idx, (nodes, kind)) in cases.iter().enumerate() {
        let expected_out = expected(nodes, *kind);
        assert_eq!(
            actual(nodes, *kind),
            expected_out,
            "Fix: {name} adversarial case {case_idx} node_count={} kind={kind} must match the independent oracle.",
            nodes.len()
        );

        let words = nodes.len().div_ceil(32);
        let mut reused = vec![0xFEED_FACE; words.saturating_add(5)];
        actual_into(nodes, *kind, &mut reused);
        assert_eq!(
            reused, expected_out,
            "Fix: {name} cpu_ref_into adversarial case {case_idx} must clear stale nodeset capacity before writing."
        );
    }
}

fn oracle_node_kind_eq(nodes: &[u32], kind: u32) -> Vec<u32> {
    let words = nodes.len().div_ceil(32);
    let mut out = vec![0u32; words];
    for (node, &value) in nodes.iter().enumerate() {
        if value == kind {
            out[node / 32] |= 1u32 << (node % 32);
        }
    }
    out
}

fn node_kind_cases() -> Vec<(Vec<u32>, u32)> {
    let mut cases = Vec::new();
    let lengths = [
        0usize, 1, 2, 3, 7, 31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256, 257, 1023, 1024, 1025,
    ];
    let kinds = [
        0u32,
        node_kind::VARIABLE,
        node_kind::CALL,
        node_kind::IMPORT,
        node_kind::LITERAL,
        node_kind::SSA,
        node_kind::BASIC_BLOCK,
        node_kind::BINARY,
        node_kind::FUNCTION_DECL,
        u32::MAX,
        0x8000_0000,
    ];

    for len in lengths {
        for &kind in &kinds {
            cases.push((vec![kind; len], kind));
            cases.push((vec![kind.wrapping_add(1); len], kind));
            cases.push((alternating_kinds(len, kind, kind.wrapping_add(1)), kind));
        }
        cases.push((canonical_mix(len), node_kind::CALL));
        cases.push((canonical_mix(len), node_kind::LITERAL));
    }

    for seed in [
        0x0000_0001,
        0xC0FF_EE11,
        0xDEAD_BEEF,
        0xA5A5_5A5A,
        0x8000_0000,
        0xFFFF_FFFE,
    ] {
        for len in lengths {
            let nodes = lcg_kinds(seed, len);
            for &kind in &kinds[..8] {
                cases.push((nodes.clone(), kind));
            }
        }
    }

    for case in 0..8192usize {
        let len = case % 257;
        let kind = kinds[case % kinds.len()];
        let nodes = generated_node_kinds(case as u64 ^ 0xBADC_0FFE, len);
        cases.push((nodes, kind));
    }

    cases
}

fn canonical_mix(len: usize) -> Vec<u32> {
    const KINDS: [u32; 8] = [
        node_kind::VARIABLE,
        node_kind::CALL,
        node_kind::IMPORT,
        node_kind::LITERAL,
        node_kind::SSA,
        node_kind::BASIC_BLOCK,
        node_kind::BINARY,
        node_kind::FUNCTION_DECL,
    ];
    (0..len).map(|idx| KINDS[idx % KINDS.len()]).collect()
}

fn alternating_kinds(len: usize, even: u32, odd: u32) -> Vec<u32> {
    (0..len)
        .map(|idx| if idx % 2 == 0 { even } else { odd })
        .collect()
}

fn lcg_kinds(seed: u32, len: usize) -> Vec<u32> {
    let mut state = seed;
    (0..len)
        .map(|idx| {
            state = state
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223)
                .rotate_left((idx % 17) as u32);
            (state & 0xF) + 1
        })
        .collect()
}

fn generated_node_kinds(seed: u64, len: usize) -> Vec<u32> {
    let mut rng = seed;
    (0..len)
        .map(|idx| {
            rng = rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            match (rng as u32).wrapping_add(idx as u32) % 11 {
                0 => node_kind::VARIABLE,
                1 => node_kind::CALL,
                2 => node_kind::IMPORT,
                3 => node_kind::LITERAL,
                4 => node_kind::SSA,
                5 => node_kind::BASIC_BLOCK,
                6 => node_kind::BINARY,
                7 => node_kind::FUNCTION_DECL,
                8 => 0,
                9 => u32::MAX,
                _ => (rng >> 32) as u32,
            }
        })
        .collect()
}
