use std::fmt;

use crate::backend::staging_reserve::CudaStorageReserveFailure;

/// Error returned when e-graph kernel work cannot be planned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CudaEGraphKernelPlanError {
    /// Threads per block was zero.
    ZeroThreadsPerBlock,
    /// Maximum blocks per launch was zero.
    ZeroMaxBlocksPerLaunch,
    /// Count arithmetic overflowed.
    CountOverflow {
        /// Field being computed.
        field: &'static str,
    },
    /// PTX generation received an invalid CUDA SM target.
    InvalidPtxTarget {
        /// Invalid `sm_XX` target.
        target_sm: u32,
    },
    /// Packed image metadata did not match the checked CUDA kernel view.
    ImageViewMismatch {
        /// Field that disagreed.
        field: &'static str,
        /// Count from the packed foundation image.
        image: usize,
        /// Count from the CUDA kernel view.
        view: usize,
    },
    /// A packed row child span pointed outside the packed child column.
    ImageColumnOutOfBounds {
        /// Column being decoded.
        column: &'static str,
        /// Row being decoded.
        row: u32,
        /// Start index into the column.
        start: usize,
        /// End index into the column.
        end: usize,
        /// Column length.
        len: usize,
    },
    /// A pair ordinal did not identify a valid row pair in a signature bucket.
    SignaturePairOrdinalOutOfBounds {
        /// Signature bucket being decoded.
        bucket_index: u32,
        /// Pair ordinal inside the bucket's triangular pair space.
        pair_ordinal: u64,
        /// Number of candidate pairs in the bucket.
        candidate_pair_count: u64,
    },
    /// A signature bucket's row range pointed outside the bucket row table.
    SignatureBucketRowsOutOfBounds {
        /// Signature bucket being decoded.
        bucket_index: u32,
        /// First row offset in the bucket row table.
        first_bucket_row: usize,
        /// Bucket row count.
        row_count: usize,
        /// Available row table length.
        bucket_rows_len: usize,
    },
    /// Planner storage reservation failed.
    StorageReserveFailed {
        /// Field being reserved.
        field: &'static str,
        /// Requested element count.
        requested: usize,
        /// Allocator error text.
        message: String,
    },
}

impl CudaStorageReserveFailure for CudaEGraphKernelPlanError {
    fn storage_reserve_failed(field: &'static str, requested: usize, message: String) -> Self {
        Self::StorageReserveFailed {
            field,
            requested,
            message,
        }
    }
}

impl fmt::Display for CudaEGraphKernelPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroThreadsPerBlock => write!(
                f,
                "CUDA e-graph kernel planner received zero threads per block. Fix: choose a non-zero launch width before planning equality-saturation work."
            ),
            Self::ZeroMaxBlocksPerLaunch => write!(
                f,
                "CUDA e-graph kernel planner received zero max blocks per launch. Fix: choose a non-zero launch partition limit."
            ),
            Self::CountOverflow { field } => write!(
                f,
                "CUDA e-graph kernel planner overflowed while computing {field}. Fix: shard the resident e-graph image before launch planning."
            ),
            Self::InvalidPtxTarget { target_sm } => write!(
                f,
                "CUDA e-graph structural-equivalence PTX generation received invalid sm_{target_sm}. Fix: pass the backend's probed CUDA PTX target."
            ),
            Self::ImageViewMismatch { field, image, view } => write!(
                f,
                "CUDA e-graph kernel planner received mismatched {field}: packed image has {image}, kernel view has {view}. Fix: build the view from the same upload plan/image."
            ),
            Self::ImageColumnOutOfBounds {
                column,
                row,
                start,
                end,
                len,
            } => write!(
                f,
                "CUDA e-graph kernel planner decoded row {row} span {column}[{start}..{end}) but {column} has {len} entries. Fix: rebuild the packed e-graph image from a validated snapshot."
            ),
            Self::SignaturePairOrdinalOutOfBounds {
                bucket_index,
                pair_ordinal,
                candidate_pair_count,
            } => write!(
                f,
                "CUDA e-graph signature bucket {bucket_index} pair ordinal {pair_ordinal} is outside {candidate_pair_count} candidate pairs. Fix: launch only planned pair-wave ranges."
            ),
            Self::SignatureBucketRowsOutOfBounds {
                bucket_index,
                first_bucket_row,
                row_count,
                bucket_rows_len,
            } => write!(
                f,
                "CUDA e-graph signature bucket {bucket_index} row range [{first_bucket_row}..{}) exceeds bucket row table length {bucket_rows_len}. Fix: rebuild the signature bucket plan.",
                first_bucket_row.saturating_add(*row_count)
            ),
            Self::StorageReserveFailed {
                field,
                requested,
                message,
            } => write!(
                f,
                "CUDA e-graph kernel planner could not reserve {requested} {field} entries: {message}. Fix: shard the resident e-graph image before launch planning."
            ),
        }
    }
}

impl std::error::Error for CudaEGraphKernelPlanError {}
