//! Backend-owned device-buffer abstraction (SEED-6).
//!
//! Today every `VyreBackend::dispatch` round-trips inputs and outputs as
//! owned `Vec<u8>` buffers  -  uploaded to the device per call, downloaded
//! back per call. For workloads that issue thousands of small dispatches
//! against the same logical buffers (the C parser preprocessor pipeline
//! is the canonical case), the host-device copies dominate wall time.
//!
//! `DeviceBuffer` is the substrate-neutral handle to a backend-owned
//! allocation. Backends that opt in implement
//! [`VyreBackend::allocate_device_buffer`] and
//! [`VyreBackend::dispatch_with_device_buffers`]; consumers that hold a
//! `Box<dyn DeviceBuffer>` can re-bind it across dispatches instead of
//! re-uploading bytes. Backends that don't opt in return
//! [`BackendError::UnsupportedFeature`]; production callers that select
//! this API must treat that as a hard capability miss, not as permission
//! to hide a host-buffer dispatch fallback.

use crate::backend::{BackendError, DispatchConfig};
use vyre_foundation::ir::Program;

/// Opaque handle to a device-resident allocation owned by one backend.
///
/// The handle is `Send + Sync` so callers can park it across awaits and
/// share it across worker threads, but it is NOT portable across
/// backends  -  the backend that allocated it must be the same backend
/// that dispatches it. Cross-backend transfer requires explicit
/// download → re-upload through the substrate-neutral host path.
///
/// `Any` is a supertrait so concrete backends and tests can downcast to
/// their own allocation type without adding substrate-specific methods
/// to this public trait.
pub trait DeviceBuffer: std::any::Any + Send + Sync + std::fmt::Debug {
    /// Stable backend identifier the buffer belongs to. Matches
    /// [`crate::backend::VyreBackend::id`] of the allocating backend.
    fn backend_id(&self) -> &'static str;

    /// Size of the allocation in bytes. The kernel sees this as the
    /// declared `BufferDecl::count * element_size`.
    fn byte_len(&self) -> usize;

    /// Optional human-readable label (debug surface only). Backends may
    /// return `None` when no label was set.
    fn debug_label(&self) -> Option<&str> {
        None
    }

    /// Erase to `&dyn Any` so callers can downcast without naming the
    /// concrete buffer type. Implementors return `self`.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Mutable variant of [`Self::as_any`]. Implementors return `self`.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Marker returned by backends that have not opted in to
/// [`DeviceBuffer`] yet. The default
/// [`crate::backend::VyreBackend::allocate_device_buffer`] returns this
/// variant via [`BackendError::UnsupportedFeature`] so the consumer
/// path is the same shape across all backends  -  opt-in detection is one
/// `Result::is_err` check, no separate trait.
pub const DEVICE_BUFFER_FEATURE: &str = "DeviceBuffer";

/// Convenience helper for default `VyreBackend::allocate_device_buffer`
/// impls  -  every shipped backend returns this variant until they
/// implement persistent device-buffer allocation.
pub(crate) fn unsupported_device_buffer(backend_id: &'static str) -> BackendError {
    BackendError::UnsupportedFeature {
        name: DEVICE_BUFFER_FEATURE.to_string(),
        backend: backend_id.to_string(),
    }
}

/// Implementor of [`DeviceBuffer`] for compatibility tests and explicit
/// host-resident fixtures  -  stores raw bytes on the host, identifies as
/// the requesting backend.
///
/// This is not a production substitute for real device allocation. Real
/// device backends override [`crate::backend::VyreBackend::allocate_device_buffer`]
/// to return their own concrete buffer type wrapped in `Box<dyn DeviceBuffer>`.
#[derive(Debug)]
pub struct HostShimBuffer {
    backend_id: &'static str,
    bytes: Vec<u8>,
    label: Option<String>,
}

impl HostShimBuffer {
    /// Allocate a zero-filled host-resident buffer. The bytes live in
    /// process memory; backends that use this still pay the upload
    /// cost on every dispatch but the consumer-side API is the same as
    /// for true device buffers.
    #[must_use]
    pub fn allocate(backend_id: &'static str, byte_len: usize) -> Box<dyn DeviceBuffer> {
        Box::new(Self {
            backend_id,
            bytes: vec![0; byte_len],
            label: None,
        })
    }

    /// Allocate from existing bytes. The buffer takes ownership.
    #[must_use]
    pub fn from_bytes(backend_id: &'static str, bytes: Vec<u8>) -> Box<dyn DeviceBuffer> {
        Box::new(Self {
            backend_id,
            bytes,
            label: None,
        })
    }

    /// Borrow the underlying bytes. Only `HostShimBuffer` exposes this  -
    /// real device buffers cannot be byte-borrowed without a download.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    /// Mutably borrow the underlying bytes. Only valid on host-shim
    /// buffers; real device buffers panic.
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    /// Attach a debug label after allocation.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = Some(label.into());
    }
}

