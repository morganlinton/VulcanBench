//! Checked CUDA host-memory registration boundary.

use std::ffi::c_void;

use cudarc::driver::sys::{CUresult, CU_MEMHOSTALLOC_PORTABLE, CU_MEMHOSTREGISTER_PORTABLE};
use vyre_driver::BackendError;

use super::allocations::cuda_check;

fn validate_nonzero_host_range(
    ptr: u64,
    byte_len: usize,
    label: &'static str,
) -> Result<(), BackendError> {
    if ptr == 0 {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} received a null host pointer; register a real host buffer before queueing CUDA DMA."
            ),
        });
    }
    if byte_len == 0 {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} requires a non-zero byte length; keep empty host buffers unregistered."
            ),
        });
    }
    vyre_driver::accounting::checked_add_u64_usize_offset_lazy(
        ptr,
        byte_len,
        || {
            BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} host range length does not fit in a CUDA host address span; split the registration into smaller chunks."
            ),
        }
        },
        || {
            BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} host range wraps the address space; pass a valid ptr..ptr+byte_len range before queueing CUDA DMA."
            ),
        }
        },
    )?;
    Ok(())
}

pub(crate) fn alloc_pinned_host_buffer(
    byte_len: usize,
    label: &'static str,
) -> Result<*mut c_void, BackendError> {
    if byte_len == 0 {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} requires a non-zero byte length; keep empty pinned-host transfers as null sentinels."
            ),
        });
    }
    let mut ptr = std::ptr::null_mut::<c_void>();
    // SAFETY: ptr is a valid out-pointer and byte_len is non-zero. The
    // PORTABLE flag keeps the allocation addressable by all CUDA contexts.
    unsafe {
        cuda_check(
            cudarc::driver::sys::cuMemHostAlloc(&mut ptr, byte_len, CU_MEMHOSTALLOC_PORTABLE),
            label,
        )?;
    }
    if ptr.is_null() {
        return Err(BackendError::DispatchFailed {
            code: None,
            message: format!(
                "{label} returned a null pinned-host pointer after reporting success. Fix: update the CUDA driver or lower pinned-host transfer pressure."
            ),
        });
    }
    Ok(ptr)
}

pub(crate) fn free_pinned_host_buffer(ptr: *mut c_void, label: &'static str) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: ptr was returned by cuMemHostAlloc and is owned by the caller.
    // This runs from Drop/pool cleanup paths, so failures are logged.
    unsafe {
        let result = cudarc::driver::sys::cuMemFreeHost(ptr);
        if result != CUresult::CUDA_SUCCESS {
            tracing::error!(
                "Fix: {label} failed while releasing pinned host allocation with {result:?}; ensure all DMA using the allocation has completed."
            );
        }
    }
}

/// Register an existing host range as page-locked memory for CUDA DMA.
///
/// # Safety
///
/// The caller must guarantee that `ptr..ptr+byte_len` is a mapped host range
/// that remains live and uniquely owned until [`unregister_host_buffer`] runs.
pub(crate) unsafe fn register_host_buffer(
    ptr: u64,
    byte_len: usize,
    label: &'static str,
) -> Result<(), BackendError> {
    validate_nonzero_host_range(ptr, byte_len, label)?;
    // SAFETY: The caller owns pointer lifetime and exclusivity. This helper
    // centralizes the CUDA registration flags and result handling.
    unsafe {
        cuda_check(
            cudarc::driver::sys::cuMemHostRegister_v2(
                ptr as *mut c_void,
                byte_len,
                CU_MEMHOSTREGISTER_PORTABLE as ::std::os::raw::c_uint,
            ),
            label,
        )
    }
}

