use super::*;

#[test]
fn backward_empty_graph() {
    let got = bwd_cpu_ref(0, &[0], &[], &[], &[], 0xFFFF_FFFF);
    assert!(got.is_empty());
}

#[test]
fn backward_single_node_no_edges() {
    let got = bwd_cpu_ref(1, &[0, 0], &[0], &[0], &[0b0001], 0xFFFF_FFFF);
    assert_eq!(got, vec![0]);
}

#[test]
fn backward_self_loops_only() {
    let got = bwd_cpu_ref(2, &[0, 1, 2], &[0, 1], &[1, 1], &[0b0001], 0xFFFF_FFFF);
    assert_eq!(got, vec![0b0001]);
}

#[test]
fn backward_disconnected_components() {
    let got = bwd_cpu_ref(
        4,
        &[0, 1, 1, 2, 2],
        &[1, 3],
        &[1, 1],
        &[0b1000],
        0xFFFF_FFFF,
    );
    assert_eq!(got, vec![0b0100]);
}

#[test]
fn backward_edge_kind_diversity_m8() {
    let got = bwd_cpu_ref(4, &[0, 2, 2, 2, 2], &[1, 2], &[0x01, 0x02], &[0b0010], 0x01);
    assert_eq!(
        got,
        vec![0b0001],
        "broken impl ignoring kind_mask would produce 0"
    );
}
