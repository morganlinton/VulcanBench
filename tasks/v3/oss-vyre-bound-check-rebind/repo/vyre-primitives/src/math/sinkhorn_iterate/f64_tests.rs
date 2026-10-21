use super::*;

#[test]
fn one_by_one_trivial_converges_immediately() {
    let (u, v, iters) = sinkhorn_iterate_f64(&[1.0], &[1.0], &[1.0], 1e-12, 100);
    assert!((u[0] * 1.0 * v[0] - 1.0).abs() < 1e-9);
    assert!(
        iters <= 2,
        "trivial should converge in <=2 iters, got {iters}"
    );
}

#[test]
fn two_by_two_balanced_converges() {
    // k = ones(2,2). a = [1, 1], b = [1, 1]. Total mass = 2 on both sides.
    let k = vec![1.0, 1.0, 1.0, 1.0];
    let a = vec![1.0, 1.0];
    let b = vec![1.0, 1.0];
    let (u, v, iters) = sinkhorn_iterate_f64(&k, &a, &b, 1e-9, 100);
    assert!(iters < 100);
    let row_err = sinkhorn_row_residual(&k, &u, &v, &a);
    let col_err = sinkhorn_col_residual(&k, &u, &v, &b);
    assert!(row_err < 1e-7, "row residual {row_err} > 1e-7");
    assert!(col_err < 1e-7, "col residual {col_err} > 1e-7");
}

#[test]
fn f64_into_reuses_work_buffers() {
    let k = vec![1.0, 1.0, 1.0, 1.0];
    let a = vec![1.0, 1.0];
    let b = vec![1.0, 1.0];
    let mut u = Vec::with_capacity(8);
    let mut v = Vec::with_capacity(8);
    let mut old = Vec::with_capacity(8);
    let u_ptr = u.as_ptr();
    let v_ptr = v.as_ptr();
    let old_ptr = old.as_ptr();
    let iters = sinkhorn_iterate_f64_into(&k, &a, &b, 1e-9, 100, &mut u, &mut v, &mut old);
    assert!(iters < 100);
    assert_eq!(u.as_ptr(), u_ptr);
    assert_eq!(v.as_ptr(), v_ptr);
    assert_eq!(old.as_ptr(), old_ptr);
}

#[test]
fn f64_into_truncates_stale_work_buffers() {
    let k = vec![1.0, 1.0, 1.0, 1.0];
    let a = vec![1.0, 1.0];
    let b = vec![1.0, 1.0];
    let mut u = Vec::with_capacity(8);
    let mut v = Vec::with_capacity(8);
    let mut old = Vec::with_capacity(8);
    u.extend([99.0; 8]);
    v.extend([99.0; 8]);
    old.extend([99.0; 8]);
    let u_ptr = u.as_ptr();
    let v_ptr = v.as_ptr();
    let old_ptr = old.as_ptr();

    let iters =
        try_sinkhorn_iterate_f64_into(&k, &a, &b, 1e-9, 100, &mut u, &mut v, &mut old).unwrap();

    assert!(iters < 100);
    assert_eq!(u.as_ptr(), u_ptr);
    assert_eq!(v.as_ptr(), v_ptr);
    assert_eq!(old.as_ptr(), old_ptr);
}

#[test]
fn f64_try_path_rejects_invalid_shape() {
    let err =
        try_sinkhorn_iterate_f64(&[1.0, 2.0], &[1.0, 1.0], &[1.0, 1.0], 1e-6, 10).unwrap_err();
    assert!(err.contains("k.len()==a.len()*b.len()"), "{err}");
}

#[test]
fn asymmetric_marginals_still_balance() {
    // 2x3 kernel, marginals a=[2, 4] b=[1, 2, 3].
    // Total mass = 6 on both sides.
    let k = vec![1.0, 2.0, 3.0, 2.0, 1.0, 1.0];
    let a = vec![2.0, 4.0];
    let b = vec![1.0, 2.0, 3.0];
    let (u, v, iters) = sinkhorn_iterate_f64(&k, &a, &b, 1e-10, 1000);
    assert!(iters < 1000);
    let row_err = sinkhorn_row_residual(&k, &u, &v, &a);
    let col_err = sinkhorn_col_residual(&k, &u, &v, &b);
    assert!(row_err < 1e-7);
    assert!(col_err < 1e-7);
}

