use smallvec::SmallVec;

use crate::backend::{CudaBackend, CudaResidentBuffer};
use crate::egraph_device_image::CudaEGraphDeviceKernelView;
use crate::egraph_readback::upload_u32_words;
use crate::CudaResidentEGraphDeviceImage;
use vyre_driver::BackendError;
use vyre_driver::LaunchPlan;

use super::ptx::{
    cuda_egraph_canonical_rewrite_kernel_ptx, cuda_egraph_signature_refresh_kernel_ptx,
};
use super::{
    args::{EGraphCanonicalRewriteKernelArgs, EGraphSignatureRefreshKernelArgs},
    helpers::ceil_div_u64,
    helpers::usize_to_u64,
    CudaEGraphCanonicalRewriteDeviceImage, CudaEGraphCanonicalRewriteKernelPtx,
    CudaEGraphCanonicalRewriteKernelResult, CudaEGraphKernelLaunchConfig,
    CudaEGraphKernelPlanError, CudaEGraphSignatureRefreshKernelPtx,
    CudaEGraphSignatureRefreshKernelResult, CUDA_EGRAPH_CANONICAL_REWRITE_RECORD_WORDS,
};

impl CudaBackend {
    /// Generate and warm-load the canonical e-graph rewrite kernel through the
    /// CUDA module cache.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if PTX generation fails, the CUDA driver
    /// rejects the PTX module, or the `main` entry symbol cannot be resolved.
    pub fn warm_egraph_canonical_rewrite_kernel(
        &self,
    ) -> Result<CudaEGraphCanonicalRewriteKernelPtx, BackendError> {
        self.warm_egraph_canonical_rewrite_kernel_with_key()
            .map(|(kernel, _)| kernel)
    }

