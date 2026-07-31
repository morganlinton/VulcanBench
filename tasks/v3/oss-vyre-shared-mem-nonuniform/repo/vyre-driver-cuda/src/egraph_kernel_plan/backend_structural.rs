use smallvec::SmallVec;

use crate::backend::{CudaBackend, CudaResidentBuffer};
use crate::egraph_device_image::{
    plan_cuda_egraph_device_upload_from_image_ref, CudaEGraphDeviceKernelView,
};
use crate::egraph_readback::{
    cleanup_egraph_kernel_handles, decode_unique_equivalence_pairs, device_ptr_at,
    download_structural_equivalence_output_ranges, read_u64_le,
    upload_structural_equivalence_scratch,
};
use crate::CudaResidentEGraphDeviceImage;
use vyre_driver::BackendError;
use vyre_driver::LaunchPlan;
use vyre_foundation::optimizer::eqsat_gpu::GpuEGraphDeviceImage;

use super::ptx::cuda_egraph_structural_equivalence_kernel_ptx;
use super::{
    args::EGraphStructuralKernelArgs, plan_cuda_egraph_signature_buckets,
    plan_cuda_egraph_structural_equivalence_launch_artifact_from_plan,
    CudaEGraphKernelLaunchConfig, CudaEGraphStructuralEquivalenceKernelPtx,
    CudaEGraphStructuralEquivalenceKernelResult, CudaEGraphStructuralEquivalenceLaunchArtifact,
};

impl CudaBackend {
    /// Generate and warm-load the structural e-graph equivalence kernel through
    /// the CUDA module cache.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if PTX generation fails, the CUDA driver
    /// rejects the PTX module, or the `main` entry symbol cannot be resolved.
    pub fn warm_egraph_structural_equivalence_kernel(
        &self,
    ) -> Result<CudaEGraphStructuralEquivalenceKernelPtx, BackendError> {
        self.warm_egraph_structural_equivalence_kernel_with_key()
            .map(|(kernel, _)| kernel)
    }

