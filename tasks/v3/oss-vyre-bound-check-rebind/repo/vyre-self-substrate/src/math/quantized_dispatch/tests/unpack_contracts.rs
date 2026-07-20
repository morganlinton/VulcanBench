use super::*;

#[test]
fn unpack_i4x8_via_dispatches_signed_boundaries() {
    let values = [-8, -7, -1, 0, 1, 2, 6, 7, -3, 4, 5, -5, -6, 3, -2, 0, 7];
    let packed = pack_i4x8_cpu(&values);

    let out = unpack_i4x8_via(&QuantizedDispatcher, &packed, values.len() as u32)
        .expect("Fix: CUDA parity tests require backend dispatch; skip test if GPU unavailable, do not panic - fake dispatcher unpacks signed INT4 lanes");

    assert_eq!(out, values);
}

#[test]
fn unpack_i4x8_via_reuses_scratch_and_output() {
    let values = [-8, -1, 0, 7, 3, -3, 6, -6];
    let packed = pack_i4x8_cpu(&values);
    let mut scratch = QuantizedUnpackGpuScratch {
        inputs: vec![Vec::with_capacity(64), Vec::with_capacity(64)],
        program_cache: ProgramCache::default(),
    };
    let mut out = Vec::with_capacity(16);
    let input_ptrs = scratch.inputs.iter().map(Vec::as_ptr).collect::<Vec<_>>();
    let out_ptr = out.as_ptr();

    unpack_i4x8_via_with_scratch_into(
        &QuantizedDispatcher,
        &packed,
        values.len() as u32,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - first unpack succeeds");
    assert_eq!(
            scratch.program_cache.builds(),
            1,
            "Fix: first quantized dispatch should build exactly one shape-specialized primitive Program."
        );
    unpack_i4x8_via_with_scratch_into(
        &QuantizedDispatcher,
        &packed,
        values.len() as u32,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - second unpack reuses buffers");
    assert_eq!(
            scratch.program_cache.builds(),
            1,
            "Fix: repeated quantized dispatch with the same lane shape must reuse the primitive Program."
        );

    assert_eq!(out, values);
    for (before, after) in input_ptrs
        .iter()
        .zip(scratch.inputs.iter().map(Vec::as_ptr))
    {
        assert_eq!(*before, after);
    }
    assert_eq!(out.as_ptr(), out_ptr);
}

#[test]
fn unpack_i4x8_via_rebuilds_cached_program_only_on_lane_shape_change() {
    let values8 = [-8, -1, 0, 7, 3, -3, 6, -6];
    let values9 = [-8, -1, 0, 7, 3, -3, 6, -6, 2];
    let packed8 = pack_i4x8_cpu(&values8);
    let packed9 = pack_i4x8_cpu(&values9);
    let mut scratch = QuantizedUnpackGpuScratch::default();
    let mut out = Vec::new();

    unpack_i4x8_via_with_scratch_into(
        &QuantizedDispatcher,
        &packed8,
        values8.len() as u32,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - first shape succeeds");
    unpack_i4x8_via_with_scratch_into(
        &QuantizedDispatcher,
        &packed8,
        values8.len() as u32,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - same shape succeeds");
    unpack_i4x8_via_with_scratch_into(
        &QuantizedDispatcher,
        &packed9,
        values9.len() as u32,
        &mut scratch,
        &mut out,
    )
    .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - changed shape succeeds");

    assert_eq!(out, values9);
    assert_eq!(
            scratch.program_cache.builds(),
            2,
            "Fix: quantized dispatch should rebuild the primitive Program only when lane_count changes."
        );
}

#[test]
fn unpack_i4x8_via_rejects_shape_errors_before_dispatch() {
    let err =
        unpack_i4x8_via(&QuantizedDispatcher, &[], 1).expect_err("missing packed word must fail");
    assert!(err.to_string().contains("packed_words.len()"));

    let err = unpack_i4x8_via(&QuantizedDispatcher, &[0], 0).expect_err("zero lanes must fail");
    assert!(err.to_string().contains("lane_count > 0"));
}

