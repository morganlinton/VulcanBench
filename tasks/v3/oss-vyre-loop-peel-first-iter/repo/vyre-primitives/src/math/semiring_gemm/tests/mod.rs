use super::*;
use vyre_foundation::ir::BufferAccess;

#[test]
fn cpu_real_2x2() {
    // [[1,2],[3,4]] · [[5,6],[7,8]] = [[19,22],[43,50]]
    let a = vec![1, 2, 3, 4];
    let b = vec![5, 6, 7, 8];
    let c = semiring_gemm_cpu(&a, &b, 2, 2, 2, Semiring::Real);
    assert_eq!(c, vec![19, 22, 43, 50]);
}

#[test]
fn cpu_real_identity() {
    // A · I = A
    let a = vec![3, 5, 7, 11];
    let i = vec![1, 0, 0, 1];
    let c = semiring_gemm_cpu(&a, &i, 2, 2, 2, Semiring::Real);
    assert_eq!(c, a);
}

#[test]
fn cpu_into_reuses_output_and_truncates_stale_tail() {
    let a = vec![3, 5, 7, 11];
    let i = vec![1, 0, 0, 1];
    let mut c = Vec::with_capacity(8);
    c.extend([99; 8]);
    let ptr = c.as_ptr();

    try_semiring_gemm_cpu_into(&a, &i, 2, 2, 2, Semiring::Real, &mut c).unwrap();

    assert_eq!(c, a);
    assert_eq!(c.as_ptr(), ptr);
}

#[test]
fn generated_cpu_matches_independent_real_gemm() {
    for case in 0..48 {
        let m = 1 + (case % 4);
        let n = 1 + (case % 5);
        let k = 1 + (case % 6);
        let a: Vec<u32> = (0..m * k)
            .map(|idx| (idx as u32).wrapping_mul(3).wrapping_add(case as u32))
            .collect();
        let b: Vec<u32> = (0..k * n)
            .map(|idx| (idx as u32).wrapping_mul(5).wrapping_add(7))
            .collect();
        let mut c = Vec::with_capacity((m * n + 3) as usize);

        try_semiring_gemm_cpu_into(&a, &b, m as u32, n as u32, k as u32, Semiring::Real, &mut c)
            .unwrap();

        for i in 0..m {
            for j in 0..n {
                let mut expected = 0u32;
                for kk in 0..k {
                    expected = expected.wrapping_add(a[i * k + kk].wrapping_mul(b[kk * n + j]));
                }
                assert_eq!(c[i * n + j], expected, "case {case} cell {i},{j}");
            }
        }
    }
}

#[test]
fn cpu_min_plus_shortest_path_step() {
    // MinPlus matmul = one Bellman-Ford relaxation step.
    // Adjacency: 0→1 cost 5, 1→2 cost 3, 0→2 cost MAX (no direct edge).
    let inf = u32::MAX;
    let a = vec![
        inf, 5, inf, // row 0: from 0
        inf, inf, 3, // row 1: from 1
        inf, inf, inf, // row 2: from 2
    ];
    // A · A  -  squaring under min-plus = paths of length ≤ 2.
    let c = semiring_gemm_cpu(&a, &a, 3, 3, 3, Semiring::MinPlus);
    // 0→2 via 1: 5 + 3 = 8.
    assert_eq!(c[0 * 3 + 2], 8);
    // 0→1 has no length-exactly-2 path: MAX.
    assert_eq!(c[0 * 3 + 1], inf);
}

#[test]
fn cpu_min_plus_saturating_no_overflow() {
    // Two MAX entries combined must stay MAX, not wrap to MAX-1.
    let inf = u32::MAX;
    let a = vec![inf, inf, inf, inf];
    let b = vec![inf, inf, inf, inf];
    let c = semiring_gemm_cpu(&a, &b, 2, 2, 2, Semiring::MinPlus);
    for v in c {
        assert_eq!(v, inf);
    }
}

#[test]
fn cpu_bool_or_reachability() {
    // 3-node graph: 0→1, 1→2. Adjacency squared = 0→2 reachable in ≤2.
    let a = vec![
        0, 1, 0, // row 0
        0, 0, 1, // row 1
        0, 0, 0, // row 2
    ];
    let c = semiring_gemm_cpu(&a, &a, 3, 3, 3, Semiring::BoolOr);
    assert_eq!(c[0 * 3 + 2], 1);
    assert_eq!(c[0 * 3 + 1], 0); // length-exactly-2 from 0 to 1: none
}

#[test]
fn cpu_lineage_scallop_join() {
    // Scallop-style which-facts-used provenance (#39).
    // Each bit in a u32 names a clause / fact:
    //   bit 0 = "fact f1 used", bit 1 = "fact f2 used".
    //
    // Edges (entry value = bitset of facts justifying that edge):
    //   0→1 justified by {f1} = 0b01
    //   1→2 justified by {f2} = 0b10
    //
    // One join step (matmul under Lineage)  -  path 0→2 should carry
    // {f1, f2} = 0b11 (both facts contributed along the derivation).
    let f1 = 0b01;
    let f2 = 0b10;
    let a = vec![
        0, f1, 0, // 0
        0, 0, f2, // 1
        0, 0, 0, // 2
    ];
    let c = semiring_gemm_cpu(&a, &a, 3, 3, 3, Semiring::Lineage);
    assert_eq!(c[0 * 3 + 2], f1 | f2, "lineage = union of facts along path");
    // No path 0→1 of length exactly 2 → identity 0.
    assert_eq!(c[0 * 3 + 1], 0);
}

