//! Boolean all-nonzero reduction over a u32 ValueSet.

crate::reduce::atomic_scalar::define_bool_reduce_op! {
    op_id: "vyre-primitives::reduce::all",
    fn_name: reduce_all,
    kind: AllNonZero,
    true_case: [1, 7, 9],
    false_case: [1, 0, 9],
    inventory_expected: [0]
}
