use crate::backend::staging_reserve::reserved_typed_vec;
use vyre_foundation::optimizer::eqsat_gpu::{gpu_egraph_row_signature, GpuEGraphDeviceImage};

use super::{CudaEGraphCanonicalRewrite, CudaEGraphKernelPlanError};

/// Host snapshot of the CUDA-resident e-graph columns needed to plan another
/// structural canonicalization round after device mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphResidentColumnSnapshot {
    /// One e-class id per row.
    pub row_eclass_ids: Vec<u32>,
    /// One language op id per row.
    pub row_language_op_ids: Vec<u32>,
    /// Child-column offset per row.
    pub row_children_offsets: Vec<u32>,
    /// Child count per row.
    pub row_children_lens: Vec<u32>,
    /// One structural signature per row.
    pub row_signatures: Vec<u32>,
    /// Flat child e-class column.
    pub children: Vec<u32>,
    /// Number of e-class groups in the resident image.
    pub eclass_group_count: usize,
}

impl CudaEGraphResidentColumnSnapshot {
    /// Build a resident-column snapshot from a foundation-packed image using
    /// fallible exact-reserve copies for every large column.
    pub fn try_from_device_image(
        image: &GpuEGraphDeviceImage,
    ) -> Result<Self, CudaEGraphKernelPlanError> {
        Ok(Self {
            row_eclass_ids: copy_u32_snapshot_column(
                image.row_eclass_ids(),
                "resident snapshot row eclass ids",
            )?,
            row_language_op_ids: copy_u32_snapshot_column(
                image.row_language_op_ids(),
                "resident snapshot row language op ids",
            )?,
            row_children_offsets: copy_u32_snapshot_column(
                image.row_children_offsets(),
                "resident snapshot row child offsets",
            )?,
            row_children_lens: copy_u32_snapshot_column(
                image.row_children_lens(),
                "resident snapshot row child lengths",
            )?,
            row_signatures: copy_u32_snapshot_column(
                image.row_signatures(),
                "resident snapshot row signatures",
            )?,
            children: copy_u32_snapshot_column(image.children(), "resident snapshot children")?,
            eclass_group_count: image.layout().eclass_group_count(),
        })
    }

    /// Build a resident-column snapshot from a foundation-packed image before
    /// any CUDA-side mutation has occurred.
    #[must_use]
    pub fn from_device_image(image: &GpuEGraphDeviceImage) -> Self {
        Self {
            row_eclass_ids: image.row_eclass_ids().to_vec(),
            row_language_op_ids: image.row_language_op_ids().to_vec(),
            row_children_offsets: image.row_children_offsets().to_vec(),
            row_children_lens: image.row_children_lens().to_vec(),
            row_signatures: image.row_signatures().to_vec(),
            children: image.children().to_vec(),
            eclass_group_count: image.layout().eclass_group_count(),
        }
    }

    /// Number of rows in the snapshot.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.row_signatures.len()
    }

    /// Number of child entries in the snapshot.
    #[must_use]
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Apply canonical e-class rewrites to row and child columns on the host.
    ///
    /// `rewrites` must be sorted by [`CudaEGraphCanonicalRewrite::eclass_id`],
    /// matching the device rewrite table consumed by the CUDA canonicalization
    /// kernel.
    pub fn apply_canonical_rewrites(&mut self, rewrites: &[CudaEGraphCanonicalRewrite]) {
        if rewrites.is_empty() {
            return;
        }
        for value in self
            .row_eclass_ids
            .iter_mut()
            .chain(self.children.iter_mut())
        {
            *value = canonicalize_eclass_id(*value, rewrites);
        }
    }

    /// Recompute every row signature from the current row metadata and child
    /// column, matching the CUDA row-signature refresh kernel.
    ///
    /// # Errors
    ///
    /// Returns [`CudaEGraphKernelPlanError`] when any row child span is
    /// malformed.
    pub fn refresh_row_signatures(&mut self) -> Result<(), CudaEGraphKernelPlanError> {
        for row in 0..self.row_count() {
            let start = usize::try_from(self.row_children_offsets[row]).map_err(|_| {
                CudaEGraphKernelPlanError::CountOverflow {
                    field: "resident snapshot row child offset",
                }
            })?;
            let len = usize::try_from(self.row_children_lens[row]).map_err(|_| {
                CudaEGraphKernelPlanError::CountOverflow {
                    field: "resident snapshot row child length",
                }
            })?;
            let end = start
                .checked_add(len)
                .ok_or(CudaEGraphKernelPlanError::CountOverflow {
                    field: "resident snapshot row child end",
                })?;
            if end > self.children.len() {
                return Err(CudaEGraphKernelPlanError::CountOverflow {
                    field: "resident snapshot row child span",
                });
            }
            self.row_signatures[row] = gpu_egraph_row_signature(
                self.row_language_op_ids[row],
                self.row_children_lens[row],
                &self.children[start..end],
            );
        }
        Ok(())
    }
}