    fn warm_egraph_canonical_rewrite_kernel_with_key(
        &self,
    ) -> Result<
        (
            CudaEGraphCanonicalRewriteKernelPtx,
            cudarc::driver::sys::CUfunction,
        ),
        BackendError,
    > {
        let kernel =
            cuda_egraph_canonical_rewrite_kernel_ptx(self.ptx_target_sm()).map_err(|error| {
                BackendError::InvalidProgram {
                    fix: error.to_string(),
                }
            })?;
        let module_key = self.module_cache_key_for_raw_ptx_artifact(&kernel.source)?;
        let function = self.module_for_ptx_with_key(&kernel.source, module_key)?;
        if function.is_null() {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: CUDA e-graph canonical-rewrite kernel loaded but resolved a null `main` function. Inspect generated PTX entry metadata before launch.".to_string(),
            });
        }
        Ok((kernel, function))
    }

    /// Apply canonical e-class rewrites directly to a resident packed e-graph
    /// image on CUDA.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if rewrite metadata is malformed, launch
    /// dimensions are invalid, or CUDA allocation, upload, launch,
    /// synchronization, or cleanup fails.
    pub fn run_egraph_canonical_rewrite_kernel(
        &self,
        image: CudaResidentEGraphDeviceImage,
        rewrites: &CudaEGraphCanonicalRewriteDeviceImage,
        config: CudaEGraphKernelLaunchConfig,
    ) -> Result<CudaEGraphCanonicalRewriteKernelResult, BackendError> {
        if config.threads_per_block == 0 {
            return Err(BackendError::InvalidProgram {
                fix: CudaEGraphKernelPlanError::ZeroThreadsPerBlock.to_string(),
            });
        }
        if config.max_blocks_per_launch == 0 {
            return Err(BackendError::InvalidProgram {
                fix: CudaEGraphKernelPlanError::ZeroMaxBlocksPerLaunch.to_string(),
            });
        }
        if rewrites.rewrite_record_words != CUDA_EGRAPH_CANONICAL_REWRITE_RECORD_WORDS {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA e-graph canonical rewrite table uses {} words per record, expected {}.",
                    rewrites.rewrite_record_words,
                    CUDA_EGRAPH_CANONICAL_REWRITE_RECORD_WORDS
                ),
            });
        }
        let expected_words = rewrites
            .rewrite_count
            .checked_mul(CUDA_EGRAPH_CANONICAL_REWRITE_RECORD_WORDS)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: "Fix: CUDA e-graph canonical rewrite table word count overflowed host usize addressing.".to_string(),
            })?;
        if expected_words != rewrites.rewrite_words.len() {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA e-graph canonical rewrite table has {} words for {} records, expected {expected_words}.",
                    rewrites.rewrite_words.len(),
                    rewrites.rewrite_count
                ),
            });
        }

        let view = self.egraph_device_kernel_view(image)?;
        let row_items =
            usize_to_u64(view.row_count(), "canonical rewrite row count").map_err(|error| {
                BackendError::InvalidProgram {
                    fix: error.to_string(),
                }
            })?;
        let child_items = usize_to_u64(view.child_count(), "canonical rewrite child count")
            .map_err(|error| BackendError::InvalidProgram {
                fix: error.to_string(),
            })?;
        let total_items = row_items
            .checked_add(child_items)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: "Fix: CUDA e-graph canonical rewrite item count overflowed u64; shard the image before launch.".to_string(),
            })?;
        if total_items == 0 || rewrites.rewrite_count == 0 {
            return Ok(CudaEGraphCanonicalRewriteKernelResult {
                rewrite_count: rewrites.rewrite_count,
                row_count: view.row_count(),
                child_count: view.child_count(),
                launch_count: 0,
                total_items: 0,
            });
        }

        let (_kernel, func) = self.warm_egraph_canonical_rewrite_kernel_with_key()?;
        let rewrite_buffer = upload_u32_words(self, &rewrites.rewrite_words)?;
        let result = self.run_egraph_canonical_rewrite_kernel_inner(
            view,
            rewrites.rewrite_count,
            total_items,
            rewrite_buffer,
            func,
            config,
        );
        let cleanup = self.free_resident(rewrite_buffer);
        match (result, cleanup) {
            (Ok(result), Ok(())) => Ok(result),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) | (Err(_), Err(error)) => Err(error),
        }
    }

    /// Generate and warm-load the row-signature refresh kernel through the
    /// CUDA module cache.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if PTX generation fails, the CUDA driver
    /// rejects the PTX module, or the `main` entry symbol cannot be resolved.
    pub fn warm_egraph_signature_refresh_kernel(
        &self,
    ) -> Result<CudaEGraphSignatureRefreshKernelPtx, BackendError> {
        self.warm_egraph_signature_refresh_kernel_with_key()
            .map(|(kernel, _)| kernel)
    }

    fn warm_egraph_signature_refresh_kernel_with_key(
        &self,
    ) -> Result<
        (
            CudaEGraphSignatureRefreshKernelPtx,
            cudarc::driver::sys::CUfunction,
        ),
        BackendError,
    > {
        let kernel =
            cuda_egraph_signature_refresh_kernel_ptx(self.ptx_target_sm()).map_err(|error| {
                BackendError::InvalidProgram {
                    fix: error.to_string(),
                }
            })?;
        let module_key = self.module_cache_key_for_raw_ptx_artifact(&kernel.source)?;
        let function = self.module_for_ptx_with_key(&kernel.source, module_key)?;
        if function.is_null() {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: CUDA e-graph row-signature refresh kernel loaded but resolved a null `main` function. Inspect generated PTX entry metadata before launch.".to_string(),
            });
        }
        Ok((kernel, function))
    }

    /// Refresh resident e-graph row signatures on CUDA after canonical rewrites.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] if launch dimensions are invalid, resident
    /// pointer resolution fails, PTX loading fails, kernel launch fails, or
    /// synchronization fails.
    pub fn run_egraph_signature_refresh_kernel(
        &self,
        image: CudaResidentEGraphDeviceImage,
        config: CudaEGraphKernelLaunchConfig,
    ) -> Result<CudaEGraphSignatureRefreshKernelResult, BackendError> {
        if config.threads_per_block == 0 {
            return Err(BackendError::InvalidProgram {
                fix: CudaEGraphKernelPlanError::ZeroThreadsPerBlock.to_string(),
            });
        }
        if config.max_blocks_per_launch == 0 {
            return Err(BackendError::InvalidProgram {
                fix: CudaEGraphKernelPlanError::ZeroMaxBlocksPerLaunch.to_string(),
            });
        }
        let view = self.egraph_device_kernel_view(image)?;
        let row_count =
            usize_to_u64(view.row_count(), "signature refresh row count").map_err(|error| {
                BackendError::InvalidProgram {
                    fix: error.to_string(),
                }
            })?;
        if row_count == 0 {
            return Ok(CudaEGraphSignatureRefreshKernelResult {
                row_count: view.row_count(),
                launch_count: 0,
                total_rows: 0,
            });
        }
        let (_kernel, func) = self.warm_egraph_signature_refresh_kernel_with_key()?;
        self.run_egraph_signature_refresh_kernel_inner(view, row_count, func, config)
    }

    fn run_egraph_canonical_rewrite_kernel_inner(
        &self,
        view: CudaEGraphDeviceKernelView,
        rewrite_count: usize,
        total_items: u64,
        rewrite_buffer: CudaResidentBuffer,
        func: cudarc::driver::sys::CUfunction,
        config: CudaEGraphKernelLaunchConfig,
    ) -> Result<CudaEGraphCanonicalRewriteKernelResult, BackendError> {
        let rewrite_words_ptr = self.resident_device_ptr(rewrite_buffer)?;
        let stream = crate::stream::CudaStream::non_blocking()?;
        let row_count = u32::try_from(view.row_count()).map_err(|error| {
            BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA e-graph canonical rewrite row count {} does not fit u32 kernel ABI: {error}. Shard the image before launch.",
                    view.row_count()
                ),
            }
        })?;
        let child_count = u32::try_from(view.child_count()).map_err(|error| {
            BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA e-graph canonical rewrite child count {} does not fit u32 kernel ABI: {error}. Shard the image before launch.",
                    view.child_count()
                ),
            }
        })?;
        let rewrite_count_u32 = u32::try_from(rewrite_count).map_err(|error| {
            BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA e-graph canonical rewrite count {rewrite_count} does not fit u32 kernel ABI: {error}. Shard the rewrite table before launch."
                ),
            }
        })?;
        let items_per_wave = u64::from(config.threads_per_block)
            .checked_mul(u64::from(config.max_blocks_per_launch))
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: "Fix: CUDA e-graph canonical rewrite launch dimensions overflowed u64 item accounting.".to_string(),
            })?;
        let mut first_item = 0_u64;
        let mut launch_count = 0_usize;
        let mut kernel_args = SmallVec::<[*mut std::ffi::c_void; 8]>::new();
        while first_item < total_items {
            let wave_items = (total_items - first_item).min(items_per_wave);
            let blocks =
                ceil_div_u64(wave_items, u64::from(config.threads_per_block)).map_err(|error| {
                    BackendError::InvalidProgram {
                        fix: error.to_string(),
                    }
                })?;
            let blocks = u32::try_from(blocks).map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA e-graph canonical rewrite block count does not fit u32 launch ABI: {error}."
                ),
            })?;
            let launch = LaunchPlan {
                element_count: u32::try_from(wave_items).map_err(|error| {
                    BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA e-graph canonical rewrite wave item count {wave_items} does not fit u32 launch accounting: {error}. Split the wave before launch."
                        ),
                    }
                })?,
                workgroup: [config.threads_per_block, 1, 1],
                grid: [blocks, 1, 1],
                param_words: Vec::new(),
                max_binding_alignment: std::mem::size_of::<u64>(),
            };
            let mut args = EGraphCanonicalRewriteKernelArgs {
                row_eclass_ids_ptr: view.row_eclass_ids_ptr(),
                children_ptr: view.children_ptr(),
                rewrite_words_ptr,
                rewrite_count: rewrite_count_u32,
                row_count,
                child_count,
                first_item,
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
            first_item =
                first_item
                    .checked_add(wave_items)
                    .ok_or_else(|| BackendError::InvalidProgram {
                        fix: "Fix: CUDA e-graph canonical rewrite launch wave cursor overflowed u64 item accounting.".to_string(),
                    })?;
            launch_count =
                launch_count
                    .checked_add(1)
                    .ok_or_else(|| BackendError::InvalidProgram {
                        fix: "Fix: CUDA e-graph canonical rewrite launch count overflowed usize."
                            .to_string(),
                    })?;
        }
        stream.synchronize()?;
        Ok(CudaEGraphCanonicalRewriteKernelResult {
            rewrite_count,
            row_count: view.row_count(),
            child_count: view.child_count(),
            launch_count,
            total_items,
        })
    }

    fn run_egraph_signature_refresh_kernel_inner(
        &self,
        view: CudaEGraphDeviceKernelView,
        row_count: u64,
        func: cudarc::driver::sys::CUfunction,
        config: CudaEGraphKernelLaunchConfig,
    ) -> Result<CudaEGraphSignatureRefreshKernelResult, BackendError> {
        let stream = crate::stream::CudaStream::non_blocking()?;
        let row_count_u32 = u32::try_from(view.row_count()).map_err(|error| {
            BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA e-graph signature refresh row count {} does not fit u32 kernel ABI: {error}. Shard the image before launch.",
                    view.row_count()
                ),
            }
        })?;
        let items_per_wave = u64::from(config.threads_per_block)
            .checked_mul(u64::from(config.max_blocks_per_launch))
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: "Fix: CUDA e-graph signature refresh launch dimensions overflowed u64 item accounting.".to_string(),
            })?;
        let mut first_row = 0_u64;
        let mut launch_count = 0_usize;
        let mut kernel_args = SmallVec::<[*mut std::ffi::c_void; 8]>::new();
        while first_row < row_count {
            let wave_rows = (row_count - first_row).min(items_per_wave);
            let blocks =
                ceil_div_u64(wave_rows, u64::from(config.threads_per_block)).map_err(|error| {
                    BackendError::InvalidProgram {
                        fix: error.to_string(),
                    }
                })?;
            let blocks = u32::try_from(blocks).map_err(|error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA e-graph signature refresh block count does not fit u32 launch ABI: {error}."
                ),
            })?;
            let launch = LaunchPlan {
                element_count: u32::try_from(wave_rows).map_err(|error| {
                    BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: CUDA e-graph signature refresh wave row count {wave_rows} does not fit u32 launch accounting: {error}. Split the wave before launch."
                        ),
                    }
                })?,
                workgroup: [config.threads_per_block, 1, 1],
                grid: [blocks, 1, 1],
                param_words: Vec::new(),
                max_binding_alignment: std::mem::size_of::<u64>(),
            };
            let mut args = EGraphSignatureRefreshKernelArgs {
                row_language_op_ids_ptr: view.row_language_op_ids_ptr(),
                row_children_offsets_ptr: view.row_children_offsets_ptr(),
                row_children_lens_ptr: view.row_children_lens_ptr(),
                row_signatures_ptr: view.row_signatures_ptr(),
                children_ptr: view.children_ptr(),
                row_count: row_count_u32,
                first_row,
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
            first_row = first_row.checked_add(wave_rows).ok_or_else(|| {
                BackendError::InvalidProgram {
                    fix: "Fix: CUDA e-graph signature refresh launch wave cursor overflowed u64 row accounting.".to_string(),
                }
            })?;
            launch_count =
                launch_count
                    .checked_add(1)
                    .ok_or_else(|| BackendError::InvalidProgram {
                        fix: "Fix: CUDA e-graph signature refresh launch count overflowed usize."
                            .to_string(),
                    })?;
        }
        stream.synchronize()?;
        Ok(CudaEGraphSignatureRefreshKernelResult {
            row_count: view.row_count(),
            launch_count,
            total_rows: row_count,
        })
    }
}