#[test]
fn cap_hit_returns_max_iterations() {
    // With max_iterations=1 we always return iter=1 (one full
    // pass executed but tolerance not met).
    let k = vec![1.0, 1.0, 1.0, 1.0];
    let a = vec![1.0, 3.0];
    let b = vec![1.0, 3.0];
    let (_u, _v, iters) = sinkhorn_iterate_f64(&k, &a, &b, 1e-15, 1);
    assert_eq!(iters, 1);
}

#[test]
fn diagonal_kernel_is_pre_balanced() {
    // k = I. a = b = [2, 3]. Solution is u = a, v = 1/a (after
    // one iteration u settles to a/v0 = a/1 = a; then v = b/u·k_col
    // = b/(u_i for diag) gives 1; further iters fixed.
    let k = vec![1.0, 0.0, 0.0, 1.0];
    let a = vec![2.0, 3.0];
    let b = vec![2.0, 3.0];
    let (u, v, _iters) = sinkhorn_iterate_f64(&k, &a, &b, 1e-10, 100);
    let row_err = sinkhorn_row_residual(&k, &u, &v, &a);
    let col_err = sinkhorn_col_residual(&k, &u, &v, &b);
    assert!(row_err < 1e-9);
    assert!(col_err < 1e-9);
}

#[test]
fn residual_helpers_are_zero_on_perfect_balance() {
    // Construct u, v, k such that diag(u) k diag(v) has row=a, col=b.
    // Simplest: k = ones(2,2), u = [a/2; a/2 ... ] actually
    // run sinkhorn and check.
    let k = vec![1.0, 1.0, 1.0, 1.0];
    let a = vec![1.0, 1.0];
    let b = vec![1.0, 1.0];
    let (u, v, _) = sinkhorn_iterate_f64(&k, &a, &b, 1e-12, 200);
    assert!(sinkhorn_row_residual(&k, &u, &v, &a) < 1e-9);
    assert!(sinkhorn_col_residual(&k, &u, &v, &b) < 1e-9);
}

#[test]
fn convergence_iters_decrease_as_tolerance_relaxes() {
    let k = vec![1.0, 2.0, 3.0, 4.0];
    let a = vec![3.0, 7.0];
    let b = vec![4.0, 6.0];
    let (_, _, tight) = sinkhorn_iterate_f64(&k, &a, &b, 1e-10, 10_000);
    let (_, _, loose) = sinkhorn_iterate_f64(&k, &a, &b, 1e-3, 10_000);
    assert!(
        loose <= tight,
        "looser tolerance should converge no slower (loose={loose}, tight={tight})"
    );
}

#[test]
fn three_by_three_uniform_kernel() {
    let k = vec![1.0; 9];
    let a = vec![1.0, 2.0, 3.0];
    let b = vec![2.0, 2.0, 2.0];
    let (u, v, iters) = sinkhorn_iterate_f64(&k, &a, &b, 1e-9, 1000);
    assert!(iters < 1000);
    assert!(sinkhorn_row_residual(&k, &u, &v, &a) < 1e-7);
    assert!(sinkhorn_col_residual(&k, &u, &v, &b) < 1e-7);
}

#[test]
#[should_panic(expected = "Sinkhorn iterate CPU reference failed")]
fn zero_tolerance_fails_loud() {
    // A non-positive tolerance is rejected by the fallible core; the infallible
    // wrapper must surface that loudly, not silently return an empty state
    // (Law 10/6).
    let _ = sinkhorn_iterate_f64(&[1.0], &[1.0], &[1.0], 0.0, 10);
}

#[test]
#[should_panic(expected = "Sinkhorn iterate CPU reference failed")]
fn shape_mismatch_fails_loud() {
    // A malformed (non-m*n) kernel must fail LOUD, not silently return an empty
    // state a GPU-vs-CPU parity assertion would accept as a match (Law 10/6).
    let _ = sinkhorn_iterate_f64(&[1.0, 2.0], &[1.0, 1.0], &[1.0, 1.0], 1e-6, 10);
}
