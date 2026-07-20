use super::*;

#[test]
fn test_sinkhorn_cpu_ref_trivial() {
    let (u, v, _iters) = cpu_ref(
        &[65536],
        &[65536],
        &[65536],
        &[65536],
        &[65536],
        &[65536],
        1,
        1,
        10,
    );
    assert_eq!(u, vec![65536]);
    assert_eq!(v, vec![65536]);
}

#[test]
fn test_sinkhorn_cpu_ref_edge() {
    // u = a / (k * v) = 65536 / (32768 * 65536) = 65536 / 2^31 = 0
    let (u, _, _) = cpu_ref(
        &[32768],
        &[32768],
        &[65536],
        &[65536],
        &[65536],
        &[65536],
        1,
        1,
        10,
    );
    assert_eq!(u, vec![0]);
}

#[test]
fn test_sinkhorn_cpu_ref_normal() {
    let k = vec![65536, 65536, 65536, 65536];
    let k_t = vec![65536, 65536, 65536, 65536];
    let a = vec![32768, 32768];
    let b = vec![32768, 32768];
    let u_c = vec![65536, 65536];
    let v_in = vec![65536, 65536];
    let (u, _v, _) = cpu_ref(&k, &k_t, &a, &b, &u_c, &v_in, 2, 2, 5);
    // Kv = [0, 0] wrapped. u = a/1 = 32768.
    assert_eq!(u, vec![32768, 32768]);
}

#[test]
fn test_sinkhorn_cpu_ref_large() {
    let k = vec![65536; 9];
    let a = vec![65536; 3];
    let b = vec![65536; 3];
    let u_c = vec![65536; 3];
    let v_in = vec![65536; 3];
    let (u, _, _) = cpu_ref(&k, &k, &a, &b, &u_c, &v_in, 3, 3, 5);
    assert_eq!(u.len(), 3);
}

#[test]
fn test_sinkhorn_cpu_ref_asym() {
    let k = vec![65536, 0, 0, 65536, 65536, 65536];
    let k_t = vec![65536, 0, 65536, 0, 65536, 65536];
    let a = vec![32768, 32768, 65536];
    let b = vec![65536, 65536];
    let u_c = vec![65536, 65536, 65536];
    let v_in = vec![65536, 65536];
    let (u, v, _) = cpu_ref(&k, &k_t, &a, &b, &u_c, &v_in, 3, 2, 5);
    assert_eq!(u.len(), 3);
    assert_eq!(v.len(), 2);
}

#[test]
fn test_sinkhorn_cpu_ref_into_reuses_buffers() {
    let k = vec![65536, 65536, 65536, 65536];
    let a = vec![32768, 32768];
    let b = vec![32768, 32768];
    let u_c = vec![65536, 65536];
    let v_in = vec![65536, 65536];
    let mut u = Vec::with_capacity(8);
    let mut v = Vec::with_capacity(8);
    let mut u_old = Vec::with_capacity(8);
    let u_ptr = u.as_ptr();
    let v_ptr = v.as_ptr();
    let old_ptr = u_old.as_ptr();
    let _iters = cpu_ref_into(
        &k, &k, &a, &b, &u_c, &v_in, 2, 2, 5, &mut u, &mut v, &mut u_old,
    );
    assert_eq!(u, vec![32768, 32768]);
    assert_eq!(u.as_ptr(), u_ptr);
    assert_eq!(v.as_ptr(), v_ptr);
    assert_eq!(u_old.as_ptr(), old_ptr);
}

#[test]
fn test_sinkhorn_cpu_ref_into_truncates_stale_buffers() {
    let k = vec![65536, 65536, 65536, 65536];
    let a = vec![32768, 32768];
    let b = vec![32768, 32768];
    let u_c = vec![65536, 65536];
    let v_in = vec![65536, 65536];
    let mut u = Vec::with_capacity(8);
    let mut v = Vec::with_capacity(8);
    let mut u_old = Vec::with_capacity(8);
    u.extend([99u32; 8]);
    v.extend([99u32; 8]);
    u_old.extend([99u32; 8]);
    let u_ptr = u.as_ptr();
    let v_ptr = v.as_ptr();
    let old_ptr = u_old.as_ptr();

    let _iters = try_cpu_ref_into(
        &k, &k, &a, &b, &u_c, &v_in, 2, 2, 5, &mut u, &mut v, &mut u_old,
    )
    .unwrap();

    assert_eq!(u, vec![32768, 32768]);
    assert_eq!(u.as_ptr(), u_ptr);
    assert_eq!(v.as_ptr(), v_ptr);
    assert_eq!(u_old.as_ptr(), old_ptr);
}

#[test]
fn test_sinkhorn_try_cpu_ref_rejects_short_buffers() {
    let err = try_cpu_ref(&[1], &[1], &[1, 1], &[1, 1], &[1, 1], &[1, 1], 2, 2, 1).unwrap_err();
    assert!(err.contains("buffer `k` is too short"), "{err}");
}

#[test]
fn test_sinkhorn_program_parity() {
    let k = vec![1, 1, 1, 1];
    let a = vec![10, 10];
    let b = vec![10, 10];
    let u_c = vec![1, 1];
    let v_in = vec![1, 1];

    let p = sinkhorn_iterate(
        "k", "kt", "a", "b", "uc", "un", "v", "kv", "ktu", "c", 2, 2, 1,
    );

    let (expected_u, _, _) = cpu_ref(&k, &k, &a, &b, &u_c, &v_in, 2, 2, 1);

    use vyre_reference::reference_eval;
    use vyre_reference::value::Value;

    let to_value = |data: &[u32]| {
        let bytes = crate::wire::pack_u32_slice(data);
        Value::Bytes(Arc::from(bytes))
    };

    let inputs = vec![
        to_value(&u_c),
        to_value(&[0_u32, 0]),
        to_value(&[0]),
        to_value(&k),
        to_value(&k),
        to_value(&a),
        to_value(&b),
        to_value(&v_in),
        to_value(&[0_u32, 0]),
        to_value(&[0_u32, 0]),
    ];

    let results = reference_eval(&p, &inputs).expect("Fix: interpreter failed");
    let actual_bytes = results[0].to_bytes();
    let actual_u: Vec<u32> = actual_bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    assert_eq!(actual_u, expected_u);
}

#[test]
fn program_declares_ten_buffers() {
    let p = sinkhorn_iterate(
        "k", "kt", "a", "b", "uc", "un", "v", "kv", "ktu", "c", 2, 2, 5,
    );
    assert_eq!(p.buffers().len(), 10);
}
