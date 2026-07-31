//! Unsigned minimum reduction over a u32 ValueSet.

crate::reduce::atomic_scalar::define_u32_reduce_op! {
    op_id: "vyre-primitives::reduce::min",
    fn_name: reduce_min,
    kind: Min,
    identity: u32::MAX,
    fold: u32::min,
    sample: [9, 3, 7, 5],
    expected: 3
}
