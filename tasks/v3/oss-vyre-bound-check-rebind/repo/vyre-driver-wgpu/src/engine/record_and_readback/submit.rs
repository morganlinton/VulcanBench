use super::readback::{PendingMap, WgpuPendingReadback};
use super::RecordedDispatch;
use crate::allocation::{reserve_smallvec_to_capacity, reserve_vec_to_capacity};
use smallvec::SmallVec;
use std::sync::Arc;
use vyre_driver::BackendError;

pub(crate) fn submit_recorded_dispatch(
    mut recorded: RecordedDispatch,
) -> Result<WgpuPendingReadback, BackendError> {
    let (device, queue) = &*recorded.device_queue;
    device.push_error_scope(wgpu::ErrorFilter::Validation);
    let command_buffer = recorded.command_buffer.take().ok_or_else(|| {
        BackendError::new(
            "recorded dispatch was submitted twice. Fix: keep RecordedDispatch ownership linear.",
        )
    })?;
    let _submission = queue.submit(std::iter::once(command_buffer));
    crate::runtime::device::poll_device_once(device)?;
    if let Some(error) = crate::runtime::device::pop_error_scope_now(device).map_err(|message| {
        BackendError::DispatchFailed {
            code: None,
            message: format!(
                "wgpu queue-submit validation did not complete without a host wait: {message}"
            ),
        }
    })? {
        return Err(BackendError::DispatchFailed {
            code: None,
            message: format!(
                "wgpu rejected queue submission: {error}. Fix: verify command-buffer resource lifetimes, dispatch dimensions, and copy ranges before submitting."
            ),
        });
    }
    pending_after_submission(recorded)
}

pub(crate) fn submit_recorded_batch(
    mut recorded: Vec<RecordedDispatch>,
) -> Result<Vec<WgpuPendingReadback>, BackendError> {
    let Some(first) = recorded.first() else {
        return Ok(Vec::new());
    };
    let device_queue = Arc::clone(&first.device_queue);
    for item in &recorded {
        if !Arc::ptr_eq(&device_queue, &item.device_queue) {
            return Err(BackendError::new(
                "batched wgpu submit received command buffers from multiple device queues. Fix: group batch jobs by backend/device before submission.",
            ));
        }
    }
    let (device, queue) = &*device_queue;
    device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut command_buffers = SmallVec::<[wgpu::CommandBuffer; 8]>::new();
    reserve_smallvec_to_capacity(
        &mut command_buffers,
        recorded.len(),
        "batched wgpu submit",
        "command buffer slot",
        "split the recorded dispatch batch before queue submission",
    )?;
    for item in &mut recorded {
        command_buffers.push(item.command_buffer.take().ok_or_else(|| {
            BackendError::new(
                "recorded dispatch batch contained a previously submitted command buffer. Fix: keep RecordedDispatch ownership linear.",
            )
        })?);
    }
    let _submission = queue.submit(command_buffers);
    crate::runtime::device::poll_device_once(device)?;
    if let Some(error) = crate::runtime::device::pop_error_scope_now(device).map_err(|message| {
        BackendError::DispatchFailed {
            code: None,
            message: format!("wgpu batched queue-submit validation did not complete without a host wait: {message}"),
        }
    })? {
        return Err(BackendError::DispatchFailed {
            code: None,
            message: format!(
                "wgpu rejected batched queue submission: {error}. Fix: verify every command buffer in the batch uses the same live device and valid copy ranges."
            ),
        });
    }
    let mut pending = Vec::new();
    reserve_vec_to_capacity(
        &mut pending,
        recorded.len(),
        "batched wgpu submit",
        "pending readback slot",
        "split the recorded dispatch batch before collecting readbacks",
    )?;
    for item in recorded {
        pending.push(pending_after_submission(item)?);
    }
    Ok(pending)
}

fn pending_after_submission(
    recorded: RecordedDispatch,
) -> Result<WgpuPendingReadback, BackendError> {
    let mut pending = smallvec::SmallVec::<[PendingMap; 4]>::new();
    reserve_smallvec_to_capacity(
        &mut pending,
        recorded.readback_buffers.len(),
        "wgpu submit",
        "pending map slot",
        "split the dispatch output set before submission",
    )?;
    for (output, readback) in recorded.readback_buffers {
        pending.push((output, readback.map_async()?));
    }
    let timestamp_profile = if let Some(recorder) = recorded.timestamp_recorder {
        Some(recorder.map_async()?)
    } else {
        None
    };

    let mut outputs = Vec::new();
    reserve_vec_to_capacity(
        &mut outputs,
        recorded.output_count,
        "wgpu submit",
        "pending output slot",
        "split the dispatch output set before submission",
    )?;

    Ok(WgpuPendingReadback {
        device_queue: recorded.device_queue,
        pending,
        outputs,
        output_count: recorded.output_count,
        output_bindings: recorded.output_bindings,
        trap_tags: recorded.trap_tags,
        timestamp_profile,
    })
}