#[test]
fn cpu_lineage_alternative_paths_union() {
    // Two parallel routes 0→3, both length 2:
    //   route via 1: edges {f1}, {f2}
    //   route via 2: edges {f3}, {f4}
    // After one join step (length-2 paths), c[0,3] should accumulate
    // BOTH route's lineage sets via OR of OR.
    let f1 = 0b0001;
    let f2 = 0b0010;
    let f3 = 0b0100;
    let f4 = 0b1000;
    let a = vec![
        0, f1, f3, 0, // 0
        0, 0, 0, f2, // 1
        0, 0, 0, f4, // 2
        0, 0, 0, 0, // 3
    ];
    let c = semiring_gemm_cpu(&a, &a, 4, 4, 4, Semiring::Lineage);
    assert_eq!(
        c[0 * 4 + 3],
        f1 | f2 | f3 | f4,
        "expected union over both paths"
    );
}

#[test]
fn cpu_max_plus_longest_path() {
    // 0→1 weight 5, 1→2 weight 3. (max,+) squared: longest path 0→2 = 8.
    let a = vec![
        0, 5, 0, // row 0
        0, 0, 3, // row 1
        0, 0, 0, // row 2
    ];
    let c = semiring_gemm_cpu(&a, &a, 3, 3, 3, Semiring::MaxPlus);
    assert_eq!(c[0 * 3 + 2], 8);
}

#[test]
fn cpu_gf2_xor_closure() {
    // GF(2): (×, +) over Z/2 = (∧, ⊕). Should be the boolean XOR-AND ring.
    let a = vec![1, 0, 1, 1];
    let b = vec![1, 1, 0, 1];
    // c[0,0] = (1∧1) ⊕ (0∧0) = 1
    // c[0,1] = (1∧1) ⊕ (0∧1) = 1
    // c[1,0] = (1∧1) ⊕ (1∧0) = 1
    // c[1,1] = (1∧1) ⊕ (1∧1) = 0
    let c = semiring_gemm_cpu(&a, &b, 2, 2, 2, Semiring::Gf2);
    assert_eq!(c, vec![1, 1, 1, 0]);
}

#[test]
fn cpu_max_times_viterbi() {
    // (×, max): emission-times-transition along best path.
    // start probs * trans probs over 1 step, 2 states:
    // a = [0.5, 0.5] (as fixed-point u32: 50, 50)
    // b = [[0.6, 0.4], [0.3, 0.7]] (60/40, 30/70)
    let a = vec![50, 50];
    let b = vec![60, 40, 30, 70];
    // c[0,0] = max(50*60, 50*30) = max(3000, 1500) = 3000
    // c[0,1] = max(50*40, 50*70) = max(2000, 3500) = 3500
    let c = semiring_gemm_cpu(&a, &b, 1, 2, 2, Semiring::MaxTimes);
    assert_eq!(c, vec![3000, 3500]);
}

#[test]
fn emitted_program_buffer_layout() {
    let p = semiring_gemm("A", "B", "C", 4, 5, 3, Semiring::Real);
    assert_eq!(p.workgroup_size, [256, 1, 1]);
    let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
    assert_eq!(names, vec!["A", "B", "C"]);
    assert_eq!(p.buffers[0].count(), 4 * 3); // m*k
    assert_eq!(p.buffers[1].count(), 3 * 5); // k*n
    assert_eq!(p.buffers[2].count(), 4 * 5); // m*n
}

#[test]
fn emitted_program_buffer_access_modes() {
    let p = semiring_gemm("A", "B", "C", 2, 2, 2, Semiring::MinPlus);
    assert_eq!(p.buffers[0].access(), BufferAccess::ReadOnly);
    assert_eq!(p.buffers[1].access(), BufferAccess::ReadOnly);
    assert_eq!(p.buffers[2].access(), BufferAccess::ReadWrite);
}

#[test]
fn zero_m_traps() {
    let p = semiring_gemm("A", "B", "C", 0, 1, 1, Semiring::Real);
    assert!(p.stats().trap());
}

#[test]
fn zero_n_traps() {
    let p = semiring_gemm("A", "B", "C", 1, 0, 1, Semiring::Real);
    assert!(p.stats().trap());
}

#[test]
fn zero_k_traps() {
    let p = semiring_gemm("A", "B", "C", 1, 1, 0, Semiring::Real);
    assert!(p.stats().trap());
}

#[test]
fn identity_table_matches_doc() {
    assert_eq!(Semiring::Real.identity(), 0);
    assert_eq!(Semiring::MinPlus.identity(), u32::MAX);
    assert_eq!(Semiring::MaxPlus.identity(), 0);
    assert_eq!(Semiring::MaxTimes.identity(), 0);
    assert_eq!(Semiring::BoolOr.identity(), 0);
    assert_eq!(Semiring::BoolAnd.identity(), u32::MAX);
    assert_eq!(Semiring::Gf2.identity(), 0);
    assert_eq!(Semiring::Lineage.identity(), 0);
}
