#[test]
fn egraph_structural_discovery_uses_borrowed_upload_plan_without_image_clone() {
    let source = include_str!("../../src/egraph_kernel_plan/backend_structural.rs");
    let method_start = source
        .find("pub fn discover_egraph_structural_equivalences")
        .expect("Fix: structural discovery method must remain present.");
    let method_end = source[method_start..]
        .find("    fn run_egraph_structural_equivalence_kernel_inner")
        .map(|offset| method_start + offset)
        .expect("Fix: structural discovery method boundary must remain discoverable.");
    let method = &source[method_start..method_end];

    assert!(
        method.contains("plan_cuda_egraph_device_upload_from_image_ref(&image)")
            && method.contains("upload_egraph_device_image_borrowed_plan"),
        "Fix: CUDA e-graph structural discovery must upload from a borrowed packed image so the same image can feed signature planning without a slab clone."
    );
    assert!(
        !method.contains("image.clone()"),
        "Fix: CUDA e-graph structural discovery must not clone the packed foundation image before upload."
    );
}

#[test]
fn egraph_device_image_upload_uses_resident_io_boundary_not_raw_cuda_ffi() {
    let source = include_str!("../../src/egraph_device_image.rs");

    assert!(
        source.contains("self.allocate_resident")
            && source.contains("upload_egraph_words_to_resident")
            && source.contains("backend.upload_resident"),
        "Fix: e-graph image upload must reuse CUDA resident allocation/upload infrastructure through the staging-free helper."
    );
    for forbidden in ["cuMemAlloc", "cuMemcpyHtoD", "cuMemcpyDtoH"] {
        assert!(
            !source.contains(forbidden),
            "Fix: e-graph image upload must not introduce a raw CUDA FFI branch `{forbidden}`."
        );
    }
}

#[test]
fn egraph_device_image_upload_uses_zero_staging_byte_view_on_little_endian_hosts() {
    let source = include_str!("../../src/egraph_device_image.rs");
    let method_start = source
        .find("pub fn upload_egraph_device_image_plan")
        .expect("Fix: e-graph upload plan method must remain present.");
    let method_end = source[method_start..]
        .find("    /// Resolve a resident e-graph image")
        .map(|offset| method_start + offset)
        .expect("Fix: e-graph upload method boundary must remain discoverable.");
    let method = &source[method_start..method_end];
    let helper_start = source
        .find("fn upload_egraph_words_to_resident")
        .expect("Fix: e-graph upload must use a dedicated word upload helper.");
    let helper_end = source[helper_start..]
        .find("#[cfg(not(target_endian = \"little\"))]")
        .map(|offset| helper_start + offset)
        .expect("Fix: little-endian upload helper boundary must remain discoverable.");
    let helper = &source[helper_start..helper_end];

    assert!(
        method.contains(
            "self.upload_egraph_device_image_words(plan.words(), plan.byte_layout(), plan.byte_len())"
        ) && method.contains("upload_egraph_words_to_resident(self, handle, words)"),
        "Fix: e-graph upload must route owned and borrowed plans through the shared staging-free helper."
    );
    assert!(
        helper.contains("#[cfg(target_endian = \"little\")]")
            && helper.contains("bytemuck::cast_slice(words)")
            && helper.contains("backend.upload_resident(handle"),
        "Fix: little-endian CUDA e-graph upload must cast the packed u32 slab to bytes without per-word staging."
    );
}

#[test]
fn egraph_signature_snapshot_uses_range_readback_not_full_slab_download() {
    let source = include_str!("../../src/egraph_kernel_plan/backend_canonicalization.rs");
    let method_start = source
        .find("pub fn download_egraph_resident_signature_snapshot")
        .expect("Fix: signature snapshot method must remain present.");
    let method_end = source[method_start..]
        .find("    /// Run one CUDA-resident structural canonicalization round")
        .map(|offset| method_start + offset)
        .expect("Fix: signature snapshot method boundary must remain discoverable.");
    let method = &source[method_start..method_end];

    assert!(
        method.contains("download_resident_range"),
        "Fix: signature-only e-graph snapshots must use ranged CUDA readback."
    );
    assert!(
        !method.contains("download_resident(image.handle())"),
        "Fix: signature-only e-graph snapshots must not download the whole resident image."
    );
}

#[test]
fn egraph_column_snapshot_uses_fused_range_readbacks_not_full_slab_download() {
    let source = include_str!("../../src/egraph_kernel_plan/backend_canonicalization.rs");
    let method_start = source
        .find("pub fn download_egraph_resident_column_snapshot")
        .expect("Fix: column snapshot method must remain present.");
    let method_end = source[method_start..]
        .find("    /// Download only the current CUDA-resident row-signature column")
        .map(|offset| method_start + offset)
        .expect("Fix: column snapshot method boundary must remain discoverable.");
    let method = &source[method_start..method_end];

    assert!(
        method.contains("download_resident_ranges_into(&ranges, &mut outputs)"),
        "Fix: full-column e-graph snapshots must use fused ranged CUDA readback for the required planning columns."
    );
    assert!(
        !method.contains("download_resident(image.handle())"),
        "Fix: full-column e-graph snapshots must not download group metadata or unrelated resident slab bytes."
    );
}

#[test]
fn egraph_u32_scratch_upload_uses_zero_staging_byte_view_on_little_endian_hosts() {
    let source = include_str!("../../src/egraph_readback.rs");
    let function_start = source
        .find("fn upload_u32_words(")
        .expect("Fix: e-graph u32 scratch upload helper must remain present.");
    let function_end = source[function_start..]
        .find("fn upload_resident_bytes")
        .map(|offset| function_start + offset)
        .expect("Fix: e-graph u32 scratch upload helper boundary must remain discoverable.");
    let function = &source[function_start..function_end];

    assert!(
        function.contains("upload_u32_words_to_resident")
            && function.contains("#[cfg(target_endian = \"little\")]")
            && function.contains("bytemuck::cast_slice(words)")
            && function.contains("EMPTY_U32_UPLOAD"),
        "Fix: CUDA e-graph u32 scratch metadata upload must avoid host byte staging on little-endian hosts while preserving empty-buffer zero initialization."
    );
}

