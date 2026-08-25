use crate::numeric::usize_to_u64;
use vyre_driver::BackendError;

pub(super) fn pool_backend_error(error: impl std::fmt::Display) -> BackendError {
    BackendError::new(format!(
        "GPU buffer pool acquisition failed: {error}. Fix: restart the process if the pool lock was poisoned, or reduce concurrent dispatch pressure."
    ))
}

pub(super) fn write_padded_input(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    bytes: &[u8],
    size: usize,
) -> Result<Option<(u64, u64)>, BackendError> {
    let zero_start = crate::padded_upload::write_padded_prefix(
        queue,
        buffer,
        bytes,
        "padded input tail offset",
    )?;

    if size > zero_start {
        Ok(Some((
            usize_to_u64(zero_start, "padded input zero-fill start")?,
            usize_to_u64(size - zero_start, "padded input zero-fill length")?,
        )))
    } else {
        Ok(None)
    }
}
