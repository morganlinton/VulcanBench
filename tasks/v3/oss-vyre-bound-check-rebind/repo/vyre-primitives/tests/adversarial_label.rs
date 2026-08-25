//! Failure-oriented adversarial tests for label primitives.
//!
//! Focus: hostile boundaries, overflow, invalid offsets, property invariants.
#![cfg(feature = "label")]

fn cpu_ref(node_tags: &[u32], family_mask: u32) -> Vec<u32> {
    let mut out = vec![0u32; node_tags.len().div_ceil(32)];
    for (node, tag) in node_tags.iter().enumerate() {
        if (tag & family_mask) != 0 {
            out[node / 32] |= 1u32 << (node % 32);
        }
    }
    out
}

#[test]
fn cpu_ref_empty() {
    let got = cpu_ref(&[], 0xFF);
    assert!(got.is_empty());
}

#[test]
fn cpu_ref_all_zeros() {
    let got = cpu_ref(&[0, 0, 0, 0], 0xFF);
    assert_eq!(got, vec![0]);
}

#[test]
fn cpu_ref_all_hits() {
    let got = cpu_ref(&[0xFF; 64], 0xFF);
    assert_eq!(got, vec![0xFFFFFFFF; 2]);
}

#[test]
fn cpu_ref_family_mask_zero() {
    let got = cpu_ref(&[0xFF; 64], 0);
    assert_eq!(got, vec![0; 2]);
}

#[test]
fn cpu_ref_u32_max_overflow_boundary() {
    // 33 nodes requires 2 words
    let tags: Vec<u32> = (0..33).map(|i| if i == 32 { 0x1 } else { 0 }).collect();
    let got = cpu_ref(&tags, 0x1);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0], 0);
    assert_eq!(got[1], 0x1);
}

#[test]
fn cpu_ref_single_node_boundary() {
    let got = cpu_ref(&[0x01], 0x01);
    assert_eq!(got, vec![0x1]);
}

#[test]
fn cpu_ref_31_nodes_fits_one_word() {
    let tags = vec![0x01; 31];
    let got = cpu_ref(&tags, 0x01);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0], 0x7FFFFFFF);
}

#[test]
fn cpu_ref_32_nodes_exact_word() {
    let tags = vec![0x01; 32];
    let got = cpu_ref(&tags, 0x01);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0], 0xFFFFFFFF);
}

#[test]
fn cpu_ref_partial_hits() {
    let tags = vec![0x01, 0x02, 0x03, 0x04];
    let got = cpu_ref(&tags, 0x02);
    assert_eq!(got, vec![0b0110]);
}