fn canonicalize_eclass_id(value: u32, rewrites: &[CudaEGraphCanonicalRewrite]) -> u32 {
    rewrites
        .binary_search_by_key(&value, |rewrite| rewrite.eclass_id)
        .map_or(value, |index| rewrites[index].representative)
}

/// Lightweight host snapshot of the CUDA-resident row-signature column needed
/// to plan the next structural candidate buckets after device mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CudaEGraphResidentSignatureSnapshot {
    /// One structural signature per row.
    pub row_signatures: Vec<u32>,
    /// Number of child entries in the resident image.
    pub child_count: usize,
    /// Number of e-class groups in the resident image.
    pub eclass_group_count: usize,
}

impl CudaEGraphResidentSignatureSnapshot {
    /// Build a signature snapshot from a foundation-packed image using a
    /// fallible exact-reserve copy for the row-signature column.
    pub fn try_from_device_image(
        image: &GpuEGraphDeviceImage,
    ) -> Result<Self, CudaEGraphKernelPlanError> {
        Ok(Self {
            row_signatures: copy_u32_snapshot_column(
                image.row_signatures(),
                "resident signature snapshot row signatures",
            )?,
            child_count: image.layout().child_count(),
            eclass_group_count: image.layout().eclass_group_count(),
        })
    }

    /// Build a signature snapshot from a foundation-packed image before any
    /// CUDA-side mutation has occurred.
    #[must_use]
    pub fn from_device_image(image: &GpuEGraphDeviceImage) -> Self {
        Self {
            row_signatures: image.row_signatures().to_vec(),
            child_count: image.layout().child_count(),
            eclass_group_count: image.layout().eclass_group_count(),
        }
    }

    /// Build a signature snapshot from a full resident-column snapshot.
    #[must_use]
    pub fn from_column_snapshot(snapshot: &CudaEGraphResidentColumnSnapshot) -> Self {
        Self {
            row_signatures: snapshot.row_signatures.clone(),
            child_count: snapshot.child_count(),
            eclass_group_count: snapshot.eclass_group_count,
        }
    }

    /// Build a signature snapshot from a full resident-column snapshot using
    /// fallible exact-reserve storage for the copied signature column.
    pub fn try_from_column_snapshot(
        snapshot: &CudaEGraphResidentColumnSnapshot,
    ) -> Result<Self, CudaEGraphKernelPlanError> {
        Ok(Self {
            row_signatures: copy_u32_snapshot_column(
                &snapshot.row_signatures,
                "resident signature snapshot from full row signatures",
            )?,
            child_count: snapshot.child_count(),
            eclass_group_count: snapshot.eclass_group_count,
        })
    }

    /// Number of rows in the snapshot.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.row_signatures.len()
    }

    /// Number of child entries in the resident image.
    #[must_use]
    pub const fn child_count(&self) -> usize {
        self.child_count
    }
}

pub(super) fn copy_u32_snapshot_column(
    column: &[u32],
    field: &'static str,
) -> Result<Vec<u32>, CudaEGraphKernelPlanError> {
    let mut out = reserved_typed_vec(column.len(), field)?;
    out.extend_from_slice(column);
    Ok(out)
}
