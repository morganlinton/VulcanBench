//! WGPU concrete `DeviceBuffer` impl wrapping the existing
//! `GpuBufferHandle`. Lets consumers allocate one persistent
//! `Box<dyn DeviceBuffer>`, upload host bytes once, dispatch many
//! times reusing the same device-resident allocation, and download
//! when the loop ends  -  paying host↔device copy cost only at the
//! boundary instead of per-dispatch.

use crate::buffer::{write_padded, GpuBufferHandle};
use crate::WgpuBackend;
use vyre_driver::{BackendError, DeviceBuffer};

/// Backend id string registered for `WgpuDeviceBuffer`. Matches
/// `WgpuBackend::id()`.
pub const WGPU_BACKEND_ID: &str = "wgpu";

/// Concrete `DeviceBuffer` impl over a `GpuBufferHandle`.
///
/// Constructed via `WgpuBackend::allocate_device_buffer`. Public so
/// downstream code can `downcast_ref::<WgpuDeviceBuffer>()` and
/// reach the underlying `wgpu::Buffer` for advanced use; opaque to
/// callers that only hold `Box<dyn DeviceBuffer>`.
#[derive(Debug)]
pub struct WgpuDeviceBuffer {
    backend_id: &'static str,
    handle: GpuBufferHandle,
    logical_byte_len: usize,
    label: Option<String>,
}

impl WgpuDeviceBuffer {
    /// Borrow the underlying handle for advanced wgpu work (custom
    /// bind groups, manual readback, etc.). Most callers should
    /// stick to the `DeviceBuffer` API.
    #[must_use]
    pub fn handle(&self) -> &GpuBufferHandle {
        &self.handle
    }
}