/// Unregister a host range previously registered with [`register_host_buffer`].
///
/// # Safety
///
/// The caller must guarantee that no in-flight CUDA operation still references
/// the host range.
pub(crate) unsafe fn unregister_host_buffer(
    ptr: u64,
    label: &'static str,
) -> Result<(), BackendError> {
    if ptr == 0 {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} received a null host pointer; only unregister host buffers that were successfully registered."
            ),
        });
    }
    // SAFETY: The caller guarantees no in-flight DMA references the host
    // range. CUDA validates the opaque registration and returns CUresult.
    unsafe {
        cuda_check(
            cudarc::driver::sys::cuMemHostUnregister(ptr as *mut c_void),
            label,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{alloc_pinned_host_buffer, register_host_buffer, unregister_host_buffer};

    #[test]
    fn pinned_host_allocation_rejects_empty_before_ffi() {
        let error = alloc_pinned_host_buffer(0, "unit pinned alloc")
            .expect_err("Fix: empty pinned-host allocation must fail before CUDA FFI.");
        assert!(
            error.to_string().contains("non-zero byte length"),
            "empty pinned allocation diagnostic must identify the length bug: {error}"
        );
    }

    #[test]
    fn host_registration_rejects_null_and_empty_before_ffi() {
        // SAFETY: This deliberately passes a null pointer to verify the wrapper
        // rejects it before reaching CUDA FFI.
        let null_error = unsafe {
            register_host_buffer(0, 4096, "unit host register")
                .expect_err("Fix: null host registration must fail before CUDA FFI.")
        };
        assert!(
            null_error.to_string().contains("null host pointer"),
            "null registration diagnostic must identify the pointer bug: {null_error}"
        );

        // SAFETY: This deliberately passes a non-null sentinel with zero length;
        // the wrapper must reject the length before reaching CUDA FFI.
        let empty_error = unsafe {
            register_host_buffer(1, 0, "unit host register")
                .expect_err("Fix: empty host registration must fail before CUDA FFI.")
        };
        assert!(
            empty_error.to_string().contains("non-zero byte length"),
            "empty registration diagnostic must identify the length bug: {empty_error}"
        );

        // SAFETY: This deliberately passes a null pointer to verify unregister
        // validation fails before reaching CUDA FFI.
        let unregister_error = unsafe {
            unregister_host_buffer(0, "unit host unregister")
                .expect_err("Fix: null host unregister must fail before CUDA FFI.")
        };
        assert!(
            unregister_error.to_string().contains("null host pointer"),
            "null unregister diagnostic must identify the pointer bug: {unregister_error}"
        );
    }

    #[test]
    fn host_registration_rejects_wrapping_ranges_before_ffi() {
        // SAFETY: This deliberately passes a wrapping address range; the wrapper
        // validates arithmetic overflow before reaching CUDA FFI.
        let error = unsafe {
            register_host_buffer(u64::MAX, 2, "unit host register")
                .expect_err("Fix: wrapping host registration ranges must fail before CUDA FFI.")
        };

        assert!(
            error.to_string().contains("wraps the address space"),
            "wrapping host registration diagnostic must identify the range bug: {error}"
        );
    }

    #[test]
    fn resident_io_uses_shared_host_registration_boundary() {
        let host_memory = include_str!("host_memory.rs");
        let resident_io = include_str!("resident_io.rs");
        let register_ffi = concat!("cudarc::driver::sys::", "cuMemHostRegister_v2(");
        let unregister_ffi = concat!("cudarc::driver::sys::", "cuMemHostUnregister(");

        assert_eq!(
            host_memory.matches(register_ffi).count(),
            1,
            "Fix: raw cuMemHostRegister_v2 must stay behind host_memory::register_host_buffer."
        );
        assert_eq!(
            host_memory.matches(unregister_ffi).count(),
            1,
            "Fix: raw cuMemHostUnregister must stay behind host_memory::unregister_host_buffer."
        );
        assert_eq!(
            resident_io.matches(register_ffi).count() + resident_io.matches(unregister_ffi).count(),
            0,
            "Fix: resident I/O must use the shared host-memory registration boundary."
        );
        assert!(
            resident_io.contains("host_memory::register_host_buffer")
                && resident_io.contains("host_memory::unregister_host_buffer"),
            "Fix: resident I/O pin/unpin APIs must call the shared host-memory helpers."
        );
    }

    #[test]
    fn allocation_pool_uses_shared_pinned_host_memory_boundary() {
        let host_memory = include_str!("host_memory.rs");
        let allocations = include_str!("allocations.rs");
        let alloc_ffi = concat!("cudarc::driver::sys::", "cuMemHostAlloc(");
        let free_ffi = concat!("cudarc::driver::sys::", "cuMemFreeHost(");

        assert_eq!(
            host_memory.matches(alloc_ffi).count(),
            1,
            "Fix: raw cuMemHostAlloc must stay behind host_memory::alloc_pinned_host_buffer."
        );
        assert_eq!(
            host_memory.matches(free_ffi).count(),
            1,
            "Fix: raw cuMemFreeHost must stay behind host_memory::free_pinned_host_buffer."
        );
        assert_eq!(
            allocations.matches(alloc_ffi).count() + allocations.matches(free_ffi).count(),
            0,
            "Fix: pinned-host allocation pools must use host_memory helpers instead of direct FFI."
        );
        assert!(
            allocations.contains("host_memory::alloc_pinned_host_buffer")
                && allocations.contains("host_memory::free_pinned_host_buffer"),
            "Fix: allocation pool acquire/release/drop paths must route through shared host-memory helpers."
        );
    }
}
