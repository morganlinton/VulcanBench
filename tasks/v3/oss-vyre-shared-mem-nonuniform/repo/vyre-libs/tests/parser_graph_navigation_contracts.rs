//! Contract tests for parser-graph navigation (AST walk primitives).
//!
//! Covers ast_walk_preorder and ast_walk_postorder over spine trees.
//! Properties: specific index sequences, boundary truncation, empty
//! trees, single-node trees, cap exhaustion, and validation.
//!
//! GPU acquisition: none  -  all tests use the reference interpreter.

#![allow(deprecated)]

mod common;
use common::decode_u32_words;
use vyre_reference::value::Value;

fn pack_spine_fixture(node_count: u32) -> (Vec<u8>, Vec<u8>) {
    let full = vyre_foundation::vast::pack_spine_vast(&vec![1u32; node_count as usize]);
    let node_len = (node_count as usize) * vyre_foundation::vast::NODE_STRIDE_U32 * 4;
    let start = vyre_foundation::vast::HEADER_LEN;
    let region = full[start..start + node_len].to_vec();
    (full, region)
}

fn pack_branching_fixture() -> Vec<u8> {
    use vyre_foundation::vast::{VastNode, NODE_STRIDE_U32, SENTINEL};

    let nodes = [
        VastNode {
            kind: 1,
            parent_idx: SENTINEL,
            first_child: 1,
            next_sibling: SENTINEL,
            src_file: 0,
            src_byte_off: 0,
            src_byte_len: 1,
            attr_off: 0,
            attr_len: 0,
            reserved: 0,
        },
        VastNode {
            kind: 1,
            parent_idx: 0,
            first_child: 4,
            next_sibling: 2,
            src_file: 0,
            src_byte_off: 1,
            src_byte_len: 1,
            attr_off: 0,
            attr_len: 0,
            reserved: 0,
        },
        VastNode {
            kind: 1,
            parent_idx: 0,
            first_child: SENTINEL,
            next_sibling: 3,
            src_file: 0,
            src_byte_off: 2,
            src_byte_len: 1,
            attr_off: 0,
            attr_len: 0,
            reserved: 0,
        },
        VastNode {
            kind: 1,
            parent_idx: 0,
            first_child: 5,
            next_sibling: SENTINEL,
            src_file: 0,
            src_byte_off: 3,
            src_byte_len: 1,
            attr_off: 0,
            attr_len: 0,
            reserved: 0,
        },
        VastNode {
            kind: 1,
            parent_idx: 1,
            first_child: SENTINEL,
            next_sibling: SENTINEL,
            src_file: 0,
            src_byte_off: 4,
            src_byte_len: 1,
            attr_off: 0,
            attr_len: 0,
            reserved: 0,
        },
        VastNode {
            kind: 1,
            parent_idx: 3,
            first_child: SENTINEL,
            next_sibling: SENTINEL,
            src_file: 0,
            src_byte_off: 5,
            src_byte_len: 1,
            attr_off: 0,
            attr_len: 0,
            reserved: 0,
        },
    ];

    let mut out = Vec::with_capacity(nodes.len() * NODE_STRIDE_U32 * 4);
    for node in nodes {
        out.extend_from_slice(&node.to_bytes());
    }
    out
}

// ---------------------------------------------------------------------------
// ast_walk_preorder
// ---------------------------------------------------------------------------

#[test]
fn preorder_basic_four_node_spine() {
    let (_, node_region) = pack_spine_fixture(4);
    let outz = vec![0u8; 32];
    let program = vyre_libs::graph::ast_walk_preorder("nodes", "out", 4, 8);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[Value::from(node_region.clone()), Value::from(outz)],
    )
    .expect("preorder must execute");

    let got = decode_u32_words(&outputs[0].to_bytes());
    let expected = vyre_foundation::vast::walk_preorder_indices(&node_region, 4, 128).unwrap();
    assert_eq!(
        &got[..expected.len()],
        &expected[..],
        "preorder output must match host oracle"
    );
}

#[test]
fn preorder_single_node() {
    let (_, node_region) = pack_spine_fixture(1);
    let outz = vec![0u8; 32]; // cap=8 u32s = 32 bytes
    let program = vyre_libs::graph::ast_walk_preorder("nodes", "out", 1, 8);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[Value::from(node_region.clone()), Value::from(outz)],
    )
    .unwrap();

    let got = decode_u32_words(&outputs[0].to_bytes());
    assert_eq!(got[0], 0, "single-node preorder must emit root = 0");
}

