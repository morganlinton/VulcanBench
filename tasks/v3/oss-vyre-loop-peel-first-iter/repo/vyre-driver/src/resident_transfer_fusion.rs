//! Backend-neutral resident transfer interval fusion.
//!
//! Resident GPU resources are long-lived allocation handles with byte-addressed
//! transfer intervals. Backends can fuse overlapping or adjacent device-to-host
//! readback intervals without changing caller-visible output slices. This module
//! owns that pure interval policy so CUDA, WGPU, and future backends do not
//! carry divergent coalescing logic.

use smallvec::SmallVec;

use crate::ordering::{iter_is_monotonic_by_key, sort_by_key_if_needed};
use crate::reservation_policy::ReservationPolicy;
use crate::transfer_accounting::TransferAccountingPolicy;
use crate::BackendError;

const TRANSFER_FUSION_RESERVATION: ReservationPolicy = ReservationPolicy::new(
    "resident transfer interval fusion",
    "split the resident transfer batch before interval fusion",
);
const TRANSFER_ACCOUNTING: TransferAccountingPolicy = TransferAccountingPolicy::new(
    "resident transfer",
    "split the transfer into bounded chunks",
);

/// One validated resident transfer interval.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ResidentTransferInterval {
    /// Stable resident allocation identity. Adjacent raw pointers from
    /// different allocations must never be coalesced.
    pub handle_id: u64,
    /// Byte-addressed transfer start. Backends may use a raw device pointer or
    /// an allocation-relative offset as long as values are comparable within
    /// one `handle_id`.
    pub src: u64,
    /// Requested byte length.
    pub byte_len: usize,
}

/// How one original request is sliced out of a fused transfer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidentTransferView {
    /// Fused transfer slot.
    pub copy_slot: usize,
    /// Byte offset within the fused transfer.
    pub byte_offset: usize,
    /// Number of bytes materialized for the original request.
    pub byte_len: usize,
}

/// Fused transfer plan plus original-request views.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FusedResidentTransfers {
    /// Fused non-empty intervals.
    pub copies: SmallVec<[ResidentTransferInterval; 8]>,
    /// Original request views, in caller order.
    pub views: SmallVec<[ResidentTransferView; 8]>,
    /// Number of non-empty fused intervals in `copies`.
    pub non_empty_copy_count: usize,
    /// Total bytes copied after handle-scoped interval fusion.
    pub bytes: u64,
}

/// Host bytes for one resident upload interval.
pub enum ResidentUploadBytes<'a> {
    /// Caller-owned immutable upload bytes.
    Borrowed(&'a [u8]),
    /// Fused scratch bytes materialized from overlapping/adjacent writes.
    Owned(Vec<u8>),
}

impl ResidentUploadBytes<'_> {
    /// Borrow the upload bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        match self {
            Self::Borrowed(bytes) => bytes,
            Self::Owned(bytes) => bytes.as_slice(),
        }
    }

    /// Upload byte length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    /// Whether this upload has no bytes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// One validated host-to-device resident upload request.
pub struct ResidentUploadCopy<'a> {
    /// Stable resident allocation identity. Adjacent raw device pointers from
    /// different allocations must never be coalesced.
    pub handle_id: u64,
    /// Device destination pointer or allocation-relative destination offset.
    pub dst_ptr: u64,
    /// Host upload bytes.
    pub bytes: ResidentUploadBytes<'a>,
}

