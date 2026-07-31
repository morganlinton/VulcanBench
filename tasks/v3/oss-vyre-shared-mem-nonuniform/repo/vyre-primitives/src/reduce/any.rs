//! Boolean any-nonzero reduction over a u32 ValueSet.

crate::reduce::atomic_scalar::define_bool_reduce_op! {
    op_id: "vyre-primitives::reduce::any",
    fn_name: reduce_any,
    kind: AnyNonZero,
    true_case: [0, 0, 1, 0],
    false_case: [0, 0, 0],
    inventory_expected: [1]
}