#[test]
fn preorder_empty_tree_is_a_valid_noop() {
    let node_region = vec![0u8; 4];
    let outz = vec![0u8; 32];
    let program = vyre_libs::graph::ast_walk_preorder("nodes", "out", 0, 8);
    assert!(
        vyre::validate(&program).is_empty(),
        "empty preorder walk must still be a valid program"
    );

    let outputs =
        vyre_reference::reference_eval(&program, &[Value::from(node_region), Value::from(outz)])
            .unwrap();
    let got = decode_u32_words(&outputs[0].to_bytes());
    assert!(
        got.iter().all(|&word| word == 0),
        "empty preorder walk must not mutate output"
    );
}

#[test]
fn preorder_cap_truncates_output() {
    let (_, node_region) = pack_spine_fixture(8);
    let cap = 3u32;
    let outz = vec![0u8; 32];
    let program = vyre_libs::graph::ast_walk_preorder("nodes", "out", 8, cap);
    let outputs =
        vyre_reference::reference_eval(&program, &[Value::from(node_region), Value::from(outz)])
            .unwrap();

    let got = decode_u32_words(&outputs[0].to_bytes());
    // The first `cap` entries should be 0, 1, 2 (spine preorder is sequential)
    assert_eq!(got[0], 0);
    assert_eq!(got[1], 1);
    assert_eq!(got[2], 2);
    // Entry 3 should remain zero because the return_() fires before store.
    // Actually, the IR stores first, then checks cap, so index cap-1 is
    // stored. After that, the next iteration hits the cap check and returns.
    // So index cap (0-based) is NOT stored.
    assert_eq!(got[cap as usize], 0, "output beyond cap must stay zero");
}

#[test]
fn preorder_eight_node_spine_matches_host() {
    let (_, node_region) = pack_spine_fixture(8);
    let outz = vec![0u8; 64];
    let program = vyre_libs::graph::ast_walk_preorder("nodes", "out", 8, 16);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[Value::from(node_region.clone()), Value::from(outz)],
    )
    .unwrap();

    let got = decode_u32_words(&outputs[0].to_bytes());
    let expected = vyre_foundation::vast::walk_preorder_indices(&node_region, 8, 128).unwrap();
    assert_eq!(&got[..expected.len()], &expected[..]);
}

#[test]
fn preorder_branching_tree_matches_host() {
    let node_region = pack_branching_fixture();
    let outz = vec![0u8; 32];
    let program = vyre_libs::graph::ast_walk_preorder("nodes", "out", 6, 8);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[Value::from(node_region.clone()), Value::from(outz)],
    )
    .unwrap();

    let got = decode_u32_words(&outputs[0].to_bytes());
    let expected = vyre_foundation::vast::walk_preorder_indices(&node_region, 6, 128).unwrap();
    assert_eq!(expected, vec![0, 1, 4, 2, 3, 5]);
    assert_eq!(&got[..expected.len()], &expected[..]);
}

#[test]
fn preorder_program_validates() {
    let p = vyre_libs::graph::ast_walk_preorder("nodes", "out", 4, 8);
    assert!(
        vyre::validate(&p).is_empty(),
        "preorder program must pass validation"
    );
}

// ---------------------------------------------------------------------------
// ast_walk_postorder
// ---------------------------------------------------------------------------

#[test]
fn postorder_basic_four_node_spine() {
    let outz = vec![0u8; 32];
    let program = vyre_libs::graph::ast_walk_postorder("out", 4);
    let outputs = vyre_reference::reference_eval(&program, &[Value::from(outz)]).unwrap();

    let got = decode_u32_words(&outputs[0].to_bytes());
    // Postorder for a spine is reverse: 3, 2, 1, 0
    assert_eq!(&got[..4], &[3, 2, 1, 0]);
}

#[test]
fn postorder_single_node() {
    let outz = vec![0u8; 8];
    let program = vyre_libs::graph::ast_walk_postorder("out", 1);
    let outputs = vyre_reference::reference_eval(&program, &[Value::from(outz)]).unwrap();

    let got = decode_u32_words(&outputs[0].to_bytes());
    assert_eq!(got[0], 0);
}

#[test]
fn postorder_empty_tree_is_a_valid_noop() {
    let outz = vec![0u8; 32];
    let program = vyre_libs::graph::ast_walk_postorder("out", 0);
    assert!(
        vyre::validate(&program).is_empty(),
        "empty postorder walk must still be a valid program"
    );

    let outputs = vyre_reference::reference_eval(&program, &[Value::from(outz)]).unwrap();
    let got = decode_u32_words(&outputs[0].to_bytes());
    assert!(
        got.iter().all(|&word| word == 0),
        "empty postorder walk must not mutate output"
    );
}

#[test]
fn postorder_matches_reverse_of_preorder_spine() {
    let (_, node_region) = pack_spine_fixture(8);
    let pre = vyre_foundation::vast::walk_preorder_indices(&node_region, 8, 128).unwrap();
    let post = vyre_foundation::vast::walk_postorder_indices(&node_region, 8, 128).unwrap();
    let rev: Vec<u32> = pre.iter().rev().copied().collect();
    assert_eq!(post, rev, "spine postorder must equal reverse of preorder");
}

