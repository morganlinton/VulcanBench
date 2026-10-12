use super::*;

#[test]
fn forward_empty_graph() {
    let got = fwd_cpu_ref(0, &[0], &[], &[], &[], 0xFFFF_FFFF);
    assert!(got.is_empty());
}

#[test]
fn forward_single_node_no_edges() {
    let got = fwd_cpu_ref(1, &[0, 0], &[0], &[0], &[0b0001], 0xFFFF_FFFF);
    assert_eq!(got, vec![0]);
}

#[test]
fn forward_self_loops_only() {
    let got = fwd_cpu_ref(2, &[0, 1, 2], &[0, 1], &[1, 1], &[0b0011], 0xFFFF_FFFF);
    assert_eq!(got, vec![0b0011]);
}

#[test]
fn forward_disconnected_components() {
    let got = fwd_cpu_ref(
        4,
        &[0, 1, 1, 2, 2],
        &[1, 3],
        &[1, 1],
        &[0b0001],
        0xFFFF_FFFF,
    );
    assert_eq!(got, vec![0b0010]);
}

#[test]
fn forward_max_node_count_cross_word() {
    let mut offsets = vec![0u32; 66];
    offsets[64] = 0;
    offsets[65] = 1;
    let mut frontier = vec![0u32; 3];
    frontier[2] = 1;
    let got = fwd_cpu_ref(65, &offsets, &[0], &[1], &frontier, 0xFFFF_FFFF);
    assert_eq!(got.len(), 3);
    assert_eq!(got[0], 1);
    assert_eq!(got[1], 0);
    assert_eq!(got[2], 0);
}

#[test]
fn forward_edge_mask_filters_all() {
    let got = fwd_cpu_ref(
        4,
        &[0, 2, 3, 4, 4],
        &[1, 2, 3, 3],
        &[0b01, 0b01, 0b01, 0b01],
        &[0b0001],
        0b10,
    );
    assert_eq!(got, vec![0]);
}

#[test]
fn forward_edge_kind_diversity_m8() {
    // DOMINANCE=0x01, ASSIGNMENT=0x02. Mask only DOMINANCE.
    let got = fwd_cpu_ref(4, &[0, 2, 2, 2, 2], &[1, 2], &[0x01, 0x02], &[0b0001], 0x01);
    assert_eq!(
        got,
        vec![0b0010],
        "broken impl ignoring kind_mask would produce 0b0110"
    );
}
