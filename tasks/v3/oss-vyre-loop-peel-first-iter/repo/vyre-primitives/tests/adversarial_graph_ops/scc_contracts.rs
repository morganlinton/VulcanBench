use super::*;

#[test]
fn scc_empty_graph() {
    let out = scc_cpu_ref(0, &[], &[], &[], 0);
    assert!(out.is_empty());
}

#[test]
fn scc_self_loop() {
    let out = scc_cpu_ref(1, &[0b0001], &[0b0001], &[u32::MAX; 1], 0);
    assert_eq!(out, vec![0]);
}

#[test]
fn scc_disconnected_components() {
    let forward = vec![0b0101];
    let backward = vec![0b0101];
    let comp_in = vec![u32::MAX; 4];
    let out = scc_cpu_ref(4, &forward, &backward, &comp_in, 0);
    assert_eq!(out[0], 0);
    assert_eq!(out[1], u32::MAX);
    assert_eq!(out[2], 0);
    assert_eq!(out[3], u32::MAX);
}

#[test]
fn scc_multi_word_cross_boundary() {
    let mut forward = vec![0u32; 3];
    let mut backward = vec![0u32; 3];
    forward[1] = 1; // node 32
    forward[2] = 1; // node 64
    backward[1] = 1;
    backward[2] = 1;
    let comp_in = vec![u32::MAX; 65];
    let out = scc_cpu_ref(65, &forward, &backward, &comp_in, 42);
    assert_eq!(out[32], 42);
    assert_eq!(out[64], 42);
    assert_eq!(out[0], u32::MAX);
    assert_eq!(out[31], u32::MAX);
    assert_eq!(out[33], u32::MAX);
    assert_eq!(out[63], u32::MAX);
}
