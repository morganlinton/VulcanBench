//! Sparse-kernel selector evidence for graph, flow, and math workloads.

use std::error::Error;
use std::fmt::{Display, Formatter};

/// Sparse selector evidence schema version.
pub const SPARSE_KERNEL_SELECTOR_SCHEMA_VERSION: u32 = 1;

/// Sparse workload class.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SparseKernelWorkloadClass {
    /// Sparse matrix-vector shape.
    SpmvLike,
    /// Sparse matrix-matrix shape.
    SpmmLike,
    /// Masked sparse update shape.
    MaskedUpdate,
    /// Frontier expansion shape.
    FrontierExpansion,
}

impl SparseKernelWorkloadClass {
    /// Stable evidence label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SpmvLike => "spmv-like",
            Self::SpmmLike => "spmm-like",
            Self::MaskedUpdate => "masked-update",
            Self::FrontierExpansion => "frontier-expansion",
        }
    }
}

/// Selected sparse execution path.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SparseKernelSelectedPath {
    /// cuSPARSE SpMV-style library baseline.
    CusparseSpmv,
    /// cuSPARSE SpMM-style library baseline.
    CusparseSpmm,
    /// cuSPARSE masked-update-style library baseline.
    CusparseMaskedUpdate,
    /// Native frontier expansion path.
    FrontierExpansion,
}

impl SparseKernelSelectedPath {
    /// Stable evidence label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CusparseSpmv => "cusparse-spmv",
            Self::CusparseSpmm => "cusparse-spmm",
            Self::CusparseMaskedUpdate => "cusparse-masked-update",
            Self::FrontierExpansion => "frontier-expansion",
        }
    }
}

/// Selector request for one sparse workload.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SparseKernelSelectorRequest {
    /// Workload class.
    pub workload_class: SparseKernelWorkloadClass,
    /// Matrix rows.
    pub rows: u32,
    /// Matrix columns.
    pub cols: u32,
    /// Non-zero entries.
    pub nnz: u32,
    /// Dense RHS columns, `1` for SpMV-like workloads.
    pub rhs_cols: u32,
    /// Mask non-zero entries for masked update workloads.
    pub mask_nnz: u32,
    /// Active frontier entries for frontier expansion workloads.
    pub frontier_nnz: u32,
    /// Comparator baseline id.
    pub baseline_id: String,
    /// Result digest supplied by the caller's benchmark/evidence producer.
    pub result_digest: [u8; 32],
    /// Host/device transfer bytes for this selector decision.
    pub transfer_bytes: u64,
}

/// Selector evidence emitted for one sparse workload.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SparseKernelSelectorEvidence {
    /// Evidence schema version.
    pub schema_version: u32,
    /// Workload class label.
    pub workload_class: &'static str,
    /// Selected execution path.
    pub selected_path: &'static str,
    /// Comparator baseline id.
    pub baseline_id: String,
    /// Matrix rows.
    pub rows: u32,
    /// Matrix columns.
    pub cols: u32,
    /// Non-zero entries.
    pub nnz: u32,
    /// Dense RHS columns.
    pub rhs_cols: u32,
    /// Mask non-zero entries.
    pub mask_nnz: u32,
    /// Active frontier entries.
    pub frontier_nnz: u32,
    /// Sparse density in basis points.
    pub density_bps: u32,
    /// Result digest supplied by the caller.
    pub result_digest: [u8; 32],
    /// Host/device transfer bytes.
    pub transfer_bytes: u64,
}

/// Sparse selector validation failure.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SparseKernelSelectorError {
    /// Matrix dimensions are zero.
    InvalidShape,
    /// Non-zero count is invalid for the matrix shape.
    InvalidNnz,
    /// RHS column count is invalid for the workload class.
    InvalidRhsColumns {
        /// Required diagnostic.
        reason: &'static str,
    },
    /// Mask count is required but missing.
    MissingMask,
    /// Frontier count is required but missing.
    MissingFrontier,
    /// Baseline id is blank.
    MissingBaselineId,
    /// Result digest is all zeros.
    MissingResultDigest,
    /// Transfer-byte accounting is missing.
    MissingTransferBytes,
}

