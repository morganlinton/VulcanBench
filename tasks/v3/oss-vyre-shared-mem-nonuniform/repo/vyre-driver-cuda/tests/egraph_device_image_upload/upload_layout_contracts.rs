use super::*;

#[test]
fn egraph_device_image_upload_plan_preserves_single_slab_layout() {
    let snapshot = GpuEGraphSnapshot::build([
        (2u32, "lit", &[][..]),
        (1u32, "lit", &[][..]),
        (2u32, "add", &[1u32, 2u32][..]),
    ]);

    let plan = plan_cuda_egraph_device_upload(&snapshot)
        .expect("Fix: valid foundation e-graph image must produce a CUDA upload plan");
    let layout = plan.byte_layout();

    assert_eq!(plan.byte_len(), plan.words().len() * 4);
    assert_eq!(layout.row_count(), 3);
    assert_eq!(layout.child_count(), 2);
    assert_eq!(layout.eclass_group_count(), 2);
    assert_eq!(layout.row_eclass_ids().offset(), 0);
    assert_eq!(layout.row_eclass_ids().byte_len(), 12);
    assert_eq!(layout.row_language_op_ids().offset(), 12);
    assert_eq!(layout.row_children_offsets().offset(), 24);
    assert_eq!(layout.row_children_lens().offset(), 36);
    assert_eq!(layout.row_signatures().offset(), 48);
    assert_eq!(layout.row_signatures().byte_len(), 12);
    assert_eq!(layout.children().offset(), 60);
    assert_eq!(layout.children().byte_len(), 8);
    assert_eq!(layout.group_eclass_ids().offset(), 68);
    assert_eq!(layout.group_offsets().offset(), 76);
    assert_eq!(layout.group_rows().offset(), 88);
}

