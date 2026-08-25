use super::*;

#[test]
fn structural_equivalence_kernel_ptx_pins_entry_abi_and_target() {
    let kernel = cuda_egraph_structural_equivalence_kernel_ptx(90)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid CUDA egraph structural-equivalence PTX must emit");

    assert_eq!(kernel.target_sm, 90);
    assert_eq!(kernel.ptx_version, "8.0");
    assert_eq!(
        kernel.entry_name,
        CUDA_EGRAPH_STRUCTURAL_EQUIVALENCE_KERNEL_ENTRY
    );
    assert_eq!(
        kernel.parameter_count,
        CUDA_EGRAPH_STRUCTURAL_EQUIVALENCE_KERNEL_PARAM_COUNT
    );
    assert_eq!(
        kernel.bucket_record_words,
        CUDA_EGRAPH_SIGNATURE_BUCKET_RECORD_WORDS
    );
    assert!(kernel.source.contains(".version 8.0"));
    assert!(kernel.source.contains(".target sm_90"));
    assert!(kernel.source.contains(".visible .entry main("));
    for param in [
        "row_eclass_ids_ptr",
        "row_language_op_ids_ptr",
        "row_children_offsets_ptr",
        "row_children_lens_ptr",
        "row_signatures_ptr",
        "children_ptr",
        "bucket_words_ptr",
        "bucket_rows_ptr",
        "output_pairs_ptr",
        "output_count_ptr",
        "bucket_index",
        "first_pair",
        "pair_count",
    ] {
        assert!(
            kernel.source.contains(param),
            "Fix: structural-equivalence PTX ABI must include parameter `{param}`."
        );
    }
}

#[test]
fn structural_equivalence_kernel_ptx_contains_non_stub_exact_compare_body() {
    let kernel = cuda_egraph_structural_equivalence_kernel_ptx(120)
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid CUDA egraph structural-equivalence PTX must emit");

    assert_eq!(kernel.ptx_version, "8.7");
    for required in [
        "PAIR_DECODE_LOOP:",
        "CHILD_LOOP:",
        "ld.global.u32",
        "setp.ne.u32",
        "atom.global.add.u64",
        "st.global.u32",
        "selp.u32",
    ] {
        assert!(
                kernel.source.contains(required),
                "Fix: structural-equivalence PTX must contain real exact-compare/output logic `{required}`."
            );
    }
    let ret_index = kernel
        .source
        .find("ret;")
        .expect("Fix: structural-equivalence PTX must return.");
    let first_load_index = kernel
        .source
        .find("ld.global.u32")
        .expect("Fix: structural-equivalence PTX must load packed columns before returning.");
    assert!(
        first_load_index < ret_index,
        "Fix: structural-equivalence PTX must not be a return-only stub."
    );
}

#[test]
fn structural_equivalence_kernel_ptx_rejects_invalid_sm_target() {
    assert_eq!(
        cuda_egraph_structural_equivalence_kernel_ptx(0)
            .expect_err("sm_0 is not a valid CUDA PTX target"),
        CudaEGraphKernelPlanError::InvalidPtxTarget { target_sm: 0 }
    );
}

#[test]

fn signature_bucket_planner_rejects_mismatched_image_and_view() {
    let image = GpuEGraphSnapshot::build([(0u32, "lit", &[][..]), (1u32, "lit", &[][..])])
        .try_pack_device_image()
        .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid egraph image must pack");
    let mismatched_view = synthetic_view(1, 0, 1);

    assert_eq!(
        plan_cuda_egraph_signature_buckets(
            &image,
            mismatched_view,
            CudaEGraphKernelLaunchConfig::default(),
        )
        .expect_err("image/view row mismatch must be rejected"),
        CudaEGraphKernelPlanError::ImageViewMismatch {
            field: "row count",
            image: 2,
            view: 1,
        }
    );
}
