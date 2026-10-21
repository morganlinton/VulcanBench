use crate::backend::staging_reserve::reserved_typed_vec;
use crate::backend::CudaBackend;
use crate::egraph_device_image::CudaEGraphDeviceKernelView;
use crate::egraph_readback::{
    egraph_column_snapshot_readback_bytes, egraph_column_snapshot_spans, read_resident_u32_range,
};
use crate::CudaResidentEGraphDeviceImage;
use vyre_driver::BackendError;
use vyre_foundation::optimizer::eqsat_gpu::GpuEGraphDeviceImage;

use super::{
    helpers::usize_to_u64, pack_cuda_egraph_canonical_rewrite_device_image,
    plan_cuda_egraph_signature_buckets, plan_cuda_egraph_signature_buckets_from_resident_snapshot,
    plan_cuda_egraph_signature_buckets_from_signature_snapshot,
    plan_cuda_egraph_structural_equivalence_launch_artifact_from_plan,
    plan_cuda_egraph_union_compaction, CudaEGraphCanonicalRewriteKernelResult,
    CudaEGraphFixedPointReadback, CudaEGraphKernelLaunchConfig, CudaEGraphKernelPlanError,
    CudaEGraphResidentColumnSnapshot, CudaEGraphResidentSignatureSnapshot,
    CudaEGraphSignatureBucketPlan, CudaEGraphSignatureRefreshKernelResult,
    CudaEGraphStructuralCanonicalizationFixedPointReport,
    CudaEGraphStructuralCanonicalizationFixedPointResult,
    CudaEGraphStructuralCanonicalizationRoundResult,
};

impl CudaBackend {
    /// Download the current CUDA-resident e-graph planning columns.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if resident download fails or any packed u32
    /// span is malformed.
    pub fn download_egraph_resident_column_snapshot(
        &self,
        image: CudaResidentEGraphDeviceImage,
    ) -> Result<CudaEGraphResidentColumnSnapshot, BackendError> {
        let layout = image.byte_layout();
        let spans = egraph_column_snapshot_spans(layout);
        let ranges = spans.map(|span| (image.handle(), span.offset(), span.byte_len()));
        let mut row_eclass_bytes = Vec::new();
        let mut row_language_op_bytes = Vec::new();
        let mut row_children_offset_bytes = Vec::new();
        let mut row_children_len_bytes = Vec::new();
        let mut row_signature_bytes = Vec::new();
        let mut child_bytes = Vec::new();
        let mut outputs: [&mut Vec<u8>; 6] = [
            &mut row_eclass_bytes,
            &mut row_language_op_bytes,
            &mut row_children_offset_bytes,
            &mut row_children_len_bytes,
            &mut row_signature_bytes,
            &mut child_bytes,
        ];
        self.download_resident_ranges_into(&ranges, &mut outputs)?;
        Ok(CudaEGraphResidentColumnSnapshot {
            row_eclass_ids: read_resident_u32_range(
                &row_eclass_bytes,
                layout.row_count(),
                "row eclass ids",
            )?,
            row_language_op_ids: read_resident_u32_range(
                &row_language_op_bytes,
                layout.row_count(),
                "row language op ids",
            )?,
            row_children_offsets: read_resident_u32_range(
                &row_children_offset_bytes,
                layout.row_count(),
                "row child offsets",
            )?,
            row_children_lens: read_resident_u32_range(
                &row_children_len_bytes,
                layout.row_count(),
                "row child lengths",
            )?,
            row_signatures: read_resident_u32_range(
                &row_signature_bytes,
                layout.row_count(),
                "row signatures",
            )?,
            children: read_resident_u32_range(&child_bytes, layout.child_count(), "children")?,
            eclass_group_count: layout.eclass_group_count(),
        })
    }

    /// Download only the current CUDA-resident row-signature column needed for
    /// planning the next fixed-point round.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if resident download fails or the signature
    /// span is malformed.
    pub fn download_egraph_resident_signature_snapshot(
        &self,
        image: CudaResidentEGraphDeviceImage,
    ) -> Result<CudaEGraphResidentSignatureSnapshot, BackendError> {
        let layout = image.byte_layout();
        let signature_span = layout.row_signatures();
        let bytes = self.download_resident_range(
            image.handle(),
            signature_span.offset(),
            signature_span.byte_len(),
        )?;
        Ok(CudaEGraphResidentSignatureSnapshot {
            row_signatures: read_resident_u32_range(&bytes, layout.row_count(), "row signatures")?,
            child_count: layout.child_count(),
            eclass_group_count: layout.eclass_group_count(),
        })
    }