/// Fuse overlapping or adjacent transfer intervals, scoped by resident handle.
///
/// # Errors
///
/// Returns [`BackendError`] when staging allocation, pointer arithmetic, or byte
/// accounting overflows.
pub fn fuse_resident_transfer_intervals(
    requested: &[ResidentTransferInterval],
) -> Result<FusedResidentTransfers, BackendError> {
    let mut copies = SmallVec::<[ResidentTransferInterval; 8]>::new();
    TRANSFER_FUSION_RESERVATION.reserve_smallvec_to_capacity(
        &mut copies,
        requested.len(),
        "fused interval",
    )?;

    let mut views = SmallVec::<[ResidentTransferView; 8]>::new();
    TRANSFER_FUSION_RESERVATION.reserve_smallvec_to_capacity(
        &mut views,
        requested.len(),
        "interval view",
    )?;
    views.resize(requested.len(), ResidentTransferView::default());

    let ordered_is_monotonic =
        iter_is_monotonic_by_key(requested.iter().filter(|copy| copy.byte_len != 0), |copy| {
            (copy.handle_id, copy.src)
        });

    let mut non_empty_copy_count = 0usize;
    let mut bytes = 0u64;
    if ordered_is_monotonic {
        for (original_index, &copy) in requested.iter().enumerate() {
            if copy.byte_len != 0 {
                push_fused_resident_transfer(
                    &mut copies,
                    &mut views,
                    &mut non_empty_copy_count,
                    &mut bytes,
                    original_index,
                    copy,
                )?;
            }
        }
        return Ok(FusedResidentTransfers {
            copies,
            views,
            non_empty_copy_count,
            bytes,
        });
    }

    let mut ordered = SmallVec::<[(usize, ResidentTransferInterval); 8]>::new();
    TRANSFER_FUSION_RESERVATION.reserve_smallvec_to_capacity(
        &mut ordered,
        requested.len(),
        "ordered interval",
    )?;
    for (original_index, &copy) in requested.iter().enumerate() {
        if copy.byte_len != 0 {
            ordered.push((original_index, copy));
        }
    }
    sort_by_key_if_needed(&mut ordered, |(_, copy)| (copy.handle_id, copy.src));
    for (original_index, copy) in ordered {
        push_fused_resident_transfer(
            &mut copies,
            &mut views,
            &mut non_empty_copy_count,
            &mut bytes,
            original_index,
            copy,
        )?;
    }

    Ok(FusedResidentTransfers {
        copies,
        views,
        non_empty_copy_count,
        bytes,
    })
}

fn push_fused_resident_transfer(
    copies: &mut SmallVec<[ResidentTransferInterval; 8]>,
    views: &mut [ResidentTransferView],
    non_empty_copy_count: &mut usize,
    bytes: &mut u64,
    original_index: usize,
    copy: ResidentTransferInterval,
) -> Result<(), BackendError> {
    let copy_len = transfer_len_u64(copy.byte_len, "transfer byte length")?;
    let copy_end = copy
        .src
        .checked_add(copy_len)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: resident transfer pointer arithmetic overflowed for handle {} at source {} len {}.",
                copy.handle_id, copy.src, copy.byte_len
            ),
        })?;

    let mut copy_slot = copies.len();
    let mut copy_start = copy.src;
    if let Some(last) = copies.last_mut() {
        let last_len = transfer_len_u64(last.byte_len, "fused transfer byte length")?;
        let last_end = last
            .src
            .checked_add(last_len)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: resident fused transfer pointer arithmetic overflowed for handle {} at source {} len {}.",
                    last.handle_id, last.src, last.byte_len
                ),
            })?;
        if last.handle_id == copy.handle_id && copy.src <= last_end {
            copy_slot -= 1;
            copy_start = last.src;
            if copy_end > last_end {
                let extension_len =
                    usize::try_from(copy_end - last_end).map_err(|_| BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: resident transfer fusion for handle {} exceeds host addressable memory; split the transfer.",
                            copy.handle_id
                        ),
                    })?;
                last.byte_len =
                    usize::try_from(copy_end - last.src).map_err(|_| BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: resident transfer fusion for handle {} exceeds host addressable memory; split the transfer.",
                            copy.handle_id
                        ),
                    })?;
                add_bytes(bytes, extension_len, "fused resident transfer")?;
            }
        } else {
            copies.push(copy);
            add_copy_count(non_empty_copy_count, "fused resident transfer")?;
            add_bytes(bytes, copy.byte_len, "fused resident transfer")?;
        }
    } else {
        copies.push(copy);
        add_copy_count(non_empty_copy_count, "fused resident transfer")?;
        add_bytes(bytes, copy.byte_len, "fused resident transfer")?;
    }

    views[original_index] = ResidentTransferView {
        copy_slot,
        byte_offset: usize::try_from(copy.src - copy_start).map_err(|_| {
            BackendError::InvalidProgram {
                fix: format!(
                    "Fix: resident transfer fusion for handle {} produced an unaddressable output offset.",
                    copy.handle_id
                ),
            }
        })?,
        byte_len: copy.byte_len,
    };
    Ok(())
}

fn transfer_len_u64(bytes: usize, label: &str) -> Result<u64, BackendError> {
    TRANSFER_ACCOUNTING.bytes_to_u64(bytes, label)
}

