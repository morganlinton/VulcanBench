use super::readback::ReadbackPollBackoff;
use crate::buffer::GpuBufferHandle;
use std::time::Instant;
use vyre_driver::BackendError;
use vyre_emit_naga::program::{TrapTag, TRAP_SIDECAR_WORDS};

pub(super) fn sidecar_flag_set(mapped: &[u8]) -> Result<bool, BackendError> {
    if mapped.len() < 4 {
        return Err(BackendError::new(format!(
            "internal wgpu trap flag readback returned {} bytes but 4 bytes are required. Fix: map the first u32 of the trap sidecar before lazy full-sidecar decode.",
            mapped.len()
        )));
    }
    let mut flag = [0u8; 4];
    flag.copy_from_slice(&mapped[..4]);
    Ok(u32::from_le_bytes(flag) != 0)
}

pub(super) fn map_full_sidecar(
    device: &wgpu::Device,
    readback_buffer: &GpuBufferHandle,
    deadline: Instant,
    trap_tags: &[TrapTag],
) -> Result<Option<BackendError>, BackendError> {
    let buf = readback_buffer.buffer();
    let sidecar_len = u64::from(TRAP_SIDECAR_WORDS) * 4;
    let slice = buf.slice(0..sidecar_len);
    let (sender, receiver) = crossbeam_channel::bounded(1);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        if let Err(error) = sender.send(result) {
            tracing::error!(
                ?error,
                "GPU trap sidecar full-readback callback result was lost because the receiver dropped"
            );
        }
    });
    let map_result = poll_map_result_until(device, &receiver, deadline).map_err(|error| {
        BackendError::new(format!(
            "GPU trap sidecar full readback callback did not complete after bounded polling: {error}. Fix: keep the GPU device alive until trap sidecar decode completes."
        ))
    })?;
    map_result.map_err(|error| {
        BackendError::new(format!(
            "GPU trap sidecar full readback mapping failed: {error:?}. Fix: use MAP_READ and COPY_DST readback buffers."
        ))
    })?;
    let mapped = slice.get_mapped_range();
    let error = sidecar_error_from_mapped(&mapped, trap_tags)?;
    drop(mapped);
    buf.unmap();
    Ok(error)
}

pub(super) fn sidecar_error_from_mapped(
    mapped: &[u8],
    trap_tags: &[TrapTag],
) -> Result<Option<BackendError>, BackendError> {
    Ok(crate::pipeline::trap_error_from_sidecar(mapped, trap_tags))
}

fn poll_map_result_until(
    device: &wgpu::Device,
    receiver: &crossbeam_channel::Receiver<Result<(), wgpu::BufferAsyncError>>,
    deadline: Instant,
) -> Result<Result<(), wgpu::BufferAsyncError>, &'static str> {
    let mut backoff = ReadbackPollBackoff::new();
    loop {
        device
            .poll(wgpu::PollType::Poll)
            .map_err(|_| "device poll failed while waiting for trap-buffer map callback")?;
        match receiver.try_recv() {
            Ok(result) => return Ok(result),
            Err(crossbeam_channel::TryRecvError::Empty) => {}
            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                return Err("callback channel disconnected");
            }
        }
        if Instant::now() >= deadline {
            return Err("deadline expired");
        }
        backoff.idle(deadline);
    }
}