    /// Discover structural e-class equivalences, derive deterministic
    /// canonical representatives, and mutate the resident e-graph image on
    /// CUDA in one round.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if resident view construction, signature
    /// planning, structural discovery, union planning, rewrite packing, kernel
    /// launch, synchronization, or cleanup fails.
    pub fn run_egraph_structural_canonicalization_round(
        &self,
        resident: CudaResidentEGraphDeviceImage,
        image: &GpuEGraphDeviceImage,
        config: CudaEGraphKernelLaunchConfig,
    ) -> Result<CudaEGraphStructuralCanonicalizationRoundResult, BackendError> {
        let view = self.egraph_device_kernel_view(resident)?;
        let signature_plan =
            plan_cuda_egraph_signature_buckets(image, view, config).map_err(|error| {
                BackendError::InvalidProgram {
                    fix: error.to_string(),
                }
            })?;
        self.run_egraph_structural_canonicalization_round_from_signature_plan(
            resident,
            view,
            signature_plan,
            config,
        )
    }

    /// Run one CUDA-resident structural canonicalization round using a current
    /// resident-column planning snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if resident view construction, signature
    /// planning, structural discovery, union planning, rewrite packing, kernel
    /// launch, synchronization, or cleanup fails.
    pub fn run_egraph_structural_canonicalization_round_from_snapshot(
        &self,
        resident: CudaResidentEGraphDeviceImage,
        snapshot: &CudaEGraphResidentColumnSnapshot,
        config: CudaEGraphKernelLaunchConfig,
    ) -> Result<CudaEGraphStructuralCanonicalizationRoundResult, BackendError> {
        let view = self.egraph_device_kernel_view(resident)?;
        let signature_plan =
            plan_cuda_egraph_signature_buckets_from_resident_snapshot(snapshot, view, config)
                .map_err(|error| BackendError::InvalidProgram {
                    fix: error.to_string(),
                })?;
        self.run_egraph_structural_canonicalization_round_from_signature_plan(
            resident,
            view,
            signature_plan,
            config,
        )
    }

    /// Run one CUDA-resident structural canonicalization round using a current
    /// resident signature-column planning snapshot.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if resident view construction, signature
    /// planning, structural discovery, union planning, rewrite packing, kernel
    /// launch, synchronization, or cleanup fails.
    pub fn run_egraph_structural_canonicalization_round_from_signature_snapshot(
        &self,
        resident: CudaResidentEGraphDeviceImage,
        snapshot: &CudaEGraphResidentSignatureSnapshot,
        config: CudaEGraphKernelLaunchConfig,
    ) -> Result<CudaEGraphStructuralCanonicalizationRoundResult, BackendError> {
        let view = self.egraph_device_kernel_view(resident)?;
        let signature_plan =
            plan_cuda_egraph_signature_buckets_from_signature_snapshot(snapshot, view, config)
                .map_err(|error| BackendError::InvalidProgram {
                    fix: error.to_string(),
                })?;
        self.run_egraph_structural_canonicalization_round_from_signature_plan(
            resident,
            view,
            signature_plan,
            config,
        )
    }

