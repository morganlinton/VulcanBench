//! Unsigned maximum reduction over a u32 ValueSet.

crate::reduce::atomic_scalar::define_u32_reduce_op! {
    op_id: "vyre-primitives::reduce::max",
    fn_name: reduce_max,
    kind: Max,
    identity: 0,
    fold: u32::max,
    sample: [9, 3, 7, 5],
    expected: 9
}