impl Display for SparseKernelSelectorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidShape => write!(
                f,
                "sparse kernel selector received zero rows or columns. Fix: record a non-empty sparse workload shape."
            ),
            Self::InvalidNnz => write!(
                f,
                "sparse kernel selector received invalid nnz for the matrix shape. Fix: bound nnz to rows*cols and keep sparse workloads non-empty."
            ),
            Self::InvalidRhsColumns { reason } => {
                write!(f, "sparse kernel selector RHS columns are invalid: {reason}.")
            }
            Self::MissingMask => write!(
                f,
                "sparse kernel selector masked-update workload has no mask tuples. Fix: record mask_nnz before selecting a masked path."
            ),
            Self::MissingFrontier => write!(
                f,
                "sparse kernel selector frontier-expansion workload has no frontier tuples. Fix: record frontier_nnz before selecting a frontier path."
            ),
            Self::MissingBaselineId => write!(
                f,
                "sparse kernel selector baseline id is blank. Fix: record the cuSPARSE/frontier comparator id."
            ),
            Self::MissingResultDigest => write!(
                f,
                "sparse kernel selector result digest is missing. Fix: attach the benchmark output digest."
            ),
            Self::MissingTransferBytes => write!(
                f,
                "sparse kernel selector transfer bytes are zero. Fix: account host/device bytes separately from logical work."
            ),
        }
    }
}

impl Error for SparseKernelSelectorError {}

/// Select and validate one sparse kernel evidence row.
///
/// # Errors
///
/// Returns [`SparseKernelSelectorError`] when shape, baseline, digest,
/// transfer-byte accounting, or workload-specific fields are incomplete.
pub fn select_sparse_kernel(
    request: SparseKernelSelectorRequest,
) -> Result<SparseKernelSelectorEvidence, SparseKernelSelectorError> {
    validate_sparse_request(&request)?;
    let selected = match request.workload_class {
        SparseKernelWorkloadClass::SpmvLike => SparseKernelSelectedPath::CusparseSpmv,
        SparseKernelWorkloadClass::SpmmLike => SparseKernelSelectedPath::CusparseSpmm,
        SparseKernelWorkloadClass::MaskedUpdate => SparseKernelSelectedPath::CusparseMaskedUpdate,
        SparseKernelWorkloadClass::FrontierExpansion => SparseKernelSelectedPath::FrontierExpansion,
    };
    Ok(SparseKernelSelectorEvidence {
        schema_version: SPARSE_KERNEL_SELECTOR_SCHEMA_VERSION,
        workload_class: request.workload_class.as_str(),
        selected_path: selected.as_str(),
        baseline_id: request.baseline_id,
        rows: request.rows,
        cols: request.cols,
        nnz: request.nnz,
        rhs_cols: request.rhs_cols,
        mask_nnz: request.mask_nnz,
        frontier_nnz: request.frontier_nnz,
        density_bps: density_bps(request.rows, request.cols, request.nnz),
        result_digest: request.result_digest,
        transfer_bytes: request.transfer_bytes,
    })
}