    fn warm_egraph_structural_equivalence_kernel_with_key(
        &self,
    ) -> Result<
        (
            CudaEGraphStructuralEquivalenceKernelPtx,
            cudarc::driver::sys::CUfunction,
        ),
        BackendError,
    > {
        let kernel = cuda_egraph_structural_equivalence_kernel_ptx(self.ptx_target_sm()).map_err(
            |error| BackendError::InvalidProgram {
                fix: error.to_string(),
            },
        )?;
        let module_key = self.module_cache_key_for_raw_ptx_artifact(&kernel.source)?;
        let function = self.module_for_ptx_with_key(&kernel.source, module_key)?;
        if function.is_null() {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: CUDA e-graph structural-equivalence kernel loaded but resolved a null `main` function. Inspect generated PTX entry metadata before launch.".to_string(),
            });
        }
        Ok((kernel, function))
    }

    /// Launch the structural e-graph equivalence kernel over a resident packed
    /// e-graph image.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if any scratch allocation, upload, kernel
    /// launch, readback, or cleanup step fails.
    pub fn run_egraph_structural_equivalence_kernel(
        &self,
        image: CudaResidentEGraphDeviceImage,
        artifact: &CudaEGraphStructuralEquivalenceLaunchArtifact,
    ) -> Result<CudaEGraphStructuralEquivalenceKernelResult, BackendError> {
        let view = self.egraph_device_kernel_view(image)?;
        let (_kernel, func) = self.warm_egraph_structural_equivalence_kernel_with_key()?;
        let mut handles = SmallVec::<[CudaResidentBuffer; 4]>::new();
        let result =
            self.run_egraph_structural_equivalence_kernel_inner(view, artifact, func, &mut handles);
        let cleanup = cleanup_egraph_kernel_handles(self, &handles);
        match (result, cleanup) {
            (Ok(result), Ok(())) => Ok(result),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) | (Err(_), Err(error)) => Err(error),
        }
    }

    /// Upload a packed foundation e-graph image, discover exact structural
    /// equivalences on CUDA, and free the temporary resident image.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if upload planning, resident upload, bucket
    /// planning, kernel execution, readback, or resident cleanup fails.
    pub fn discover_egraph_structural_equivalences(
        &self,
        image: GpuEGraphDeviceImage,
        config: CudaEGraphKernelLaunchConfig,
    ) -> Result<CudaEGraphStructuralEquivalenceKernelResult, BackendError> {
        let upload_plan =
            plan_cuda_egraph_device_upload_from_image_ref(&image).map_err(|error| {
                BackendError::InvalidProgram {
                    fix: error.to_string(),
                }
            })?;
        let resident = self.upload_egraph_device_image_borrowed_plan(upload_plan)?;
        let result = (|| {
            let view = self.egraph_device_kernel_view(resident)?;
            let signature_plan =
                plan_cuda_egraph_signature_buckets(&image, view, config).map_err(|error| {
                    BackendError::InvalidProgram {
                        fix: error.to_string(),
                    }
                })?;
            let artifact =
                plan_cuda_egraph_structural_equivalence_launch_artifact_from_plan(signature_plan)
                    .map_err(|error| BackendError::InvalidProgram {
                    fix: error.to_string(),
                })?;
            self.run_egraph_structural_equivalence_kernel(resident, &artifact)
        })();
        let cleanup = self.free_resident(resident.handle());
        match (result, cleanup) {
            (Ok(result), Ok(())) => Ok(result),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) | (Err(_), Err(error)) => Err(error),
        }
    }

    fn run_egraph_structural_equivalence_kernel_inner(
        &self,
        view: CudaEGraphDeviceKernelView,
        artifact: &CudaEGraphStructuralEquivalenceLaunchArtifact,
        func: cudarc::driver::sys::CUfunction,
        handles: &mut SmallVec<[CudaResidentBuffer; 4]>,
    ) -> Result<CudaEGraphStructuralEquivalenceKernelResult, BackendError> {
        let scratch = upload_structural_equivalence_scratch(self, artifact)?;
        handles.push(scratch.handle);
        let scratch_base_ptr = self.resident_device_ptr(scratch.handle)?;
        let bucket_words_ptr = device_ptr_at(
            scratch_base_ptr,
            scratch.bucket_words_offset,
            "bucket words",
        )?;
        let bucket_rows_ptr =
            device_ptr_at(scratch_base_ptr, scratch.bucket_rows_offset, "bucket rows")?;
        let output_pairs_ptr = device_ptr_at(
            scratch_base_ptr,
            scratch.output_pairs_offset,
            "output pairs",
        )?;
        let output_count_ptr = device_ptr_at(
            scratch_base_ptr,
            scratch.output_count_offset,
            "output count",
        )?;
        let stream = crate::stream::CudaStream::non_blocking()?;
        let mut kernel_args = SmallVec::<[*mut std::ffi::c_void; 8]>::new();
        for wave in &artifact.pair_waves {
            let launch = LaunchPlan {
                element_count: u32::try_from(wave.pair_count).map_err(|error| {
                    BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA e-graph structural-equivalence wave pair count {} does not fit u32 launch accounting: {error}. Split the wave before launch.",
                            wave.pair_count
                        ),
                    }
                })?,
                workgroup: [wave.threads_per_block, 1, 1],
                grid: [wave.blocks, 1, 1],
                param_words: Vec::new(),
                max_binding_alignment: std::mem::size_of::<u64>(),
            };
            let mut args = EGraphStructuralKernelArgs {
                row_eclass_ids_ptr: view.row_eclass_ids_ptr(),
                row_language_op_ids_ptr: view.row_language_op_ids_ptr(),
                row_children_offsets_ptr: view.row_children_offsets_ptr(),
                row_children_lens_ptr: view.row_children_lens_ptr(),
                row_signatures_ptr: view.row_signatures_ptr(),
                children_ptr: view.children_ptr(),
                bucket_words_ptr,
                bucket_rows_ptr,
                output_pairs_ptr,
                output_count_ptr,
                bucket_index: wave.bucket_index,
                first_pair: wave.first_pair,
                pair_count: wave.pair_count,
            };
            args.write_kernel_args_into(&mut kernel_args)?;
            self.launch_resolved_function(
                func,
                &mut kernel_args,
                &launch,
                stream.raw(),
                false,
                false,
            )?;
        }
        stream.synchronize()?;

        let (count_bytes, pair_bytes) =
            download_structural_equivalence_output_ranges(self, &scratch)?;
        let count_bytes = count_bytes
            .get(..8)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: "Fix: CUDA e-graph fused scratch readback did not contain the 8-byte structural equivalence counter.".to_string(),
            })?;
        let device_reported_count = read_u64_le(count_bytes, "structural equivalence count")?;
        let planned_capacity = artifact.output.max_equivalences;
        // Fail closed: do NOT silently truncate the equivalence set.  A capped
        // result fed into `plan_cuda_egraph_union_compaction` would produce an
        // incomplete union plan and corrupt the e-graph canonicalization with no
        // visible error.  The caller must re-plan with a larger `max_equivalences`
        // (or the planner must raise the cap automatically) and retry.
        if device_reported_count > planned_capacity {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA e-graph structural-equivalence kernel reported {device_reported_count} \
                     equivalence pairs but the planned output buffer holds at most {planned_capacity}. \
                     Re-plan with a larger `max_equivalences` before re-launching; the current result \
                     is incomplete and must not be used for union compaction."
                ),
            });
        }
        let pair_bytes = pair_bytes
            .get(..scratch.output_pairs_bytes)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: "Fix: CUDA e-graph fused scratch readback did not contain the planned structural equivalence output-pair range.".to_string(),
            })?;
        let (emitted_pair_count, unique) =
            decode_unique_equivalence_pairs(&pair_bytes, device_reported_count)?;
        Ok(CudaEGraphStructuralEquivalenceKernelResult {
            emitted_pair_count,
            unique,
            device_reported_count,
            // Always false after the fail-closed guard above: kept in the struct
            // so that callers which snapshot telemetry can record the value, and
            // so that the existing integration-test assertions remain valid.
            overflowed_output_capacity: false,
        })
    }
}