fn add_bytes(total: &mut u64, bytes: usize, label: &str) -> Result<(), BackendError> {
    TRANSFER_ACCOUNTING.add_bytes(total, bytes, label)
}

fn add_copy_count(total: &mut usize, label: &str) -> Result<(), BackendError> {
    TRANSFER_ACCOUNTING.add_copy_count(total, label)
}

/// Push one non-empty resident upload copy and account its requested bytes.
///
/// # Errors
///
/// Returns [`BackendError`] when staging queue growth or byte accounting
/// overflows.
pub fn push_resident_upload_copy<'a>(
    copies: &mut SmallVec<[ResidentUploadCopy<'a>; 8]>,
    uploaded_bytes: &mut u64,
    handle_id: u64,
    dst_ptr: u64,
    bytes: &'a [u8],
    label: &str,
) -> Result<(), BackendError> {
    if bytes.is_empty() {
        return Ok(());
    }
    let new_len = crate::accounting::checked_add_usize_lazy(copies.len(), 1, || {
        BackendError::InvalidProgram {
            fix: format!(
                "Fix: resident {label} upload copy queue length overflowed; split the resident upload batch."
            ),
        }
    })?;
    TRANSFER_FUSION_RESERVATION.reserve_smallvec_to_capacity(
        copies,
        new_len,
        "resident upload copy queue",
    )?;
    add_bytes(uploaded_bytes, bytes.len(), label)?;
    copies.push(ResidentUploadCopy {
        handle_id,
        dst_ptr,
        bytes: ResidentUploadBytes::Borrowed(bytes),
    });
    Ok(())
}

