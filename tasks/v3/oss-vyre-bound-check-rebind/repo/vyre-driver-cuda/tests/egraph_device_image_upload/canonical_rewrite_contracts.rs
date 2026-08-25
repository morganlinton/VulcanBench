use super::*;

#[test]
fn egraph_canonical_rewrite_kernel_updates_live_cuda_resident_image() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let snapshot = GpuEGraphSnapshot::build([
        (10u32, "lit", &[][..]),
        (20u32, "lit", &[][..]),
        (30u32, "add", &[10u32, 20u32][..]),
        (40u32, "add", &[20u32, 10u32][..]),
        (50u32, "mul", &[30u32, 40u32][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: valid foundation e-graph image must pack.");
    let upload_plan = plan_cuda_egraph_device_upload_from_image(image)
        .expect("Fix: packed e-graph image must produce a CUDA upload plan.");
    let byte_layout = upload_plan.byte_layout();
    let resident = backend
        .upload_egraph_device_image_plan(upload_plan)
        .expect("Fix: CUDA e-graph resident image upload failed.");
    let union_plan = plan_cuda_egraph_union_compaction(
        &[
            Equivalence {
                left: 20,
                right: 10,
            },
            Equivalence {
                left: 40,
                right: 30,
            },
        ],
        CudaEGraphKernelLaunchConfig {
            threads_per_block: 3,
            max_blocks_per_launch: 2,
        },
    )
    .expect("Fix: CUDA e-graph union compaction plan must build.");
    let rewrite_image = pack_cuda_egraph_canonical_rewrite_device_image(&union_plan)
        .expect("Fix: CUDA e-graph canonical rewrite image must pack.");

    let result = backend
        .run_egraph_canonical_rewrite_kernel(
            resident,
            &rewrite_image,
            CudaEGraphKernelLaunchConfig {
                threads_per_block: 3,
                max_blocks_per_launch: 2,
            },
        )
        .expect("Fix: live CUDA canonical rewrite kernel must update resident e-graph image.");
    let output = backend
        .download_resident(resident.handle())
        .expect("Fix: CUDA e-graph resident image download failed after canonical rewrite.");

    assert_eq!(result.rewrite_count, 2);
    assert_eq!(result.row_count, 5);
    assert_eq!(result.child_count, 6);
    assert_eq!(result.launch_count, 2);
    assert_eq!(result.total_items, 11);
    assert_eq!(
        read_u32_span(&output, byte_layout.row_eclass_ids(), 5),
        vec![10, 10, 30, 30, 50]
    );
    assert_eq!(
        read_u32_span(&output, byte_layout.children(), 6),
        vec![10, 10, 10, 10, 30, 30]
    );

    backend
        .free_resident(resident.handle())
        .expect("Fix: CUDA e-graph resident image free failed.");
}

#[test]
fn egraph_structural_canonicalization_round_discovers_and_rewrites_live_cuda_image() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let snapshot = GpuEGraphSnapshot::build([
        (10u32, "lit", &[][..]),
        (20u32, "lit", &[][..]),
        (30u32, "add", &[10u32, 20u32][..]),
        (40u32, "add", &[10u32, 20u32][..]),
        (50u32, "mul", &[30u32, 40u32][..]),
        (60u32, "mul", &[30u32, 40u32][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: valid foundation e-graph image must pack.");
    let upload_plan = plan_cuda_egraph_device_upload_from_image(image.clone())
        .expect("Fix: packed e-graph image must produce a CUDA upload plan.");
    let byte_layout = upload_plan.byte_layout();
    let resident = backend
        .upload_egraph_device_image_plan(upload_plan)
        .expect("Fix: CUDA e-graph resident image upload failed.");

    let result = backend
        .run_egraph_structural_canonicalization_round(
            resident,
            &image,
            CudaEGraphKernelLaunchConfig {
                threads_per_block: 4,
                max_blocks_per_launch: 2,
            },
        )
        .expect("Fix: live CUDA e-graph canonicalization round must discover, plan, and rewrite.");
    let output = backend
        .download_resident(resident.handle())
        .expect("Fix: CUDA e-graph resident image download failed after canonicalization round.");

    assert_eq!(
        result.discovery.unique,
        vec![
            Equivalence {
                left: 10,
                right: 20,
            },
            Equivalence {
                left: 30,
                right: 40,
            },
            Equivalence {
                left: 50,
                right: 60,
            },
        ]
    );
    assert_eq!(result.union_plan.canonical_rewrites.len(), 3);
    assert_eq!(result.rewrite.rewrite_count, 3);
    assert_eq!(result.rewrite.row_count, 6);
    assert_eq!(result.rewrite.child_count, 8);
    assert_eq!(result.signature_refresh.row_count, 6);
    assert_eq!(result.signature_refresh.total_rows, 6);
    assert_eq!(
        read_u32_span(&output, byte_layout.row_eclass_ids(), 6),
        vec![10, 10, 30, 30, 50, 50]
    );
    assert_eq!(
        read_u32_span(&output, byte_layout.children(), 8),
        vec![10, 10, 10, 10, 30, 30, 30, 30]
    );

    backend
        .free_resident(resident.handle())
        .expect("Fix: CUDA e-graph resident image free failed.");
}

#[test]

fn egraph_signature_refresh_exposes_post_rewrite_structural_duplicates() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let snapshot = GpuEGraphSnapshot::build([
        (10u32, "lit", &[][..]),
        (20u32, "lit", &[][..]),
        (30u32, "add", &[10u32, 10u32][..]),
        (40u32, "add", &[10u32, 20u32][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: valid foundation e-graph image must pack.");
    assert_ne!(
        image.row_signatures()[2],
        image.row_signatures()[3],
        "Fix: this fixture must require canonical rewrite before rows become structural duplicates."
    );
    let upload_plan = plan_cuda_egraph_device_upload_from_image(image.clone())
        .expect("Fix: packed e-graph image must produce a CUDA upload plan.");
    let byte_layout = upload_plan.byte_layout();
    let resident = backend
        .upload_egraph_device_image_plan(upload_plan)
        .expect("Fix: CUDA e-graph resident image upload failed.");

    let result = backend
        .run_egraph_structural_canonicalization_round(
            resident,
            &image,
            CudaEGraphKernelLaunchConfig {
                threads_per_block: 2,
                max_blocks_per_launch: 2,
            },
        )
        .expect("Fix: live CUDA canonicalization round must refresh row signatures after rewrite.");
    let output = backend
        .download_resident(resident.handle())
        .expect("Fix: CUDA e-graph resident image download failed after signature refresh.");
    let row_signatures = read_u32_span(&output, byte_layout.row_signatures(), 4);

    assert_eq!(
        result.discovery.unique,
        vec![Equivalence {
            left: 10,
            right: 20,
        }]
    );
    assert_eq!(result.rewrite.rewrite_count, 1);
    assert_eq!(result.signature_refresh.row_count, 4);
    assert_eq!(result.signature_refresh.total_rows, 4);
    assert_eq!(
        read_u32_span(&output, byte_layout.children(), 4),
        vec![10, 10, 10, 10]
    );
    assert_eq!(
        row_signatures[2], row_signatures[3],
        "Fix: CUDA signature refresh must expose duplicates created by canonical child rewrites."
    );

    backend
        .free_resident(resident.handle())
        .expect("Fix: CUDA e-graph resident image free failed.");
}