#[test]
fn postorder_program_validates() {
    let p = vyre_libs::graph::ast_walk_postorder("out", 4);
    assert!(
        vyre::validate(&p).is_empty(),
        "postorder program must pass validation"
    );
}

#[test]
fn postorder_eight_node_sequence() {
    let outz = vec![0u8; 64];
    let program = vyre_libs::graph::ast_walk_postorder("out", 8);
    let outputs = vyre_reference::reference_eval(&program, &[Value::from(outz)]).unwrap();

    let got = decode_u32_words(&outputs[0].to_bytes());
    let expected: Vec<u32> = (0..8).rev().collect();
    assert_eq!(&got[..8], &expected[..]);
}

#[test]
fn postorder_branching_tree_matches_host() {
    let node_region = pack_branching_fixture();
    let outz = vec![0u8; 32];
    let program = vyre_libs::graph::ast_walk_postorder_nodes("nodes", "out", 6, 8);
    let outputs = vyre_reference::reference_eval(
        &program,
        &[Value::from(node_region.clone()), Value::from(outz)],
    )
    .unwrap();

    let got = decode_u32_words(&outputs[0].to_bytes());
    let expected = vyre_foundation::vast::walk_postorder_indices(&node_region, 6, 128).unwrap();
    assert_eq!(expected, vec![4, 1, 2, 5, 3, 0]);
    assert_eq!(&got[..expected.len()], &expected[..]);
}

// ---------------------------------------------------------------------------
// Navigation invariants across both walks
// ---------------------------------------------------------------------------

#[test]
fn preorder_postorder_bijection() {
    // For any spine tree, preorder and postorder are bijections over
    // the node index set 0..node_count-1.
    for n in [1, 2, 4, 8, 16] {
        let (_, node_region) = pack_spine_fixture(n);
        let pre = vyre_foundation::vast::walk_preorder_indices(&node_region, n, 128).unwrap();
        let post = vyre_foundation::vast::walk_postorder_indices(&node_region, n, 128).unwrap();

        let mut pre_sorted = pre.clone();
        pre_sorted.sort_unstable();
        let mut post_sorted = post.clone();
        post_sorted.sort_unstable();

        let expected: Vec<u32> = (0..n).collect();
        assert_eq!(
            pre_sorted, expected,
            "preorder must be a permutation of 0..{n}"
        );
        assert_eq!(
            post_sorted, expected,
            "postorder must be a permutation of 0..{n}"
        );
    }
}

#[test]
fn preorder_root_is_always_zero() {
    for n in [1, 2, 4, 8, 16, 32] {
        let (_, node_region) = pack_spine_fixture(n);
        let pre = vyre_foundation::vast::walk_preorder_indices(&node_region, n, 128).unwrap();
        assert_eq!(pre[0], 0, "preorder root must always be 0 for spine trees");
    }
}

#[test]
fn postorder_last_is_always_zero_for_spine() {
    for n in [1, 2, 4, 8, 16, 32] {
        let (_, node_region) = pack_spine_fixture(n);
        let post = vyre_foundation::vast::walk_postorder_indices(&node_region, n, 128).unwrap();
        assert_eq!(
            post[post.len() - 1],
            0,
            "postorder last element must be root (0) for spine trees"
        );
    }
}

#[test]
fn preorder_cap_less_than_node_count_truncates_correctly() {
    let n = 16u32;
    let (_, node_region) = pack_spine_fixture(n);
    let cap = 5u32;
    let outz = vec![0u8; 64];
    let program = vyre_libs::graph::ast_walk_preorder("nodes", "out", n, cap);
    let outputs =
        vyre_reference::reference_eval(&program, &[Value::from(node_region), Value::from(outz)])
            .unwrap();

    let got = decode_u32_words(&outputs[0].to_bytes());
    for i in 0..cap {
        assert_eq!(
            got[i as usize], i,
            "preorder cap must preserve first {cap} elements"
        );
    }
    // Everything beyond cap must stay zero (output buffer was zeroed)
    for i in cap..n {
        assert_eq!(got[i as usize], 0, "preorder must not write beyond cap");
    }
}

#[test]
fn preorder_cap_of_one_only_emits_root() {
    let n = 8u32;
    let (_, node_region) = pack_spine_fixture(n);
    let outz = vec![0u8; 32];
    let program = vyre_libs::graph::ast_walk_preorder("nodes", "out", n, 1);
    let outputs =
        vyre_reference::reference_eval(&program, &[Value::from(node_region), Value::from(outz)])
            .unwrap();

    let got = decode_u32_words(&outputs[0].to_bytes());
    assert_eq!(got[0], 0);
    assert!(
        got[1..].iter().all(|&v| v == 0),
        "only root must be emitted when cap=1"
    );
}