impl DeviceBuffer for HostShimBuffer {
    fn backend_id(&self) -> &'static str {
        self.backend_id
    }

    fn byte_len(&self) -> usize {
        self.bytes.len()
    }

    fn debug_label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Default `VyreBackend::dispatch_with_device_buffers` implementation
/// shape: validate every input/output buffer belongs to the same
/// backend, then delegate. Real device backends override this to bind
/// their concrete buffer type without the host round-trip.
///
/// Consumers that don't care about backend identity can pass any
/// `&dyn DeviceBuffer` and trust the validation here to surface the
/// mismatch with an actionable error.
///
/// # Errors
///
/// Returns [`BackendError::UnsupportedFeature`] when the buffer's
/// `backend_id` does not match `self_backend_id`, OR when the backend
/// has not opted in to device-buffer dispatch.
pub fn validate_buffer_ownership<'a>(
    self_backend_id: &str,
    buffers: impl IntoIterator<Item = &'a dyn DeviceBuffer>,
) -> Result<(), BackendError> {
    for (idx, buffer) in buffers.into_iter().enumerate() {
        if buffer.backend_id() != self_backend_id {
            return Err(BackendError::UnsupportedFeature {
                name: format!(
                    "DeviceBuffer cross-backend dispatch (buffer {idx} owned by `{}`)",
                    buffer.backend_id()
                ),
                backend: self_backend_id.to_string(),
            });
        }
    }
    Ok(())
}

/// Default `dispatch_with_device_buffers` body. Backends that have not
/// implemented their concrete persistent-buffer path fail loudly after
/// ownership validation.
///
/// Earlier versions mirrored device-buffer dispatch through
/// [`HostShimBuffer`] and regular `dispatch`. That hid host copies behind
/// the resident-buffer API and made performance regressions look like
/// working functionality. Backends with real device buffers MUST override
/// this method.
///
/// # Errors
///
/// Returns [`BackendError::UnsupportedFeature`] when the backend has not
/// provided a real resident-buffer implementation.
pub fn default_dispatch_with_device_buffers(
    backend: &dyn crate::backend::VyreBackend,
    program: &Program,
    inputs: &[&dyn DeviceBuffer],
    outputs: &mut [&mut dyn DeviceBuffer],
    config: &DispatchConfig,
) -> Result<(), BackendError> {
    let _ = (program, config);
    validate_buffer_ownership(backend.id(), inputs.iter().copied())?;
    validate_buffer_ownership(
        backend.id(),
        outputs.iter().map(|b| &**b as &dyn DeviceBuffer),
    )?;
    Err(BackendError::UnsupportedFeature {
        name: "DeviceBuffer dispatch requires a backend-native resident-buffer implementation; host-shim dispatch fallback is forbidden".to_string(),
        backend: backend.id().to_string(),
    })
}

/// Compile-time confirmation that the trait is dyn-safe.
const _ASSERT_DYN_SAFE: Option<&dyn DeviceBuffer> = None;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_shim_buffer_reports_size_and_backend() {
        let buf = HostShimBuffer::allocate("test-backend", 64);
        assert_eq!(buf.backend_id(), "test-backend");
        assert_eq!(buf.byte_len(), 64);
        assert!(buf.debug_label().is_none());
    }

    #[test]
    fn host_shim_buffer_round_trips_bytes() {
        let mut buf = HostShimBuffer::allocate("test-backend", 8);
        let shim = buf
            .as_any_mut()
            .downcast_mut::<HostShimBuffer>()
            .expect("Fix: HostShimBuffer");
        shim.as_mut_slice()
            .copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        let shim_ref = buf
            .as_any()
            .downcast_ref::<HostShimBuffer>()
            .expect("Fix: HostShimBuffer");
        assert_eq!(shim_ref.as_slice(), &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn validate_buffer_ownership_rejects_cross_backend() {
        let cuda_buf = HostShimBuffer::allocate("cuda", 4);
        let wgpu_buf = HostShimBuffer::allocate("wgpu", 4);
        let result =
            validate_buffer_ownership("cuda", [cuda_buf.as_ref(), wgpu_buf.as_ref()].into_iter());
        assert!(matches!(
            result,
            Err(BackendError::UnsupportedFeature { .. })
        ));
    }

    #[test]
    fn validate_buffer_ownership_accepts_same_backend() {
        let a = HostShimBuffer::allocate("cuda", 4);
        let b = HostShimBuffer::allocate("cuda", 8);
        validate_buffer_ownership("cuda", [a.as_ref(), b.as_ref()].into_iter())
            .expect("Fix: same-backend buffers must validate");
    }

    #[test]
    fn unsupported_device_buffer_marks_feature_correctly() {
        let err = unsupported_device_buffer("test-backend");
        match err {
            BackendError::UnsupportedFeature { name, backend } => {
                assert_eq!(name, DEVICE_BUFFER_FEATURE);
                assert_eq!(backend, "test-backend");
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }
}