impl DeviceBuffer for WgpuDeviceBuffer {
    fn backend_id(&self) -> &'static str {
        self.backend_id
    }

    fn byte_len(&self) -> usize {
        self.logical_byte_len
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

impl WgpuBackend {
    /// Allocate a new GPU-resident buffer of `byte_len` bytes. The
    /// buffer is created with STORAGE | COPY_SRC | COPY_DST so it can
    /// participate in dispatch as either input or output and round-
    /// trip through `upload_device_buffer` / `download_device_buffer`.
    ///
    /// # Errors
    /// Returns a backend error if the underlying wgpu allocation
    /// fails (e.g. byte_len exceeds device limits).
    pub fn allocate_wgpu_device_buffer(
        &self,
        byte_len: usize,
    ) -> Result<Box<dyn DeviceBuffer>, BackendError> {
        let device_queue = self.current_device_queue();
        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let len = u64::try_from(byte_len).map_err(|_| {
            BackendError::new(format!(
                "Fix: WgpuBackend::allocate_device_buffer received byte_len {byte_len} that does not fit in u64."
            ))
        })?;
        let handle = GpuBufferHandle::alloc(&device_queue.0, len, usage)?;
        Ok(Box::new(WgpuDeviceBuffer {
            backend_id: WGPU_BACKEND_ID,
            handle,
            logical_byte_len: byte_len,
            label: None,
        }))
    }

    /// Upload `bytes` into a previously-allocated wgpu DeviceBuffer.
    /// Bytes shorter than the allocation are written at offset 0; the
    /// remainder of the buffer is left as-is. Bytes longer than the
    /// allocation are an error.
    ///
    /// # Errors
    /// Returns a backend error when the buffer was not allocated by
    /// this backend, when `bytes` exceeds the buffer's allocation, or
    /// when the wgpu queue write fails.
    pub fn upload_wgpu_device_buffer(
        &self,
        buffer: &mut dyn DeviceBuffer,
        bytes: &[u8],
    ) -> Result<(), BackendError> {
        let backend_id = buffer.backend_id().to_string();
        let wgpu_buf = buffer
            .as_any_mut()
            .downcast_mut::<WgpuDeviceBuffer>()
            .ok_or_else(|| {
                BackendError::new(format!(
                    "Fix: upload_device_buffer expected a WgpuDeviceBuffer (allocated by `wgpu` backend) but got buffer owned by `{backend_id}`."
                ))
            })?;
        let byte_len = u64::try_from(bytes.len()).map_err(|source| {
            BackendError::new(format!(
                "Fix: upload_device_buffer byte length cannot fit u64: {source}. Shard the upload before writing to a WGPU buffer."
            ))
        })?;
        if byte_len > wgpu_buf.handle.byte_len() {
            return Err(BackendError::new(format!(
                "Fix: upload_device_buffer received {} bytes, exceeds logical length {} bytes for WgpuDeviceBuffer.",
                bytes.len(),
                wgpu_buf.handle.byte_len()
            )));
        }
        let device_queue = self.current_device_queue();
        write_padded(
            &device_queue.1,
            wgpu_buf.handle.buffer(),
            bytes,
            wgpu_buf.handle.allocation_len(),
        )
    }

    /// Download the full byte_len of a previously-allocated wgpu
    /// DeviceBuffer into a fresh `Vec<u8>`.
    ///
    /// # Errors
    /// Returns a backend error when the buffer was not allocated by
    /// this backend or when the readback fails (typically: buffer
    /// missing COPY_SRC, which the standard allocator path includes).
    pub fn download_wgpu_device_buffer(
        &self,
        buffer: &dyn DeviceBuffer,
    ) -> Result<Vec<u8>, BackendError> {
        let wgpu_buf = buffer
            .as_any()
            .downcast_ref::<WgpuDeviceBuffer>()
            .ok_or_else(|| {
                BackendError::new(format!(
                    "Fix: download_device_buffer expected a WgpuDeviceBuffer (allocated by `wgpu` backend) but got buffer owned by `{}`.",
                    buffer.backend_id()
                ))
        })?;
        let device_queue = self.current_device_queue();
        let mut out = Vec::new();
        wgpu_buf
            .handle
            .readback(&device_queue.0, &device_queue.1, &mut out)?;
        Ok(out)
    }

    /// Free a previously-allocated wgpu DeviceBuffer. The wgpu
    /// allocation is released when the underlying Arc<wgpu::Buffer>
    /// reaches zero references  -  dropping the box here is sufficient.
    ///
    /// # Errors
    /// Returns a backend error when the buffer was not allocated by
    /// this backend.
    pub fn free_wgpu_device_buffer(
        &self,
        buffer: Box<dyn DeviceBuffer>,
    ) -> Result<(), BackendError> {
        let backend_id = buffer.backend_id().to_string();
        if buffer.as_any().downcast_ref::<WgpuDeviceBuffer>().is_none() {
            return Err(BackendError::new(format!(
                "Fix: free_device_buffer expected a WgpuDeviceBuffer but got buffer owned by `{backend_id}`."
            )));
        }
        drop(buffer);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn required_backend() -> WgpuBackend {
        WgpuBackend::new().unwrap_or_else(|error| {
            panic!(
                "Fix: WGPU device-buffer tests require a working GPU adapter on this fleet; repair adapter probing/driver configuration instead of silently skipping: {error}"
            )
        })
    }

    #[test]
    fn allocate_round_trip_when_gpu_present() {
        let backend = required_backend();
        let mut buffer = backend.allocate_wgpu_device_buffer(64).expect(
            "Fix: GPU-resident allocation should succeed for 64 bytes on a healthy adapter",
        );
        assert_eq!(buffer.backend_id(), WGPU_BACKEND_ID);
        assert!(buffer.byte_len() >= 64);
        let payload: Vec<u8> = (0..64).collect();
        backend
            .upload_wgpu_device_buffer(buffer.as_mut(), &payload)
            .expect("Fix: upload must succeed for 64 bytes within allocation.");
        let read = backend
            .download_wgpu_device_buffer(buffer.as_ref())
            .expect("Fix: download must succeed after upload on a freshly-allocated buffer.");
        assert!(read.starts_with(&payload), "round-trip must preserve bytes");
        backend
            .free_wgpu_device_buffer(buffer)
            .expect("Fix: free must accept a buffer the same backend allocated.");
    }

    #[test]
    fn upload_rejects_oversize() {
        let backend = required_backend();
        let mut buffer = backend
            .allocate_wgpu_device_buffer(16)
            .expect("Fix: 16-byte allocation must succeed.");
        let too_big = vec![0u8; 4096];
        let err = backend
            .upload_wgpu_device_buffer(buffer.as_mut(), &too_big)
            .expect_err("Fix: oversize upload must error, not silently truncate.");
        let msg = format!("{err}");
        assert!(
            msg.contains("exceeds logical length"),
            "error must explain the size mismatch, got: {msg}"
        );
    }

    #[test]
    fn cross_backend_buffer_rejected() {
        let backend = required_backend();
        let mut alien = vyre_driver::HostShimBuffer::allocate("not-wgpu", 32);
        let err = backend
            .upload_wgpu_device_buffer(alien.as_mut(), &[1u8; 4])
            .expect_err("Fix: WgpuBackend must reject buffers owned by other backends.");
        let msg = format!("{err}");
        assert!(
            msg.contains("not-wgpu"),
            "error must name the offending backend id, got: {msg}"
        );
    }

    #[test]
    fn upload_rejects_padding_overwrite_beyond_logical_length() {
        let backend = required_backend();
        let mut buffer = backend
            .allocate_wgpu_device_buffer(17)
            .expect("Fix: 17-byte allocation must succeed.");
        let padding_overwrite = vec![0u8; 20];
        let err = backend
            .upload_wgpu_device_buffer(buffer.as_mut(), &padding_overwrite)
            .expect_err("Fix: WgpuBackend must reject writes past the logical DeviceBuffer length even when the allocation is padded.");
        let msg = format!("{err}");
        assert!(
            msg.contains("logical length 17"),
            "error must explain the logical length boundary, got: {msg}"
        );
    }

    #[test]
    fn device_buffer_source_has_no_release_path_panic_or_padded_upload_boundary() {
        let source = include_str!("device_buffer.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: device buffer production source must precede tests");
        assert!(
            !production.contains(concat!("panic", "!("))
                && !production.contains(".unwrap_or_else(")
                && !production.contains("Vec::with_capacity"),
            "Fix: WGPU DeviceBuffer release path must not panic or preallocate padded readback capacity."
        );
        assert!(
            production.contains("logical_byte_len")
                && production.contains("byte_len > wgpu_buf.handle.byte_len()")
                && production.contains("let mut out = Vec::new();"),
            "Fix: WGPU DeviceBuffer must preserve logical byte length and reject padded overwrite attempts."
        );
    }
}