#[test]
fn borrowed_egraph_device_image_upload_plan_matches_owned_plan_without_image_clone() {
    let snapshot = GpuEGraphSnapshot::build([
        (2u32, "lit", &[][..]),
        (1u32, "lit", &[][..]),
        (2u32, "add", &[1u32, 2u32][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: valid foundation e-graph image must pack.");
    let owned = plan_cuda_egraph_device_upload_from_image(image.clone())
        .expect("Fix: owned CUDA e-graph upload plan must build.");
    let borrowed = plan_cuda_egraph_device_upload_from_image_ref(&image)
        .expect("Fix: borrowed CUDA e-graph upload plan must build.");

    assert_eq!(borrowed.words(), owned.words());
    assert_eq!(borrowed.byte_layout(), owned.byte_layout());
    assert_eq!(borrowed.byte_len(), owned.byte_len());
}

#[test]
fn cuda_upload_byte_layout_matches_foundation_device_image_layout() {
    let snapshot = GpuEGraphSnapshot::build([
        (2u32, "lit", &[][..]),
        (1u32, "lit", &[][..]),
        (2u32, "add", &[1u32, 2u32][..]),
        (3u32, "mul", &[2u32, 1u32][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: valid foundation e-graph image must pack.");
    let plan = plan_cuda_egraph_device_upload_from_image_ref(&image)
        .expect("Fix: CUDA borrowed upload plan must derive from foundation image layout.");
    let cuda = plan.byte_layout();
    let foundation = image.layout();

    assert_eq!(plan.words(), image.words());
    assert_eq!(plan.byte_len(), image.words().len() * 4);
    assert_eq!(cuda.row_count(), foundation.row_count());
    assert_eq!(cuda.child_count(), foundation.child_count());
    assert_eq!(cuda.eclass_group_count(), foundation.eclass_group_count());
    assert_span_matches_foundation(cuda.row_eclass_ids(), foundation.row_eclass_ids());
    assert_span_matches_foundation(cuda.row_language_op_ids(), foundation.row_language_op_ids());
    assert_span_matches_foundation(
        cuda.row_children_offsets(),
        foundation.row_children_offsets(),
    );
    assert_span_matches_foundation(cuda.row_children_lens(), foundation.row_children_lens());
    assert_span_matches_foundation(cuda.row_signatures(), foundation.row_signatures());
    assert_span_matches_foundation(cuda.children(), foundation.children());
    assert_span_matches_foundation(cuda.group_eclass_ids(), foundation.group_eclass_ids());
    assert_span_matches_foundation(cuda.group_offsets(), foundation.group_offsets());
    assert_span_matches_foundation(cuda.group_rows(), foundation.group_rows());
}

#[test]
fn egraph_device_image_upload_plan_rejects_malformed_snapshot() {
    let mut snapshot = GpuEGraphSnapshot::build([(0u32, "lit", &[][..])]);
    snapshot.rows[0].language_op_id = 99;

    let error = plan_cuda_egraph_device_upload(&snapshot)
        .expect_err("Fix: CUDA upload planning must reject malformed e-graph images");

    match error {
        CudaEGraphDeviceUploadError::Image(GpuEGraphDeviceImageError::Integrity(error)) => {
            assert_eq!(error.context(), "unknown language_op_id");
            assert_eq!(error.row(), 0);
            assert_eq!(error.value(), 99);
        }
        other => panic!("expected integrity error from foundation image packer, got {other}"),
    }
}

fn assert_span_matches_foundation(
    cuda: CudaEGraphDeviceByteSpan,
    foundation: vyre_foundation::optimizer::eqsat_gpu::GpuEGraphDeviceSpan,
) {
    assert_eq!(cuda.offset(), foundation.offset() * 4);
    assert_eq!(cuda.byte_len(), foundation.len() * 4);
}

#[test]
fn borrowed_egraph_device_image_upload_round_trips_through_cuda_resident_memory() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let snapshot = GpuEGraphSnapshot::build([
        (10u32, "lit", &[][..]),
        (20u32, "lit", &[][..]),
        (30u32, "add", &[10u32, 20u32][..]),
        (40u32, "add", &[10u32, 20u32][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: valid foundation e-graph image must pack.");
    let borrowed = plan_cuda_egraph_device_upload_from_image_ref(&image)
        .expect("Fix: borrowed CUDA e-graph upload plan must build.");
    let expected_bytes = borrowed
        .words()
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();

    let resident = backend
        .upload_egraph_device_image_borrowed_plan(borrowed)
        .expect("Fix: borrowed CUDA e-graph resident image upload failed.");
    let output = backend
        .download_resident(resident.handle())
        .expect("Fix: borrowed CUDA e-graph resident image download failed.");

    assert_eq!(output, expected_bytes);
    assert_eq!(resident.byte_len(), borrowed.byte_len());
    assert_eq!(resident.word_count(), borrowed.words().len());

    backend
        .free_resident(resident.handle())
        .expect("Fix: borrowed CUDA e-graph resident image free failed.");
}

#[test]
fn egraph_device_image_upload_round_trips_through_cuda_resident_memory() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let snapshot = GpuEGraphSnapshot::build([
        (2u32, "lit", &[][..]),
        (1u32, "lit", &[][..]),
        (2u32, "add", &[1u32, 2u32][..]),
    ]);
    let plan = plan_cuda_egraph_device_upload(&snapshot)
        .expect("Fix: valid foundation e-graph image must produce a CUDA upload plan");
    let expected_bytes = plan
        .words()
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();

    let resident = backend
        .upload_egraph_device_image_plan(plan)
        .expect("Fix: CUDA e-graph resident image upload failed.");
    let output = backend
        .download_resident(resident.handle())
        .expect("Fix: CUDA e-graph resident image download failed.");

    assert_eq!(resident.byte_len(), expected_bytes.len());
    assert_eq!(resident.word_count(), expected_bytes.len() / 4);
    assert_eq!(output, expected_bytes);

    let view = backend
        .egraph_device_kernel_view(resident)
        .expect("Fix: resident e-graph image must resolve to checked kernel pointers.");
    assert_ne!(view.base_ptr(), 0);
    assert_eq!(view.byte_len(), expected_bytes.len());
    assert_eq!(view.row_count(), 3);
    assert_eq!(view.child_count(), 2);
    assert_eq!(view.eclass_group_count(), 2);
    assert_eq!(view.row_eclass_ids_ptr(), view.base_ptr());
    assert_eq!(view.row_language_op_ids_ptr(), view.base_ptr() + 12);
    assert_eq!(view.row_children_offsets_ptr(), view.base_ptr() + 24);
    assert_eq!(view.row_children_lens_ptr(), view.base_ptr() + 36);
    assert_eq!(view.row_signatures_ptr(), view.base_ptr() + 48);
    assert_eq!(view.children_ptr(), view.base_ptr() + 60);
    assert_eq!(view.group_eclass_ids_ptr(), view.base_ptr() + 68);
    assert_eq!(view.group_offsets_ptr(), view.base_ptr() + 76);
    assert_eq!(view.group_rows_ptr(), view.base_ptr() + 88);

    backend
        .free_resident(resident.handle())
        .expect("Fix: CUDA e-graph resident image free failed.");
}