/// Fuse same-handle adjacent or overlapping resident upload intervals.
///
/// Fusion only folds into the immediately preceding fused interval, never
/// globally sorts caller uploads, and therefore preserves ordered write
/// semantics.
///
/// # Errors
///
/// Returns [`BackendError`] when pointer arithmetic, byte accounting, or scratch
/// allocation fails.
pub fn fuse_resident_upload_copies<'a>(
    copies: SmallVec<[ResidentUploadCopy<'a>; 8]>,
) -> Result<(SmallVec<[ResidentUploadCopy<'a>; 8]>, u64), BackendError> {
    let mut fused = SmallVec::<[ResidentUploadCopy<'a>; 8]>::new();
    TRANSFER_FUSION_RESERVATION.reserve_smallvec_to_capacity(
        &mut fused,
        copies.len(),
        "fused resident upload copy",
    )?;

    for copy in copies {
        let copy_len = copy.bytes.len();
        let copy_end = checked_upload_end(copy.dst_ptr, copy_len, copy.handle_id, "upload")?;
        if let Some(last) = fused.last_mut() {
            let last_len = last.bytes.len();
            let last_end =
                checked_upload_end(last.dst_ptr, last_len, last.handle_id, "fused upload")?;
            if last.handle_id == copy.handle_id
                && copy.dst_ptr <= last_end
                && copy_end >= last.dst_ptr
            {
                if copy.dst_ptr <= last.dst_ptr && copy_end >= last_end {
                    *last = copy;
                    coalesce_resident_upload_tail(&mut fused)?;
                    continue;
                }
                let new_start = last.dst_ptr.min(copy.dst_ptr);
                let new_end = last_end.max(copy_end);
                let new_len = resident_upload_len(new_end - new_start, copy.handle_id)?;
                if new_start == last.dst_ptr {
                    let offset =
                        resident_upload_offset(copy.dst_ptr - last.dst_ptr, copy.handle_id)?;
                    let copy_bytes = copy.bytes.as_slice();
                    let last_bytes = resident_upload_copy_owned(last)?;
                    if new_len > last_bytes.len() {
                        TRANSFER_FUSION_RESERVATION.reserve_vec_to_capacity(
                            last_bytes,
                            new_len,
                            "fused resident upload bytes",
                        )?;
                        last_bytes.resize(new_len, 0);
                    }
                    last_bytes[offset..offset + copy_len].copy_from_slice(copy_bytes);
                } else {
                    let last_offset =
                        resident_upload_offset(last.dst_ptr - new_start, copy.handle_id)?;
                    let copy_offset =
                        resident_upload_offset(copy.dst_ptr - new_start, copy.handle_id)?;
                    let mut merged = TRANSFER_FUSION_RESERVATION
                        .reserved_vec(new_len, "fused resident upload bytes")?;
                    merged.resize(new_len, 0);
                    let last_bytes = last.bytes.as_slice();
                    merged[last_offset..last_offset + last_bytes.len()].copy_from_slice(last_bytes);
                    merged[copy_offset..copy_offset + copy_len]
                        .copy_from_slice(copy.bytes.as_slice());
                    last.dst_ptr = new_start;
                    last.bytes = ResidentUploadBytes::Owned(merged);
                }
                coalesce_resident_upload_tail(&mut fused)?;
                continue;
            }
        }

        fused.push(copy);
    }

    let uploaded_bytes = fused_resident_upload_bytes(&fused)?;
    Ok((fused, uploaded_bytes))
}

fn coalesce_resident_upload_tail<'a>(
    fused: &mut SmallVec<[ResidentUploadCopy<'a>; 8]>,
) -> Result<(), BackendError> {
    loop {
        if fused.len() < 2 {
            return Ok(());
        }
        let Some(last) = fused.pop() else {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: resident upload tail fusion lost the last copy after a length check; keep fusion mutation single-threaded.".to_string(),
            });
        };
        let Some(previous) = fused.last_mut() else {
            fused.push(last);
            return Err(BackendError::InvalidProgram {
                fix: "Fix: resident upload tail fusion lost the previous copy after a length check; keep fusion mutation single-threaded.".to_string(),
            });
        };
        let previous_len = previous.bytes.len();
        let previous_end = checked_upload_end(
            previous.dst_ptr,
            previous_len,
            previous.handle_id,
            "previous fused upload",
        )?;
        let last_len = last.bytes.len();
        let last_end =
            checked_upload_end(last.dst_ptr, last_len, last.handle_id, "last fused upload")?;
        if previous.handle_id != last.handle_id
            || last.dst_ptr > previous_end
            || last_end < previous.dst_ptr
        {
            fused.push(last);
            return Ok(());
        }

        let new_start = previous.dst_ptr.min(last.dst_ptr);
        let new_end = previous_end.max(last_end);
        let new_len = resident_upload_len(new_end - new_start, previous.handle_id)?;
        let previous_offset =
            resident_upload_offset(previous.dst_ptr - new_start, previous.handle_id)?;
        let last_offset = resident_upload_offset(last.dst_ptr - new_start, last.handle_id)?;
        let mut merged =
            TRANSFER_FUSION_RESERVATION.reserved_vec(new_len, "fused resident upload bytes")?;
        merged.resize(new_len, 0);
        let previous_bytes = previous.bytes.as_slice();
        merged[previous_offset..previous_offset + previous_bytes.len()]
            .copy_from_slice(previous_bytes);
        merged[last_offset..last_offset + last.bytes.len()].copy_from_slice(last.bytes.as_slice());
        previous.dst_ptr = new_start;
        previous.bytes = ResidentUploadBytes::Owned(merged);
    }
}

fn checked_upload_end(
    dst_ptr: u64,
    byte_len: usize,
    handle_id: u64,
    label: &str,
) -> Result<u64, BackendError> {
    crate::accounting::checked_add_u64_usize_offset_lazy(
        dst_ptr,
        byte_len,
        || {
            BackendError::InvalidProgram {
            fix: format!(
                "Fix: resident {label} byte length {byte_len} does not fit device pointer arithmetic for handle {handle_id}."
            ),
        }
        },
        || {
            BackendError::InvalidProgram {
            fix: format!(
                "Fix: resident {label} pointer arithmetic overflowed for handle {handle_id} at destination {dst_ptr} len {byte_len}."
            ),
        }
        },
    )
}

fn resident_upload_len(delta: u64, handle_id: u64) -> Result<usize, BackendError> {
    usize::try_from(delta).map_err(|_| BackendError::InvalidProgram {
        fix: format!(
            "Fix: resident upload fusion for handle {handle_id} exceeds host addressable memory; split the upload."
        ),
    })
}

fn resident_upload_offset(delta: u64, handle_id: u64) -> Result<usize, BackendError> {
    usize::try_from(delta).map_err(|_| BackendError::InvalidProgram {
        fix: format!(
            "Fix: resident upload fusion for handle {handle_id} produced an unaddressable offset."
        ),
    })
}