    fn run_egraph_structural_canonicalization_round_from_signature_plan(
        &self,
        resident: CudaResidentEGraphDeviceImage,
        view: CudaEGraphDeviceKernelView,
        signature_plan: CudaEGraphSignatureBucketPlan,
        config: CudaEGraphKernelLaunchConfig,
    ) -> Result<CudaEGraphStructuralCanonicalizationRoundResult, BackendError> {
        let artifact =
            plan_cuda_egraph_structural_equivalence_launch_artifact_from_plan(signature_plan)
                .map_err(|error| BackendError::InvalidProgram {
                    fix: error.to_string(),
                })?;
        let discovery = self.run_egraph_structural_equivalence_kernel(resident, &artifact)?;
        let union_plan =
            plan_cuda_egraph_union_compaction(&discovery.unique, config).map_err(|error| {
                BackendError::InvalidProgram {
                    fix: error.to_string(),
                }
            })?;
        if union_plan.canonical_rewrites.is_empty() {
            return Ok(CudaEGraphStructuralCanonicalizationRoundResult {
                discovery,
                union_plan,
                rewrite: CudaEGraphCanonicalRewriteKernelResult {
                    rewrite_count: 0,
                    row_count: view.row_count(),
                    child_count: view.child_count(),
                    launch_count: 0,
                    total_items: 0,
                },
                signature_refresh: CudaEGraphSignatureRefreshKernelResult {
                    row_count: view.row_count(),
                    launch_count: 0,
                    total_rows: 0,
                },
            });
        }
        let rewrite_image =
            pack_cuda_egraph_canonical_rewrite_device_image(&union_plan).map_err(|error| {
                BackendError::InvalidProgram {
                    fix: error.to_string(),
                }
            })?;
        let rewrite = self.run_egraph_canonical_rewrite_kernel(resident, &rewrite_image, config)?;
        let signature_refresh = if rewrite.rewrite_count == 0 {
            CudaEGraphSignatureRefreshKernelResult {
                row_count: view.row_count(),
                launch_count: 0,
                total_rows: 0,
            }
        } else {
            self.run_egraph_signature_refresh_kernel(resident, config)?
        };
        Ok(CudaEGraphStructuralCanonicalizationRoundResult {
            discovery,
            union_plan,
            rewrite,
            signature_refresh,
        })
    }

    /// Iterate CUDA-resident structural canonicalization until a no-op
    /// discovery round proves fixed-point convergence or `max_rounds` is
    /// reached.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if any round fails or resident snapshot
    /// readback fails between rounds.
    pub fn run_egraph_structural_canonicalization_fixed_point(
        &self,
        resident: CudaResidentEGraphDeviceImage,
        initial_image: &GpuEGraphDeviceImage,
        config: CudaEGraphKernelLaunchConfig,
        max_rounds: usize,
    ) -> Result<CudaEGraphStructuralCanonicalizationFixedPointResult, BackendError> {
        let report = self.run_egraph_structural_canonicalization_fixed_point_with_readback(
            resident,
            initial_image,
            config,
            max_rounds,
            CudaEGraphFixedPointReadback::FullColumns,
        )?;
        let final_snapshot =
            report
                .final_snapshot
                .ok_or_else(|| BackendError::InvalidProgram {
                    fix: "Fix: CUDA e-graph fixed-point full-column readback was requested but no final snapshot was produced."
                        .to_string(),
                })?;
        Ok(CudaEGraphStructuralCanonicalizationFixedPointResult {
            rounds: report.rounds,
            final_snapshot,
            converged: report.converged,
            max_rounds: report.max_rounds,
            total_discovered_pairs: report.total_discovered_pairs,
            total_rewrites: report.total_rewrites,
        })
    }

