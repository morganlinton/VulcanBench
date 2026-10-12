//! CUDA e-graph kernel argument table builders.

use crate::backend::staging_reserve::reserve_smallvec;
use smallvec::SmallVec;
use vyre_driver::BackendError;

use super::{
    CUDA_EGRAPH_CANONICAL_REWRITE_KERNEL_PARAM_COUNT,
    CUDA_EGRAPH_SIGNATURE_REFRESH_KERNEL_PARAM_COUNT,
    CUDA_EGRAPH_STRUCTURAL_EQUIVALENCE_KERNEL_PARAM_COUNT,
};

pub(super) struct EGraphStructuralKernelArgs {
    pub(super) row_eclass_ids_ptr: u64,
    pub(super) row_language_op_ids_ptr: u64,
    pub(super) row_children_offsets_ptr: u64,
    pub(super) row_children_lens_ptr: u64,
    pub(super) row_signatures_ptr: u64,
    pub(super) children_ptr: u64,
    pub(super) bucket_words_ptr: u64,
    pub(super) bucket_rows_ptr: u64,
    pub(super) output_pairs_ptr: u64,
    pub(super) output_count_ptr: u64,
    pub(super) bucket_index: u32,
    pub(super) first_pair: u64,
    pub(super) pair_count: u64,
}

impl EGraphStructuralKernelArgs {
    pub(super) fn write_kernel_args_into(
        &mut self,
        args: &mut SmallVec<[*mut std::ffi::c_void; 8]>,
    ) -> Result<(), BackendError> {
        reserve_egraph_kernel_args(
            args,
            CUDA_EGRAPH_STRUCTURAL_EQUIVALENCE_KERNEL_PARAM_COUNT,
            "structural-equivalence",
        )?;
        args.push(&mut self.row_eclass_ids_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.row_language_op_ids_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.row_children_offsets_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.row_children_lens_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.row_signatures_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.children_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.bucket_words_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.bucket_rows_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.output_pairs_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.output_count_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.bucket_index as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.first_pair as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.pair_count as *mut _ as *mut std::ffi::c_void);
        Ok(())
    }
}

pub(super) struct EGraphCanonicalRewriteKernelArgs {
    pub(super) row_eclass_ids_ptr: u64,
    pub(super) children_ptr: u64,
    pub(super) rewrite_words_ptr: u64,
    pub(super) rewrite_count: u32,
    pub(super) row_count: u32,
    pub(super) child_count: u32,
    pub(super) first_item: u64,
}

impl EGraphCanonicalRewriteKernelArgs {
    pub(super) fn write_kernel_args_into(
        &mut self,
        args: &mut SmallVec<[*mut std::ffi::c_void; 8]>,
    ) -> Result<(), BackendError> {
        reserve_egraph_kernel_args(
            args,
            CUDA_EGRAPH_CANONICAL_REWRITE_KERNEL_PARAM_COUNT,
            "canonical-rewrite",
        )?;
        args.push(&mut self.row_eclass_ids_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.children_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.rewrite_words_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.rewrite_count as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.row_count as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.child_count as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.first_item as *mut _ as *mut std::ffi::c_void);
        Ok(())
    }
}

pub(super) struct EGraphSignatureRefreshKernelArgs {
    pub(super) row_language_op_ids_ptr: u64,
    pub(super) row_children_offsets_ptr: u64,
    pub(super) row_children_lens_ptr: u64,
    pub(super) row_signatures_ptr: u64,
    pub(super) children_ptr: u64,
    pub(super) row_count: u32,
    pub(super) first_row: u64,
}

impl EGraphSignatureRefreshKernelArgs {
    pub(super) fn write_kernel_args_into(
        &mut self,
        args: &mut SmallVec<[*mut std::ffi::c_void; 8]>,
    ) -> Result<(), BackendError> {
        reserve_egraph_kernel_args(
            args,
            CUDA_EGRAPH_SIGNATURE_REFRESH_KERNEL_PARAM_COUNT,
            "signature-refresh",
        )?;
        args.push(&mut self.row_language_op_ids_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.row_children_offsets_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.row_children_lens_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.row_signatures_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.children_ptr as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.row_count as *mut _ as *mut std::ffi::c_void);
        args.push(&mut self.first_row as *mut _ as *mut std::ffi::c_void);
        Ok(())
    }
}

fn reserve_egraph_kernel_args(
    args: &mut SmallVec<[*mut std::ffi::c_void; 8]>,
    arg_count: usize,
    context: &'static str,
) -> Result<(), BackendError> {
    args.clear();
    reserve_smallvec(args, arg_count, context)
}
