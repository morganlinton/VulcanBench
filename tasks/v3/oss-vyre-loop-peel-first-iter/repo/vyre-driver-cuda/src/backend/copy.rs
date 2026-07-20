//! Shared checked CUDA copy primitives.
//!
//! All CUDA host-to-device upload paths should pass through this module so
//! stream choice, zero-byte behavior, telemetry hooks, and future batched-copy
//! instrumentation live behind one boundary instead of repeated FFI blocks.

use std::ffi::c_void;

use cudarc::driver::sys::CUstream;
use vyre_driver::BackendError;

use super::allocations::cuda_check;

pub(crate) const CUDA_ASYNC_COPY_ALIGNMENT: usize = 16;

pub(crate) fn aligned_async_copy_len(byte_len: usize) -> Result<usize, BackendError> {
    if byte_len == 0 {
        return Ok(0);
    }
    byte_len
        .checked_add(CUDA_ASYNC_COPY_ALIGNMENT - 1)
        .map(|len| len & !(CUDA_ASYNC_COPY_ALIGNMENT - 1))
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: CUDA async transfer length {byte_len} cannot be rounded to {CUDA_ASYNC_COPY_ALIGNMENT}-byte alignment without overflowing usize."
            ),
        })
}

fn validate_nonzero_host_to_device_copy(
    dst: u64,
    src: *const c_void,
    stream: CUstream,
    label: &'static str,
) -> Result<(), BackendError> {
    if dst == 0 {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} received a null CUDA device destination for a non-zero host-to-device copy. Preserve descriptor ordering and allocate device storage before enqueueing the copy."
            ),
        });
    }
    if src.is_null() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} received a null host source for a non-zero host-to-device copy. Keep the pinned host staging allocation alive until the CUDA stream completes."
            ),
        });
    }
    if stream.is_null() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} received a null CUDA stream for a non-zero host-to-device copy. Use a backend-owned stream instead of the legacy default stream."
            ),
        });
    }
    Ok(())
}

fn validate_nonzero_device_to_host_copy(
    dst: *mut c_void,
    src: u64,
    stream: CUstream,
    label: &'static str,
) -> Result<(), BackendError> {
    if dst.is_null() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} received a null host destination for a non-zero device-to-host copy. Allocate pinned host readback storage before enqueueing the copy."
            ),
        });
    }
    if src == 0 {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} received a null CUDA device source for a non-zero device-to-host copy. Preserve output descriptor ordering and allocate device storage before readback."
            ),
        });
    }
    if stream.is_null() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} received a null CUDA stream for a non-zero device-to-host copy. Use a backend-owned stream instead of the legacy default stream."
            ),
        });
    }
    Ok(())
}

fn validate_nonzero_sync_device_to_host_copy(
    dst: *mut c_void,
    src: u64,
    label: &'static str,
) -> Result<(), BackendError> {
    if dst.is_null() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} received a null host destination for a non-zero synchronous device-to-host copy. Allocate readback storage before copying."
            ),
        });
    }
    if src == 0 {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} received a null CUDA device source for a non-zero synchronous device-to-host copy. Preserve output descriptor ordering and allocate device storage before readback."
            ),
        });
    }
    Ok(())
}

fn validate_nonzero_device_memset(
    dst: u64,
    stream: CUstream,
    label: &'static str,
) -> Result<(), BackendError> {
    if dst == 0 {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} received a null CUDA device destination for a non-zero memset. Allocate resident or transient device storage before enqueueing the clear."
            ),
        });
    }
    if stream.is_null() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} received a null CUDA stream for a non-zero memset. Use a backend-owned stream instead of the legacy default stream."
            ),
        });
    }
    Ok(())
}

/// Enqueue an async host-to-device copy on `stream`.
///
/// Zero-byte uploads are no-ops and do not touch CUDA. Non-zero uploads require
/// `src` and `dst` to be valid for `byte_len` bytes until `stream` reaches the
/// copy.
///
/// # Safety
///
/// The caller must guarantee that `src` points to a host allocation valid for
/// `byte_len`, `dst` is a valid device pointer for `byte_len`, and both remain
/// alive until the CUDA stream has completed the copy.
pub(crate) unsafe fn h2d_async_checked(
    dst: u64,
    src: *const c_void,
    byte_len: usize,
    stream: CUstream,
) -> Result<(), BackendError> {
    // SAFETY: Forwarding the caller's pointer and stream guarantees.
    unsafe { h2d_async_checked_with_label(dst, src, byte_len, stream, "cuMemcpyHtoDAsync_v2") }
}