    /// Iterate CUDA-resident structural canonicalization with explicit control
    /// over the final host readback volume.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if any round fails or the requested resident
    /// snapshot readback fails.
    pub fn run_egraph_structural_canonicalization_fixed_point_with_readback(
        &self,
        resident: CudaResidentEGraphDeviceImage,
        initial_image: &GpuEGraphDeviceImage,
        config: CudaEGraphKernelLaunchConfig,
        max_rounds: usize,
        final_readback: CudaEGraphFixedPointReadback,
    ) -> Result<CudaEGraphStructuralCanonicalizationFixedPointReport, BackendError> {
        let mut rounds = reserved_typed_vec(max_rounds, "egraph fixed point rounds").map_err(
            |error: CudaEGraphKernelPlanError| BackendError::InvalidProgram {
                fix: error.to_string(),
            },
        )?;
        let mut column_snapshot = CudaEGraphResidentColumnSnapshot::try_from_device_image(
            initial_image,
        )
        .map_err(|error| BackendError::InvalidProgram {
            fix: error.to_string(),
        })?;
        let mut signature_snapshot = Some(
            CudaEGraphResidentSignatureSnapshot::from_column_snapshot(&column_snapshot),
        );
        let mut signature_snapshot_current = true;
        let mut total_discovered_pairs = 0_u64;
        let mut total_rewrites = 0_u64;
        let mut converged = false;
        let layout = resident.byte_layout();
        let final_full_readback_bytes = egraph_column_snapshot_readback_bytes(layout)?;
        let final_signature_snapshot_bytes = layout.row_signatures().byte_len();

        for round_index in 0..max_rounds {
            let round = if round_index == 0 {
                self.run_egraph_structural_canonicalization_round_from_snapshot(
                    resident,
                    &column_snapshot,
                    config,
                )?
            } else {
                let snapshot = signature_snapshot.as_ref().ok_or_else(|| {
                    BackendError::InvalidProgram {
                        fix: "Fix: CUDA e-graph fixed-point planner lost the current signature snapshot before a follow-up round.".to_string(),
                    }
                })?;
                self.run_egraph_structural_canonicalization_round_from_signature_snapshot(
                    resident, snapshot, config,
                )?
            };
            let discovered_pairs = usize_to_u64(
                round.discovery.unique.len(),
                "fixed point discovered pair count",
            )
            .map_err(|error| BackendError::InvalidProgram {
                fix: error.to_string(),
            })?;
            let rewrite_count =
                usize_to_u64(round.rewrite.rewrite_count, "fixed point rewrite count").map_err(
                    |error| BackendError::InvalidProgram {
                        fix: error.to_string(),
                    },
                )?;
            total_discovered_pairs = total_discovered_pairs
                .checked_add(discovered_pairs)
                .ok_or_else(|| BackendError::InvalidProgram {
                    fix: "Fix: CUDA e-graph fixed-point discovered pair count overflowed u64."
                        .to_string(),
                })?;
            total_rewrites = total_rewrites.checked_add(rewrite_count).ok_or_else(|| {
                BackendError::InvalidProgram {
                    fix: "Fix: CUDA e-graph fixed-point rewrite count overflowed u64.".to_string(),
                }
            })?;
            if discovered_pairs == 0 || rewrite_count == 0 {
                rounds.push(round);
                converged = true;
                break;
            }
            column_snapshot.apply_canonical_rewrites(&round.union_plan.canonical_rewrites);
            if round.signature_refresh.launch_count > 0 {
                column_snapshot.refresh_row_signatures().map_err(|error| {
                    BackendError::InvalidProgram {
                        fix: error.to_string(),
                    }
                })?;
            }
            signature_snapshot = Some(CudaEGraphResidentSignatureSnapshot::from_column_snapshot(
                &column_snapshot,
            ));
            signature_snapshot_current = true;
            rounds.push(round);
            if round_index + 1 == max_rounds {
                break;
            }
        }

        let final_snapshot = match final_readback {
            CudaEGraphFixedPointReadback::FullColumns => Some(column_snapshot),
            CudaEGraphFixedPointReadback::None | CudaEGraphFixedPointReadback::Signatures => None,
        };
        let final_additional_readback_bytes = match final_readback {
            CudaEGraphFixedPointReadback::FullColumns
            | CudaEGraphFixedPointReadback::Signatures
            | CudaEGraphFixedPointReadback::None => 0,
        };
        let final_signature_snapshot = match final_readback {
            CudaEGraphFixedPointReadback::None => None,
            CudaEGraphFixedPointReadback::Signatures => {
                if signature_snapshot_current {
                    signature_snapshot
                } else if total_rewrites == 0 {
                    Some(
                        CudaEGraphResidentSignatureSnapshot::try_from_device_image(initial_image)
                            .map_err(|error| BackendError::InvalidProgram {
                            fix: error.to_string(),
                        })?,
                    )
                } else {
                    return Err(BackendError::InvalidProgram {
                        fix: "Fix: CUDA e-graph fixed-point host planning mirror lost the current signature snapshot after device rewrites.".to_string(),
                    });
                }
            }
            CudaEGraphFixedPointReadback::FullColumns => final_snapshot
                .as_ref()
                .map(CudaEGraphResidentSignatureSnapshot::from_column_snapshot),
        };
        let avoided_final_readback_bytes = final_full_readback_bytes
            .checked_sub(final_additional_readback_bytes)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: "Fix: CUDA e-graph final readback accounting underflowed.".to_string(),
            })?;

        Ok(CudaEGraphStructuralCanonicalizationFixedPointReport {
            rounds,
            final_snapshot,
            final_signature_snapshot,
            final_readback,
            final_full_readback_bytes,
            final_signature_snapshot_bytes,
            final_additional_readback_bytes,
            avoided_final_readback_bytes,
            converged,
            max_rounds,
            total_discovered_pairs,
            total_rewrites,
        })
    }
}
