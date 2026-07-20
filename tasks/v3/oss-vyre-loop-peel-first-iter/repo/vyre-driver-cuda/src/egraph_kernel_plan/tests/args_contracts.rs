use super::*;

#[test]
fn egraph_kernel_args_into_reuses_capacity_and_preserves_abi_order() {
    let mut table = smallvec::SmallVec::<[*mut std::ffi::c_void; 8]>::new();
    let mut structural = EGraphStructuralKernelArgs {
        row_eclass_ids_ptr: 1,
        row_language_op_ids_ptr: 2,
        row_children_offsets_ptr: 3,
        row_children_lens_ptr: 4,
        row_signatures_ptr: 5,
        children_ptr: 6,
        bucket_words_ptr: 7,
        bucket_rows_ptr: 8,
        output_pairs_ptr: 9,
        output_count_ptr: 10,
        bucket_index: 11,
        first_pair: 12,
        pair_count: 13,
    };

    structural
        .write_kernel_args_into(&mut table)
        .expect("Fix: structural e-graph kernel args should build");
    let capacity = table.capacity();
    assert_eq!(table.len(), 13);
    assert_eq!(
        table[0],
        &mut structural.row_eclass_ids_ptr as *mut _ as *mut std::ffi::c_void
    );
    assert_eq!(
        table[12],
        &mut structural.pair_count as *mut _ as *mut std::ffi::c_void
    );

    let mut rewrite = EGraphCanonicalRewriteKernelArgs {
        row_eclass_ids_ptr: 21,
        children_ptr: 22,
        rewrite_words_ptr: 23,
        rewrite_count: 24,
        row_count: 25,
        child_count: 26,
        first_item: 27,
    };
    rewrite
        .write_kernel_args_into(&mut table)
        .expect("Fix: canonical rewrite e-graph kernel args should reuse table");
    assert_eq!(table.capacity(), capacity);
    assert_eq!(table.len(), 7);
    assert_eq!(
        table[0],
        &mut rewrite.row_eclass_ids_ptr as *mut _ as *mut std::ffi::c_void
    );
    assert_eq!(
        table[6],
        &mut rewrite.first_item as *mut _ as *mut std::ffi::c_void
    );
}

