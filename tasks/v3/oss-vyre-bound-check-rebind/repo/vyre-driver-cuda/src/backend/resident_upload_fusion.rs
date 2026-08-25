//! CUDA-facing resident H2D upload interval fusion adapter.
//!
//! Ordered overwrite fusion is backend-neutral. This module preserves the CUDA
//! domain names used by resident IO/dispatch while delegating the actual fusion
//! algorithm to `vyre-driver`.

use smallvec::SmallVec;
use vyre_driver::resident_transfer_fusion::{
    fuse_resident_upload_copies as driver_fuse_resident_upload_copies,
    push_resident_upload_copy as driver_push_resident_upload_copy,
    ResidentUploadBytes as DriverResidentUploadBytes,
    ResidentUploadCopy as DriverResidentUploadCopy,
};
use vyre_driver::BackendError;

/// Host bytes for one resident upload interval.
pub(crate) type ResidentUploadBytes<'a> = DriverResidentUploadBytes<'a>;

/// One validated host-to-device upload request.
pub(crate) type ResidentUploadCopy<'a> = DriverResidentUploadCopy<'a>;

/// Push one non-empty upload copy and account its requested bytes.
pub(crate) fn push_resident_upload_copy<'a>(
    copies: &mut SmallVec<[ResidentUploadCopy<'a>; 8]>,
    uploaded_bytes: &mut u64,
    handle_id: u64,
    dst_ptr: u64,
    bytes: &'a [u8],
    label: &str,
) -> Result<(), BackendError> {
    driver_push_resident_upload_copy(copies, uploaded_bytes, handle_id, dst_ptr, bytes, label)
}