/// Enqueue an async host-to-device copy with an operation label used in
/// diagnostics.
///
/// # Safety
///
/// Same as [`h2d_async_checked`].
pub(crate) unsafe fn h2d_async_checked_with_label(
    dst: u64,
    src: *const c_void,
    byte_len: usize,
    stream: CUstream,
    label: &'static str,
) -> Result<(), BackendError> {
    if byte_len == 0 {
        return Ok(());
    }
    validate_nonzero_host_to_device_copy(dst, src, stream, label)?;
    // SAFETY: The caller owns pointer validity and stream lifetime. This helper
    // centralizes result checking and the zero-byte no-op policy.
    unsafe {
        cuda_check(
            cudarc::driver::sys::cuMemcpyHtoDAsync_v2(dst, src, byte_len, stream),
            label,
        )
    }
}

/// Enqueue an async device-to-host copy on `stream`.
///
/// Zero-byte readbacks are no-ops and do not touch CUDA.
///
/// # Safety
///
/// The caller must guarantee that `dst` points to host storage valid for
/// `byte_len`, `src` is a valid device pointer for `byte_len`, and both remain
/// alive until the CUDA stream has completed the copy.
pub(crate) unsafe fn d2h_async_checked(
    dst: *mut c_void,
    src: u64,
    byte_len: usize,
    stream: CUstream,
) -> Result<(), BackendError> {
    // SAFETY: Forwarding the caller's pointer and stream guarantees.
    unsafe { d2h_async_checked_with_label(dst, src, byte_len, stream, "cuMemcpyDtoHAsync_v2") }
}

/// Enqueue an async device-to-host copy with an operation label used in
/// diagnostics.
///
/// # Safety
///
/// Same as [`d2h_async_checked`].
pub(crate) unsafe fn d2h_async_checked_with_label(
    dst: *mut c_void,
    src: u64,
    byte_len: usize,
    stream: CUstream,
    label: &'static str,
) -> Result<(), BackendError> {
    if byte_len == 0 {
        return Ok(());
    }
    validate_nonzero_device_to_host_copy(dst, src, stream, label)?;
    // SAFETY: The caller owns pointer validity and stream lifetime. This helper
    // centralizes result checking and the zero-byte no-op policy.
    unsafe {
        cuda_check(
            cudarc::driver::sys::cuMemcpyDtoHAsync_v2(dst, src, byte_len, stream),
            label,
        )
    }
}

/// Execute a synchronous device-to-host copy.
///
/// Zero-byte readbacks are no-ops and do not touch CUDA.
///
/// # Safety
///
/// The caller must guarantee that `dst` points to host storage valid for
/// `byte_len`, `src` is a valid device pointer for `byte_len`, and all prior
/// device writes visible to the copy have completed or are otherwise ordered.
pub(crate) unsafe fn d2h_sync_checked(
    dst: *mut c_void,
    src: u64,
    byte_len: usize,
) -> Result<(), BackendError> {
    // SAFETY: Forwarding the caller's pointer and ordering guarantees.
    unsafe { d2h_sync_checked_with_label(dst, src, byte_len, "cuMemcpyDtoH_v2") }
}

/// Execute a synchronous device-to-host copy with an operation label used in
/// diagnostics.
///
/// # Safety
///
/// Same as [`d2h_sync_checked`].
pub(crate) unsafe fn d2h_sync_checked_with_label(
    dst: *mut c_void,
    src: u64,
    byte_len: usize,
    label: &'static str,
) -> Result<(), BackendError> {
    if byte_len == 0 {
        return Ok(());
    }
    validate_nonzero_sync_device_to_host_copy(dst, src, label)?;
    // SAFETY: The caller owns pointer validity and ordering. This helper
    // centralizes result checking and the zero-byte no-op policy.
    unsafe {
        cuda_check(
            cudarc::driver::sys::cuMemcpyDtoH_v2(dst, src, byte_len),
            label,
        )
    }
}

