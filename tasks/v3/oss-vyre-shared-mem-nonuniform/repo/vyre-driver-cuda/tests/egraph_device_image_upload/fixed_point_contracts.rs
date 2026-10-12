use super::*;

#[test]
fn egraph_structural_canonicalization_fixed_point_chases_chained_cuda_duplicates() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let snapshot = GpuEGraphSnapshot::build([
        (10u32, "lit", &[][..]),
        (20u32, "lit", &[][..]),
        (30u32, "add", &[10u32, 10u32][..]),
        (40u32, "add", &[10u32, 20u32][..]),
        (50u32, "mul", &[30u32, 30u32][..]),
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
        .run_egraph_structural_canonicalization_fixed_point(
            resident,
            &image,
            CudaEGraphKernelLaunchConfig {
                threads_per_block: 2,
                max_blocks_per_launch: 2,
            },
            5,
        )
        .expect("Fix: live CUDA fixed-point canonicalization must chase chained duplicates.");
    let output = backend
        .download_resident(resident.handle())
        .expect("Fix: CUDA e-graph resident image download failed after fixed point.");
    let signature_snapshot = backend
        .download_egraph_resident_signature_snapshot(resident)
        .expect("Fix: CUDA e-graph signature-only snapshot must read refreshed signatures.");

    assert!(result.converged);
    assert_eq!(result.rounds.len(), 4);
    assert_eq!(result.total_discovered_pairs, 3);
    assert_eq!(result.total_rewrites, 3);
    assert_eq!(
        result.rounds[0].discovery.unique,
        vec![Equivalence {
            left: 10,
            right: 20,
        }]
    );
    assert_eq!(
        result.rounds[1].discovery.unique,
        vec![Equivalence {
            left: 30,
            right: 40,
        }]
    );
    assert_eq!(
        result.rounds[2].discovery.unique,
        vec![Equivalence {
            left: 50,
            right: 60,
        }]
    );
    assert!(result.rounds[3].discovery.unique.is_empty());
    assert!(result.rounds[3].union_plan.canonical_pairs.is_empty());
    assert_eq!(result.rounds[3].union_plan.canonical_rewrites.len(), 0);
    assert!(result.rounds[3].union_plan.waves.is_empty());
    assert_eq!(result.rounds[3].union_plan.total_items, 0);
    assert_eq!(result.rounds[3].union_plan.total_blocks, 0);
    assert_eq!(result.rounds[3].rewrite.rewrite_count, 0);
    assert_eq!(result.rounds[3].rewrite.launch_count, 0);
    assert_eq!(result.rounds[3].rewrite.total_items, 0);
    assert_eq!(result.rounds[3].signature_refresh.launch_count, 0);
    assert_eq!(result.rounds[3].signature_refresh.total_rows, 0);
    assert_eq!(
        read_u32_span(&output, byte_layout.row_eclass_ids(), 6),
        vec![10, 10, 30, 30, 50, 50]
    );
    assert_eq!(
        read_u32_span(&output, byte_layout.children(), 8),
        vec![10, 10, 10, 10, 30, 30, 30, 30]
    );
    assert_eq!(
        result.final_snapshot.row_eclass_ids,
        vec![10, 10, 30, 30, 50, 50]
    );
    assert_eq!(
        result.final_snapshot.children,
        vec![10, 10, 10, 10, 30, 30, 30, 30]
    );
    assert_eq!(
        signature_snapshot.row_signatures,
        result.final_snapshot.row_signatures
    );
    assert_eq!(
        signature_snapshot.child_count(),
        result.final_snapshot.child_count()
    );

    backend
        .free_resident(resident.handle())
        .expect("Fix: CUDA e-graph resident image free failed.");
}

#[test]
fn egraph_fixed_point_signature_readback_skips_full_final_snapshot() {
    let backend =
        CudaBackend::acquire().expect("Fix: CUDA backend acquire failed on a GPU-required host.");
    let snapshot = GpuEGraphSnapshot::build([
        (10u32, "lit", &[][..]),
        (20u32, "lit", &[][..]),
        (30u32, "add", &[10u32, 10u32][..]),
        (40u32, "add", &[10u32, 20u32][..]),
        (50u32, "mul", &[30u32, 30u32][..]),
        (60u32, "mul", &[30u32, 40u32][..]),
    ]);
    let image = snapshot
        .try_pack_device_image()
        .expect("Fix: valid foundation e-graph image must pack.");

    for final_readback in [
        CudaEGraphFixedPointReadback::FullColumns,
        CudaEGraphFixedPointReadback::Signatures,
        CudaEGraphFixedPointReadback::None,
    ] {
        let upload_plan = plan_cuda_egraph_device_upload_from_image(image.clone())
            .expect("Fix: packed e-graph image must produce a CUDA upload plan.");
        let expected_full_bytes = expected_column_snapshot_bytes(upload_plan.byte_layout());
        let expected_signature_bytes = upload_plan.byte_layout().row_signatures().byte_len();
        let resident = backend
            .upload_egraph_device_image_plan(upload_plan)
            .expect("Fix: CUDA e-graph resident image upload failed.");

        let result = backend
            .run_egraph_structural_canonicalization_fixed_point_with_readback(
                resident,
                &image,
                CudaEGraphKernelLaunchConfig {
                    threads_per_block: 2,
                    max_blocks_per_launch: 2,
                },
                5,
                final_readback,
            )
            .expect("Fix: live CUDA fixed-point canonicalization must support policy-controlled final readback.");

        assert!(result.converged);
        assert_eq!(result.total_discovered_pairs, 3);
        assert_eq!(result.total_rewrites, 3);
        assert_eq!(result.final_readback, final_readback);
        assert_eq!(result.final_full_readback_bytes, expected_full_bytes);
        assert_eq!(
            result.final_signature_snapshot_bytes,
            expected_signature_bytes
        );
        match final_readback {
            CudaEGraphFixedPointReadback::FullColumns => {
                assert_eq!(result.final_additional_readback_bytes, 0);
                assert_eq!(result.avoided_final_readback_bytes, expected_full_bytes);
                assert!(
                    result.final_snapshot.is_some(),
                    "Fix: full-column fixed-point readback must return final resident columns."
                );
                assert!(
                    result.final_signature_snapshot.is_some(),
                    "Fix: full-column fixed-point readback must expose a derivable signature snapshot."
                );
            }
            CudaEGraphFixedPointReadback::Signatures => {
                let device_signature_snapshot = backend
                    .download_egraph_resident_signature_snapshot(resident)
                    .expect(
                        "Fix: CUDA e-graph signature-only snapshot must read refreshed signatures.",
                    );
                assert_eq!(result.final_additional_readback_bytes, 0);
                assert_eq!(result.avoided_final_readback_bytes, expected_full_bytes);
                assert!(
                    result.final_snapshot.is_none(),
                    "Fix: signature-only fixed-point readback must not force full resident column download."
                );
                assert_eq!(
                    result
                        .final_signature_snapshot
                        .as_ref()
                        .expect("Fix: signature-only fixed-point readback must return signatures."),
                    &device_signature_snapshot
                );
            }
            CudaEGraphFixedPointReadback::None => {
                assert_eq!(result.final_additional_readback_bytes, 0);
                assert_eq!(result.avoided_final_readback_bytes, expected_full_bytes);
                assert!(
                    result.final_snapshot.is_none(),
                    "Fix: no-readback fixed-point policy must not return full resident columns."
                );
                assert!(
                    result.final_signature_snapshot.is_none(),
                    "Fix: no-readback fixed-point policy must not return a signature snapshot."
                );
            }
        }

        backend
            .free_resident(resident.handle())
            .expect("Fix: CUDA e-graph resident image free failed.");
    }
}

#[test]
fn egraph_fixed_point_signature_readback_after_max_rounds_reads_only_signatures() {
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
    let upload_plan = plan_cuda_egraph_device_upload_from_image(image.clone())
        .expect("Fix: packed e-graph image must produce a CUDA upload plan.");
    let expected_full_bytes = expected_column_snapshot_bytes(upload_plan.byte_layout());
    let expected_signature_bytes = upload_plan.byte_layout().row_signatures().byte_len();
    let resident = backend
        .upload_egraph_device_image_plan(upload_plan)
        .expect("Fix: CUDA e-graph resident image upload failed.");

    let result = backend
        .run_egraph_structural_canonicalization_fixed_point_with_readback(
            resident,
            &image,
            CudaEGraphKernelLaunchConfig {
                threads_per_block: 2,
                max_blocks_per_launch: 2,
            },
            1,
            CudaEGraphFixedPointReadback::Signatures,
        )
        .expect("Fix: signature-only fixed-point readback after max rounds must use ranged CUDA readback.");
    let device_signature_snapshot = backend
        .download_egraph_resident_signature_snapshot(resident)
        .expect("Fix: CUDA e-graph signature-only snapshot must read refreshed signatures.");

    assert!(!result.converged);
    assert_eq!(result.rounds.len(), 1);
    assert_eq!(result.total_discovered_pairs, 1);
    assert_eq!(result.total_rewrites, 1);
    assert!(result.final_snapshot.is_none());
    assert_eq!(result.final_full_readback_bytes, expected_full_bytes);
    assert_eq!(
        result.final_signature_snapshot_bytes,
        expected_signature_bytes
    );
    assert_eq!(result.final_additional_readback_bytes, 0);
    assert_eq!(result.avoided_final_readback_bytes, expected_full_bytes);
    assert_eq!(
        result
            .final_signature_snapshot
            .as_ref()
            .expect("Fix: signature-only fixed-point result must include refreshed signatures."),
        &device_signature_snapshot
    );

    backend
        .free_resident(resident.handle())
        .expect("Fix: CUDA e-graph resident image free failed.");
}