fn validate_sparse_request(
    request: &SparseKernelSelectorRequest,
) -> Result<(), SparseKernelSelectorError> {
    if request.rows == 0 || request.cols == 0 {
        return Err(SparseKernelSelectorError::InvalidShape);
    }
    let Some(cells) = request.rows.checked_mul(request.cols) else {
        return Err(SparseKernelSelectorError::InvalidNnz);
    };
    if request.nnz == 0 || request.nnz > cells {
        return Err(SparseKernelSelectorError::InvalidNnz);
    }
    match request.workload_class {
        SparseKernelWorkloadClass::SpmvLike => {
            if request.rhs_cols != 1 {
                return Err(SparseKernelSelectorError::InvalidRhsColumns {
                    reason: "SpMV-like workloads require rhs_cols == 1",
                });
            }
        }
        SparseKernelWorkloadClass::SpmmLike => {
            if request.rhs_cols <= 1 {
                return Err(SparseKernelSelectorError::InvalidRhsColumns {
                    reason: "SpMM-like workloads require rhs_cols > 1",
                });
            }
        }
        SparseKernelWorkloadClass::MaskedUpdate => {
            if request.mask_nnz == 0 {
                return Err(SparseKernelSelectorError::MissingMask);
            }
        }
        SparseKernelWorkloadClass::FrontierExpansion => {
            if request.frontier_nnz == 0 {
                return Err(SparseKernelSelectorError::MissingFrontier);
            }
        }
    }
    if request.baseline_id.trim().is_empty() {
        return Err(SparseKernelSelectorError::MissingBaselineId);
    }
    if request.result_digest == [0; 32] {
        return Err(SparseKernelSelectorError::MissingResultDigest);
    }
    if request.transfer_bytes == 0 {
        return Err(SparseKernelSelectorError::MissingTransferBytes);
    }
    Ok(())
}

fn density_bps(rows: u32, cols: u32, nnz: u32) -> u32 {
    let cells = u64::from(rows).saturating_mul(u64::from(cols)).max(1);
    ((u64::from(nnz) * 10_000) / cells).min(10_000) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn request(workload_class: SparseKernelWorkloadClass) -> SparseKernelSelectorRequest {
        SparseKernelSelectorRequest {
            workload_class,
            rows: 128,
            cols: 256,
            nnz: 512,
            rhs_cols: 1,
            mask_nnz: 0,
            frontier_nnz: 0,
            baseline_id: "cusparse-12.5".to_string(),
            result_digest: digest(7),
            transfer_bytes: 4096,
        }
    }

    #[test]
    fn sparse_selector_classifies_spmv_spmm_masked_and_frontier_workloads() {
        let spmv = select_sparse_kernel(request(SparseKernelWorkloadClass::SpmvLike)).unwrap();
        assert_eq!(spmv.selected_path, "cusparse-spmv");
        assert_eq!(spmv.baseline_id, "cusparse-12.5");
        assert_eq!(spmv.result_digest, digest(7));
        assert_eq!(spmv.transfer_bytes, 4096);

        let mut spmm_request = request(SparseKernelWorkloadClass::SpmmLike);
        spmm_request.rhs_cols = 8;
        let spmm = select_sparse_kernel(spmm_request).unwrap();
        assert_eq!(spmm.selected_path, "cusparse-spmm");

        let mut masked_request = request(SparseKernelWorkloadClass::MaskedUpdate);
        masked_request.mask_nnz = 64;
        let masked = select_sparse_kernel(masked_request).unwrap();
        assert_eq!(masked.selected_path, "cusparse-masked-update");
        assert_eq!(masked.mask_nnz, 64);

        let mut frontier_request = request(SparseKernelWorkloadClass::FrontierExpansion);
        frontier_request.frontier_nnz = 32;
        let frontier = select_sparse_kernel(frontier_request).unwrap();
        assert_eq!(frontier.selected_path, "frontier-expansion");
        assert_eq!(frontier.frontier_nnz, 32);
    }

    #[test]
    fn sparse_selector_rejects_missing_result_digest() {
        let mut request = request(SparseKernelWorkloadClass::SpmvLike);
        request.result_digest = [0; 32];

        let error = select_sparse_kernel(request).unwrap_err();

        assert_eq!(error, SparseKernelSelectorError::MissingResultDigest);
    }

    #[test]
    fn sparse_selector_rejects_missing_transfer_bytes() {
        let mut request = request(SparseKernelWorkloadClass::SpmvLike);
        request.transfer_bytes = 0;

        let error = select_sparse_kernel(request).unwrap_err();

        assert_eq!(error, SparseKernelSelectorError::MissingTransferBytes);
    }
}