fn fused_resident_upload_bytes(copies: &[ResidentUploadCopy<'_>]) -> Result<u64, BackendError> {
    let mut uploaded_bytes = 0u64;
    for copy in copies {
        add_bytes(&mut uploaded_bytes, copy.bytes.len(), "fused upload")?;
    }
    Ok(uploaded_bytes)
}

fn resident_upload_copy_owned<'copy, 'a>(
    copy: &'copy mut ResidentUploadCopy<'a>,
) -> Result<&'copy mut Vec<u8>, BackendError> {
    let borrowed = match &copy.bytes {
        ResidentUploadBytes::Borrowed(bytes) => Some(*bytes),
        ResidentUploadBytes::Owned(_) => None,
    };
    if let Some(bytes) = borrowed {
        let mut owned =
            TRANSFER_FUSION_RESERVATION.reserved_vec(bytes.len(), "fused resident upload bytes")?;
        owned.extend_from_slice(bytes);
        copy.bytes = ResidentUploadBytes::Owned(owned);
    }
    match &mut copy.bytes {
        ResidentUploadBytes::Owned(bytes) => Ok(bytes),
        ResidentUploadBytes::Borrowed(_) => unreachable!("resident upload bytes were promoted"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use smallvec::SmallVec;

    use super::{
        fuse_resident_transfer_intervals, fuse_resident_upload_copies, push_resident_upload_copy,
        ResidentTransferInterval, ResidentUploadBytes, ResidentUploadCopy,
    };

    #[test]
    fn monotonic_same_handle_intervals_fuse_without_sorting() {
        let requested = [
            ResidentTransferInterval {
                handle_id: 7,
                src: 100,
                byte_len: 4,
            },
            ResidentTransferInterval {
                handle_id: 7,
                src: 104,
                byte_len: 8,
            },
            ResidentTransferInterval {
                handle_id: 7,
                src: 120,
                byte_len: 4,
            },
        ];

        let fused = fuse_resident_transfer_intervals(&requested)
            .expect("Fix: monotonic resident transfer fusion must not require sorting");

        assert_eq!(fused.copies.len(), 2);
        assert_eq!(fused.copies[0].src, 100);
        assert_eq!(fused.copies[0].byte_len, 12);
        assert_eq!(fused.views[1].copy_slot, 0);
        assert_eq!(fused.views[1].byte_offset, 4);
        assert_eq!(fused.bytes, 16);
    }

    #[test]
    fn adjacent_raw_offsets_from_distinct_handles_do_not_fuse() {
        let requested = [
            ResidentTransferInterval {
                handle_id: 1,
                src: 1024,
                byte_len: 8,
            },
            ResidentTransferInterval {
                handle_id: 2,
                src: 1032,
                byte_len: 8,
            },
        ];

        let fused = fuse_resident_transfer_intervals(&requested)
            .expect("Fix: distinct handles must not fail transfer fusion");

        assert_eq!(fused.copies.len(), 2);
        assert_eq!(fused.bytes, 16);
    }

    #[test]
    fn unordered_intervals_preserve_original_view_order() {
        let requested = [
            ResidentTransferInterval {
                handle_id: 3,
                src: 40,
                byte_len: 4,
            },
            ResidentTransferInterval {
                handle_id: 3,
                src: 32,
                byte_len: 12,
            },
            ResidentTransferInterval {
                handle_id: 1,
                src: 8,
                byte_len: 4,
            },
        ];

        let fused = fuse_resident_transfer_intervals(&requested)
            .expect("Fix: unordered resident transfer fusion must preserve caller views");

        assert_eq!(fused.views.len(), requested.len());
        assert_eq!(fused.copies.len(), 2);
        assert_eq!(
            materialize_view(
                &fused.copies,
                fused.views[0].copy_slot,
                fused.views[0].byte_offset,
                fused.views[0].byte_len
            ),
            materialize_request(requested[0])
        );
        assert_eq!(
            materialize_view(
                &fused.copies,
                fused.views[1].copy_slot,
                fused.views[1].byte_offset,
                fused.views[1].byte_len
            ),
            materialize_request(requested[1])
        );
        assert_eq!(
            materialize_view(
                &fused.copies,
                fused.views[2].copy_slot,
                fused.views[2].byte_offset,
                fused.views[2].byte_len
            ),
            materialize_request(requested[2])
        );
    }

    #[test]
    fn zero_byte_intervals_keep_empty_views_without_copy_accounting() {
        let requested = [
            ResidentTransferInterval {
                handle_id: 9,
                src: 1,
                byte_len: 0,
            },
            ResidentTransferInterval {
                handle_id: 9,
                src: 1,
                byte_len: 4,
            },
            ResidentTransferInterval {
                handle_id: 9,
                src: 5,
                byte_len: 0,
            },
        ];

        let fused = fuse_resident_transfer_intervals(&requested)
            .expect("Fix: zero-byte resident transfer views must not fail fusion");

        assert_eq!(fused.copies.len(), 1);
        assert_eq!(fused.non_empty_copy_count, 1);
        assert_eq!(fused.bytes, 4);
        assert_eq!(fused.views[0].byte_len, 0);
        assert_eq!(fused.views[2].byte_len, 0);
        assert_eq!(
            materialize_view(
                &fused.copies,
                fused.views[0].copy_slot,
                fused.views[0].byte_offset,
                fused.views[0].byte_len
            ),
            Vec::<u8>::new()
        );
    }

    #[test]
    fn pointer_arithmetic_overflow_is_reported_as_invalid_program() {
        let requested = [ResidentTransferInterval {
            handle_id: 5,
            src: u64::MAX - 1,
            byte_len: 4,
        }];

        let error = fuse_resident_transfer_intervals(&requested)
            .expect_err("Fix: overflowing resident transfer intervals must be rejected");

        assert!(
            error
                .to_string()
                .contains("resident transfer pointer arithmetic overflowed"),
            "Fix: overflow failures must point at resident transfer arithmetic, got {error}"
        );
    }

    #[test]
    fn generated_fusion_preserves_every_requested_output_and_accounts_union_bytes() {
        for seed in 0..8192_u64 {
            let requested = generated_requests(seed);
            let fused = fuse_resident_transfer_intervals(&requested)
                .expect("Fix: generated resident transfer requests must fuse without overflow");

            assert_eq!(fused.views.len(), requested.len());
            assert_eq!(fused.non_empty_copy_count, fused.copies.len());
            assert_eq!(
                fused.bytes,
                expected_union_bytes(&requested),
                "Fix: fused byte accounting must equal the handle-scoped interval union for seed {seed}."
            );

            for pair in fused.copies.windows(2) {
                let left = pair[0];
                let right = pair[1];
                let left_end = left.src + left.byte_len as u64;
                assert!(
                    left.handle_id != right.handle_id || right.src > left_end,
                    "Fix: fused copies must not leave mergeable same-handle intervals for seed {seed}."
                );
            }

            for (index, request) in requested.iter().enumerate() {
                let view = fused.views[index];
                assert_eq!(view.byte_len, request.byte_len);
                if request.byte_len != 0 {
                    assert!(view.copy_slot < fused.copies.len());
                    assert_eq!(
                        materialize_view(
                            &fused.copies,
                            view.copy_slot,
                            view.byte_offset,
                            view.byte_len
                        ),
                        materialize_request(*request),
                        "Fix: fused view must materialize request {index} for seed {seed}."
                    );
                }
            }
        }
    }

    #[test]
    fn generated_upload_fusion_preserves_ordered_write_semantics() {
        for seed in 0..4096_u64 {
            let requests = generated_upload_requests(seed);
            let mut copies = SmallVec::<[ResidentUploadCopy<'_>; 8]>::new();
            for request in &requests {
                copies.push(ResidentUploadCopy {
                    handle_id: request.handle_id,
                    dst_ptr: request.dst_ptr,
                    bytes: ResidentUploadBytes::Borrowed(request.bytes.as_slice()),
                });
            }

            let expected = materialize_upload_requests(&requests);
            let requested_bytes = requests
                .iter()
                .map(|request| request.bytes.len() as u64)
                .sum::<u64>();
            let (fused, fused_bytes) = fuse_resident_upload_copies(copies)
                .expect("Fix: generated resident upload fusion must not overflow");

            assert_eq!(
                materialize_upload_fused(&fused),
                expected,
                "Fix: shared resident upload fusion must preserve ordered write semantics for seed {seed}."
            );
            assert!(
                fused_bytes <= requested_bytes,
                "Fix: shared resident upload byte accounting must not exceed requested bytes for seed {seed}."
            );
            for pair in fused.as_slice().windows(2) {
                let left = &pair[0];
                let right = &pair[1];
                let left_end = left.dst_ptr + left.bytes.len() as u64;
                assert!(
                    left.handle_id != right.handle_id
                        || right.dst_ptr < left.dst_ptr
                        || right.dst_ptr > left_end,
                    "Fix: shared resident upload fusion left a mergeable monotonic same-handle interval for seed {seed}."
                );
            }
        }
    }

    #[test]
    fn upload_push_accounting_failure_is_transactional() {
        let bytes = [42_u8];
        let mut copies = SmallVec::<[ResidentUploadCopy<'_>; 8]>::new();
        let mut uploaded_bytes = u64::MAX;

        let error =
            push_resident_upload_copy(&mut copies, &mut uploaded_bytes, 9, 0xBEEF, &bytes, "unit")
                .expect_err("Fix: resident upload byte-accounting overflow must reject the copy.");

        assert!(error.to_string().contains("byte accounting overflowed"));
        assert!(copies.is_empty());
        assert_eq!(uploaded_bytes, u64::MAX);
    }

    struct UploadRequest {
        handle_id: u64,
        dst_ptr: u64,
        bytes: Vec<u8>,
    }

    fn generated_upload_requests(seed: u64) -> Vec<UploadRequest> {
        let mut state = seed ^ 0x5151_C0DA_9E37_1234;
        let count = 1 + (next_u64(&mut state) as usize % 16);
        let mut requests = Vec::with_capacity(count);
        for _ in 0..count {
            let handle_id = next_u64(&mut state) % 4;
            let dst_ptr = next_u64(&mut state) % 64;
            let len = 1 + (next_u64(&mut state) as usize % 16);
            let mut bytes = Vec::with_capacity(len);
            for _ in 0..len {
                bytes.push(next_u64(&mut state) as u8);
            }
            requests.push(UploadRequest {
                handle_id,
                dst_ptr,
                bytes,
            });
        }
        requests
    }

    fn materialize_upload_requests(requests: &[UploadRequest]) -> HashMap<(u64, u64), u8> {
        let mut memory = HashMap::new();
        for request in requests {
            for (offset, &byte) in request.bytes.iter().enumerate() {
                memory.insert((request.handle_id, request.dst_ptr + offset as u64), byte);
            }
        }
        memory
    }

    fn materialize_upload_fused(copies: &[ResidentUploadCopy<'_>]) -> HashMap<(u64, u64), u8> {
        let mut memory = HashMap::new();
        for copy in copies {
            for (offset, &byte) in copy.bytes.as_slice().iter().enumerate() {
                memory.insert((copy.handle_id, copy.dst_ptr + offset as u64), byte);
            }
        }
        memory
    }

    fn generated_requests(seed: u64) -> Vec<ResidentTransferInterval> {
        let count = (seed as usize % 17) + 1;
        let mut requests = Vec::with_capacity(count);
        for i in 0..count {
            let handle_id = ((seed >> (i % 11)) + i as u64) % 5;
            let src = ((seed.wrapping_mul(31) + (i as u64 * 13)) % 64) * 4;
            let byte_len = ((seed as usize + i * 7) % 9) * 4;
            requests.push(ResidentTransferInterval {
                handle_id,
                src,
                byte_len,
            });
        }
        if seed % 3 == 0 {
            requests.reverse();
        }
        requests
    }

    fn expected_union_bytes(requests: &[ResidentTransferInterval]) -> u64 {
        let mut covered = HashSet::<(u64, u64)>::new();
        for request in requests {
            for byte in 0..request.byte_len as u64 {
                covered.insert((request.handle_id, request.src + byte));
            }
        }
        covered.len() as u64
    }

    fn materialize_view(
        copies: &[ResidentTransferInterval],
        copy_slot: usize,
        byte_offset: usize,
        byte_len: usize,
    ) -> Vec<u8> {
        if byte_len == 0 {
            return Vec::new();
        }
        let copy = copies[copy_slot];
        (0..byte_len)
            .map(|offset| ((copy.src + (byte_offset + offset) as u64) & 0xFF) as u8)
            .collect()
    }

    fn materialize_request(request: ResidentTransferInterval) -> Vec<u8> {
        (0..request.byte_len)
            .map(|offset| ((request.src + offset as u64) & 0xFF) as u8)
            .collect()
    }

    fn next_u64(state: &mut u64) -> u64 {
        let mut x = *state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *state = x;
        x
    }
}