/// Fuse same-handle adjacent or overlapping upload intervals.
pub(crate) fn fuse_resident_upload_copies<'a>(
    copies: SmallVec<[ResidentUploadCopy<'a>; 8]>,
) -> Result<(SmallVec<[ResidentUploadCopy<'a>; 8]>, u64), BackendError> {
    driver_fuse_resident_upload_copies(copies)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use smallvec::SmallVec;

    use super::{
        fuse_resident_upload_copies, push_resident_upload_copy, ResidentUploadBytes,
        ResidentUploadCopy,
    };

    #[test]
    fn cuda_upload_fusion_is_adapter_not_algorithm_fork() {
        let production = include_str!("resident_upload_fusion.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: resident upload fusion production source must precede tests.");

        assert!(
            production.contains("vyre_driver::resident_transfer_fusion"),
            "Fix: CUDA resident upload fusion must delegate to the backend-neutral driver owner."
        );
        for forbidden in [
            "TransferAccountingPolicy",
            "fn coalesce_resident_upload_tail",
            "fn resident_upload_copy_owned",
            "checked_add_u64_usize_offset_lazy",
            "reserved_vec",
            "reserve_vec",
            "reserve_smallvec",
        ] {
            assert!(
                !production.contains(forbidden),
                "Fix: CUDA resident upload fusion must not carry local ordered-overwrite fusion logic: {forbidden}."
            );
        }
    }

    #[test]
    fn empty_resident_upload_copy_does_not_schedule_dma() {
        let mut copies = SmallVec::<[ResidentUploadCopy<'_>; 8]>::new();
        let mut uploaded_bytes = 0_u64;

        push_resident_upload_copy(&mut copies, &mut uploaded_bytes, 7, 0xCAFE, &[], "unit")
            .expect("Fix: empty resident upload staging must not fail.");

        assert!(copies.is_empty());
        assert_eq!(uploaded_bytes, 0);
    }

    #[test]
    fn resident_upload_copy_accounts_non_empty_bytes_once() {
        let bytes = [1_u8, 2, 3, 4, 5];
        let mut copies = SmallVec::<[ResidentUploadCopy<'_>; 8]>::new();
        let mut uploaded_bytes = 0_u64;

        push_resident_upload_copy(&mut copies, &mut uploaded_bytes, 9, 0xBEEF, &bytes, "unit")
            .expect("Fix: non-empty resident upload staging must account bytes.");

        assert_eq!(copies.len(), 1);
        assert_eq!(copies[0].handle_id, 9);
        assert_eq!(copies[0].dst_ptr, 0xBEEF);
        assert_eq!(copies[0].bytes.as_slice(), bytes.as_slice());
        assert_eq!(uploaded_bytes, bytes.len() as u64);
    }

    #[test]
    fn resident_upload_copy_accounting_failure_is_transactional() {
        let bytes = [42_u8];
        let mut copies = SmallVec::<[ResidentUploadCopy<'_>; 8]>::new();
        let mut uploaded_bytes = u64::MAX;

        let error =
            push_resident_upload_copy(&mut copies, &mut uploaded_bytes, 9, 0xBEEF, &bytes, "unit")
                .expect_err("Fix: resident upload byte-accounting overflow must reject the copy.");

        assert!(
            error.to_string().contains("byte accounting overflowed"),
            "overflow diagnostic must identify the accounting bug: {error}"
        );
        assert!(
            copies.is_empty(),
            "Fix: failed resident upload accounting must not leave an unaccounted DMA copy queued."
        );
        assert_eq!(
            uploaded_bytes,
            u64::MAX,
            "Fix: failed resident upload accounting must not partially mutate byte counters."
        );
    }

    #[test]
    fn generated_resident_upload_fusion_preserves_ordered_write_semantics() {
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

            let expected = materialize_requests(&requests);
            let requested_bytes = requests
                .iter()
                .map(|request| request.bytes.len() as u64)
                .sum::<u64>();
            let (fused, fused_bytes) = fuse_resident_upload_copies(copies)
                .expect("Fix: generated resident upload fusion must not overflow");

            assert_eq!(
                materialize_fused(&fused),
                expected,
                "Fix: fused resident uploads must preserve ordered write semantics for seed {seed}."
            );
            assert!(
                fused_bytes <= requested_bytes,
                "Fix: fused resident upload byte accounting must not exceed requested bytes for seed {seed}."
            );
            for pair in fused.as_slice().windows(2) {
                let left = &pair[0];
                let right = &pair[1];
                let left_end = left.dst_ptr + left.bytes.len() as u64;
                assert!(
                    left.handle_id != right.handle_id
                        || right.dst_ptr < left.dst_ptr
                        || right.dst_ptr > left_end,
                    "Fix: resident upload fusion left a mergeable monotonic same-handle interval for seed {seed}."
                );
            }
        }
    }

    #[test]
    fn adjacent_raw_destinations_from_distinct_handles_do_not_fuse_uploads() {
        let first = [1u8, 2, 3, 4];
        let second = [5u8, 6, 7, 8];
        let copies = SmallVec::<[ResidentUploadCopy<'_>; 8]>::from_vec(vec![
            ResidentUploadCopy {
                handle_id: 1,
                dst_ptr: 100,
                bytes: ResidentUploadBytes::Borrowed(first.as_slice()),
            },
            ResidentUploadCopy {
                handle_id: 2,
                dst_ptr: 104,
                bytes: ResidentUploadBytes::Borrowed(second.as_slice()),
            },
        ]);

        let (fused, fused_bytes) = fuse_resident_upload_copies(copies)
            .expect("Fix: distinct-handle adjacent uploads must fuse-check without error");

        assert_eq!(
            fused.len(),
            2,
            "Fix: adjacent raw destinations from distinct resident allocations must not coalesce."
        );
        assert_eq!(fused_bytes, 8);
    }

    #[test]
    fn backward_overlapping_uploads_fuse_and_preserve_later_prefix_write() {
        let first = [4u8, 5, 6, 7];
        let second = [1u8, 2, 9, 8];
        let copies = SmallVec::<[ResidentUploadCopy<'_>; 8]>::from_vec(vec![
            ResidentUploadCopy {
                handle_id: 7,
                dst_ptr: 104,
                bytes: ResidentUploadBytes::Borrowed(first.as_slice()),
            },
            ResidentUploadCopy {
                handle_id: 7,
                dst_ptr: 102,
                bytes: ResidentUploadBytes::Borrowed(second.as_slice()),
            },
        ]);

        let (fused, fused_bytes) = fuse_resident_upload_copies(copies)
            .expect("Fix: backward-overlap resident uploads must fuse without error");

        assert_eq!(
            fused.len(),
            1,
            "Fix: backward-overlapping same-handle uploads must coalesce into one H2D copy."
        );
        assert_eq!(fused[0].dst_ptr, 102);
        assert_eq!(fused[0].bytes.as_slice(), &[1, 2, 9, 8, 6, 7]);
        assert_eq!(fused_bytes, 6);
    }

    #[test]
    fn later_full_overwrite_replaces_prior_upload_without_materializing_old_bytes() {
        let first = [1u8, 2, 3, 4];
        let second = [9u8, 8, 7, 6];
        let copies = SmallVec::<[ResidentUploadCopy<'_>; 8]>::from_vec(vec![
            ResidentUploadCopy {
                handle_id: 7,
                dst_ptr: 100,
                bytes: ResidentUploadBytes::Borrowed(first.as_slice()),
            },
            ResidentUploadCopy {
                handle_id: 7,
                dst_ptr: 100,
                bytes: ResidentUploadBytes::Borrowed(second.as_slice()),
            },
        ]);

        let (fused, fused_bytes) = fuse_resident_upload_copies(copies)
            .expect("Fix: full-overwrite resident uploads must fuse without error");

        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].dst_ptr, 100);
        assert_eq!(fused[0].bytes.as_slice(), second.as_slice());
        assert!(
            matches!(fused[0].bytes, ResidentUploadBytes::Borrowed(_)),
            "Fix: later full overwrite should keep the newer borrowed payload instead of allocating fused owned bytes."
        );
        assert_eq!(fused_bytes, second.len() as u64);
    }

    #[test]
    fn later_wider_overwrite_replaces_prior_upload_without_prefix_merge_allocation() {
        let first = [4u8, 5, 6, 7];
        let second = [1u8, 2, 3, 4, 5, 6, 7, 8];
        let copies = SmallVec::<[ResidentUploadCopy<'_>; 8]>::from_vec(vec![
            ResidentUploadCopy {
                handle_id: 9,
                dst_ptr: 104,
                bytes: ResidentUploadBytes::Borrowed(first.as_slice()),
            },
            ResidentUploadCopy {
                handle_id: 9,
                dst_ptr: 100,
                bytes: ResidentUploadBytes::Borrowed(second.as_slice()),
            },
        ]);

        let (fused, fused_bytes) = fuse_resident_upload_copies(copies)
            .expect("Fix: wider full-overwrite resident uploads must fuse without error");

        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].dst_ptr, 100);
        assert_eq!(fused[0].bytes.as_slice(), second.as_slice());
        assert!(
            matches!(fused[0].bytes, ResidentUploadBytes::Borrowed(_)),
            "Fix: wider full overwrite should replace the old interval instead of allocating a merged prefix buffer."
        );
        assert_eq!(fused_bytes, second.len() as u64);
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

    fn materialize_requests(requests: &[UploadRequest]) -> HashMap<(u64, u64), u8> {
        let mut memory = HashMap::new();
        for request in requests {
            for (offset, &byte) in request.bytes.iter().enumerate() {
                memory.insert((request.handle_id, request.dst_ptr + offset as u64), byte);
            }
        }
        memory
    }

    fn materialize_fused(copies: &[ResidentUploadCopy<'_>]) -> HashMap<(u64, u64), u8> {
        let mut memory = HashMap::new();
        for copy in copies {
            for (offset, &byte) in copy.bytes.as_slice().iter().enumerate() {
                memory.insert((copy.handle_id, copy.dst_ptr + offset as u64), byte);
            }
        }
        memory
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