/// Enqueue an async byte-pattern device memset on `stream`.
///
/// Zero-byte clears are no-ops and do not touch CUDA.
///
/// # Safety
///
/// The caller must guarantee that `dst` is a valid device pointer for
/// `byte_len` bytes and remains alive until the CUDA stream has completed the
/// memset.
pub(crate) unsafe fn memset_d8_async_checked(
    dst: u64,
    value: u8,
    byte_len: usize,
    stream: CUstream,
) -> Result<(), BackendError> {
    if byte_len == 0 {
        return Ok(());
    }
    validate_nonzero_device_memset(dst, stream, "cuMemsetD8Async")?;
    // SAFETY: The caller owns pointer validity and stream lifetime. This helper
    // centralizes result checking and zero-byte no-op behavior.
    unsafe {
        cuda_check(
            cudarc::driver::sys::cuMemsetD8Async(dst, value, byte_len, stream),
            "cuMemsetD8Async",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_byte_h2d_copy_is_noop_before_cuda_ffi() {
        // SAFETY: The helper returns before dereferencing pointers or using the
        // stream when byte_len is zero.
        let result = unsafe { h2d_async_checked(0, std::ptr::null(), 0, std::ptr::null_mut()) };

        assert_eq!(
            result,
            Ok(()),
            "Fix: zero-byte H2D copies must not touch CUDA or require a live stream."
        );
    }

    #[test]
    fn zero_byte_d2h_copy_is_noop_before_cuda_ffi() {
        // SAFETY: The helper returns before dereferencing pointers or using the
        // stream when byte_len is zero.
        let result = unsafe { d2h_async_checked(std::ptr::null_mut(), 0, 0, std::ptr::null_mut()) };

        assert_eq!(
            result,
            Ok(()),
            "Fix: zero-byte D2H copies must not touch CUDA or require a live stream."
        );
    }

    #[test]
    fn zero_byte_sync_d2h_copy_is_noop_before_cuda_ffi() {
        // SAFETY: The helper returns before dereferencing pointers when byte_len
        // is zero.
        let result = unsafe { d2h_sync_checked(std::ptr::null_mut(), 0, 0) };

        assert_eq!(
            result,
            Ok(()),
            "Fix: zero-byte synchronous D2H copies must not touch CUDA."
        );
    }

    #[test]
    fn zero_byte_memset_is_noop_before_cuda_ffi() {
        // SAFETY: The helper returns before using the device pointer or stream
        // when byte_len is zero.
        let result = unsafe { memset_d8_async_checked(0, 0, 0, std::ptr::null_mut()) };

        assert_eq!(
            result,
            Ok(()),
            "Fix: zero-byte CUDA memsets must not touch CUDA or require a live stream."
        );
    }

    #[test]
    fn h2d_copy_helper_is_single_ffi_boundary() {
        let source = include_str!("copy.rs");
        assert!(source.contains("cuMemcpyHtoDAsync_v2"));
        assert!(source.contains("cuMemcpyDtoHAsync_v2"));
        assert!(source.contains("cuMemcpyDtoH_v2"));
        assert!(source.contains("cuMemsetD8Async"));
        assert!(
            source.contains("if byte_len == 0"),
            "Fix: shared copy primitives must preserve zero-byte no-op behavior."
        );
    }

    #[test]
    fn aligned_async_copy_len_rounds_to_cuda_dma_boundary() {
        assert_eq!(aligned_async_copy_len(0).unwrap(), 0);
        assert_eq!(
            aligned_async_copy_len(1).unwrap(),
            CUDA_ASYNC_COPY_ALIGNMENT
        );
        assert_eq!(
            aligned_async_copy_len(15).unwrap(),
            CUDA_ASYNC_COPY_ALIGNMENT
        );
        assert_eq!(
            aligned_async_copy_len(16).unwrap(),
            CUDA_ASYNC_COPY_ALIGNMENT
        );
        assert_eq!(
            aligned_async_copy_len(17).unwrap(),
            CUDA_ASYNC_COPY_ALIGNMENT * 2
        );
        assert!(
            aligned_async_copy_len(usize::MAX).is_err(),
            "Fix: CUDA async copy padding must report usize overflow instead of wrapping."
        );
    }

    #[test]
    fn nonzero_copy_helpers_reject_null_pointers_before_cuda_ffi() {
        let mut byte = 0u8;
        let host_ptr = (&mut byte as *mut u8).cast::<c_void>();
        let stream = std::ptr::NonNull::<cudarc::driver::sys::CUstream_st>::dangling().as_ptr();

        // SAFETY: The null device destination is intentional and must be
        // rejected by validation before CUDA FFI.
        let h2d_null_dst = unsafe { h2d_async_checked(0, host_ptr.cast_const(), 1, stream) }
            .expect_err("Fix: non-zero H2D copy with null device destination must fail pre-FFI.");
        assert!(h2d_null_dst
            .to_string()
            .contains("null CUDA device destination"));

        // SAFETY: The null host source is intentional and must be rejected by
        // validation before CUDA FFI.
        let h2d_null_src = unsafe { h2d_async_checked(1, std::ptr::null(), 1, stream) }
            .expect_err("Fix: non-zero H2D copy with null host source must fail pre-FFI.");
        assert!(h2d_null_src.to_string().contains("null host source"));

        let h2d_null_stream = {
            // SAFETY: The null stream is intentional and must be rejected by
            // validation before CUDA FFI.
            unsafe { h2d_async_checked(1, host_ptr.cast_const(), 1, std::ptr::null_mut()) }
        }
        .expect_err("Fix: non-zero H2D copy with null stream must fail pre-FFI.");
        assert!(h2d_null_stream.to_string().contains("null CUDA stream"));

        // SAFETY: The null host destination is intentional and must be rejected
        // by validation before CUDA FFI.
        let d2h_null_dst = unsafe { d2h_async_checked(std::ptr::null_mut(), 1, 1, stream) }
            .expect_err("Fix: non-zero D2H copy with null host destination must fail pre-FFI.");
        assert!(d2h_null_dst.to_string().contains("null host destination"));

        // SAFETY: The null device source is intentional and must be rejected by
        // validation before CUDA FFI.
        let d2h_null_src = unsafe { d2h_async_checked(host_ptr, 0, 1, stream) }
            .expect_err("Fix: non-zero D2H copy with null device source must fail pre-FFI.");
        assert!(d2h_null_src.to_string().contains("null CUDA device source"));

        // SAFETY: The null stream is intentional and must be rejected by
        // validation before CUDA FFI.
        let d2h_null_stream = unsafe { d2h_async_checked(host_ptr, 1, 1, std::ptr::null_mut()) }
            .expect_err("Fix: non-zero D2H copy with null stream must fail pre-FFI.");
        assert!(d2h_null_stream.to_string().contains("null CUDA stream"));

        // SAFETY: The null host destination is intentional and must be rejected
        // by validation before CUDA FFI.
        let sync_null_dst = unsafe { d2h_sync_checked(std::ptr::null_mut(), 1, 1) }.expect_err(
            "Fix: non-zero sync D2H copy with null host destination must fail pre-FFI.",
        );
        assert!(sync_null_dst.to_string().contains("null host destination"));

        // SAFETY: The null device source is intentional and must be rejected by
        // validation before CUDA FFI.
        let sync_null_src = unsafe { d2h_sync_checked(host_ptr, 0, 1) }
            .expect_err("Fix: non-zero sync D2H copy with null device source must fail pre-FFI.");
        assert!(sync_null_src
            .to_string()
            .contains("null CUDA device source"));

        // SAFETY: The null device destination is intentional and must be
        // rejected by validation before CUDA FFI.
        let memset_null_dst = unsafe { memset_d8_async_checked(0, 0, 1, stream) }
            .expect_err("Fix: non-zero memset with null device destination must fail pre-FFI.");
        assert!(memset_null_dst
            .to_string()
            .contains("null CUDA device destination"));

        // SAFETY: The null stream is intentional and must be rejected by
        // validation before CUDA FFI.
        let memset_null_stream = unsafe { memset_d8_async_checked(1, 0, 1, std::ptr::null_mut()) }
            .expect_err("Fix: non-zero memset with null stream must fail pre-FFI.");
        assert!(memset_null_stream.to_string().contains("null CUDA stream"));
    }

    #[test]
    fn copy_boundary_validates_nonzero_inputs_before_ffi() {
        let source = include_str!("copy.rs");
        assert!(
            source.contains("validate_nonzero_host_to_device_copy")
                && source.contains("validate_nonzero_device_to_host_copy")
                && source.contains("validate_nonzero_sync_device_to_host_copy")
                && source.contains("validate_nonzero_device_memset"),
            "Fix: shared CUDA copy primitives must validate non-zero pointer and stream inputs before FFI."
        );
    }

    #[test]
    fn resident_staged_sync_readback_uses_shared_copy_helper() {
        let resident_dispatch = [
            include_str!("resident_dispatch/helpers.rs"),
            include_str!("resident_dispatch/borrowed.rs"),
            include_str!("resident_dispatch/async_dispatch.rs"),
            include_str!("resident_dispatch/batch.rs"),
            include_str!("resident_dispatch/sync.rs"),
            include_str!("resident_dispatch/sequence_api.rs"),
            include_str!("resident_dispatch/sequence_fused.rs"),
            include_str!("resident_dispatch/timed.rs"),
        ]
        .concat();
        let ffi = concat!("cudarc::driver::sys::", "cuMemcpyDtoH_v2(");

        assert_eq!(
            resident_dispatch.matches(ffi).count(),
            0,
            "Fix: resident staged synchronous readback must route through copy::d2h_sync_checked_with_label."
        );
        assert!(
            resident_dispatch.contains("copy::d2h_sync_checked_with_label"),
            "Fix: resident staged synchronous readback must use the shared copy boundary."
        );
    }
}
