use super::pool_backend_error;
use crate::buffer::{BufferPool, GpuBufferHandle};
use crate::numeric::rounded_f64_to_u64;
use crossbeam_channel::Receiver;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;
use vyre_driver::BackendError;

type MapResult = Result<(), wgpu::BufferAsyncError>;

const TIMESTAMP_QUERY_COUNT: u32 = 4;
const TIMESTAMP_READBACK_BYTES: u64 = 32;

pub(crate) struct TimestampRecorder {
    pub(crate) query_set: wgpu::QuerySet,
    resolve_buffer: GpuBufferHandle,
    readback_buffer: GpuBufferHandle,
    host_upload_us: u64,
    timestamp_period_ns: f32,
}

pub(crate) struct PendingTimestampProfile {
    readback_buffer: GpuBufferHandle,
    pub(super) receiver: Receiver<MapResult>,
    pub(super) ready: Arc<AtomicBool>,
    host_upload_us: u64,
    timestamp_period_ns: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TimestampProfile {
    pub(crate) dispatch_ns: u64,
    pub(super) copy_ns: u64,
    pub(super) gpu_total_ns: u64,
}

impl TimestampRecorder {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pool: &BufferPool,
        requested: bool,
        host_upload_us: u64,
    ) -> Result<Option<Self>, BackendError> {
        if !requested {
            return Ok(None);
        }
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY)
            || !device
                .features()
                .contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS)
        {
            return Err(BackendError::new(
                "GPU timestamp profiling was requested but TIMESTAMP_QUERY and TIMESTAMP_QUERY_INSIDE_ENCODERS are not both enabled on this wgpu device. Fix: inspect adapter feature negotiation and driver support; do not silently profile with host-only timing.",
            ));
        }

        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("vyre dispatch timestamp queries"),
            ty: wgpu::QueryType::Timestamp,
            count: TIMESTAMP_QUERY_COUNT,
        });
        let resolve_buffer = pool
            .acquire(
                TIMESTAMP_READBACK_BYTES,
                wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            )
            .map_err(pool_backend_error)?;
        let readback_buffer = pool
            .acquire(
                TIMESTAMP_READBACK_BYTES,
                wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            )
            .map_err(pool_backend_error)?;

        Ok(Some(Self {
            query_set,
            resolve_buffer,
            readback_buffer,
            host_upload_us,
            timestamp_period_ns: queue.get_timestamp_period(),
        }))
    }

    pub(crate) fn resolve(&self, encoder: &mut wgpu::CommandEncoder) -> Result<(), BackendError> {
        encoder.resolve_query_set(
            &self.query_set,
            0..TIMESTAMP_QUERY_COUNT,
            self.resolve_buffer.buffer(),
            0,
        );
        encoder.copy_buffer_to_buffer(
            self.resolve_buffer.buffer(),
            0,
            self.readback_buffer.buffer(),
            0,
            TIMESTAMP_READBACK_BYTES,
        );
        Ok(())
    }

    pub(crate) fn map_async(self) -> Result<PendingTimestampProfile, BackendError> {
        let buf = self.readback_buffer.buffer();
        let slice = buf.slice(0..TIMESTAMP_READBACK_BYTES);
        let (sender, receiver) = crossbeam_channel::bounded(1);
        let ready = Arc::new(AtomicBool::new(false));
        let ready_cb = Arc::clone(&ready);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            if let Err(error) = sender.send(result) {
                tracing::error!(
                    ?error,
                    "GPU timestamp callback result was lost because the receiver dropped"
                );
            }
            ready_cb.store(true, Ordering::Release);
        });
        Ok(PendingTimestampProfile {
            readback_buffer: self.readback_buffer,
            receiver,
            ready,
            host_upload_us: self.host_upload_us,
            timestamp_period_ns: self.timestamp_period_ns,
        })
    }
}

pub(crate) fn collect_timestamp_profile(
    profile: Option<PendingTimestampProfile>,
    deadline: Instant,
) -> Result<Option<TimestampProfile>, BackendError> {
    let Some(profile) = profile else {
        return Ok(None);
    };
    let now = Instant::now();
    if now >= deadline {
        return Err(BackendError::new(
            "GPU timestamp profile readback did not complete within 30s after submission wait. Fix: inspect timestamp query resolve and readback buffer lifetimes.",
        ));
    }
    let map_result = profile.receiver.recv_timeout(deadline - now).map_err(|error| {
        BackendError::new(format!(
            "GPU timestamp profile callback did not complete after submission wait: {error}. Fix: keep the timestamp readback buffer alive until profiling finishes."
        ))
    })?;
    map_result.map_err(|error| {
        BackendError::new(format!(
            "GPU timestamp profile mapping failed: {error:?}. Fix: use QUERY_RESOLVE plus MAP_READ-compatible readback buffers."
        ))
    })?;

    let buf = profile.readback_buffer.buffer();
    let slice = buf.slice(0..TIMESTAMP_READBACK_BYTES);
    let mapped = slice.get_mapped_range();
    if mapped.len() < TIMESTAMP_READBACK_BYTES as usize {
        let len = mapped.len();
        drop(mapped);
        buf.unmap();
        return Err(BackendError::new(format!(
            "GPU timestamp profile returned {len} bytes, expected {TIMESTAMP_READBACK_BYTES}. Fix: keep timestamp query count and readback buffer size synchronized."
        )));
    }
    let mut ticks = [0u64; TIMESTAMP_QUERY_COUNT as usize];
    for (index, chunk) in mapped
        .chunks_exact(std::mem::size_of::<u64>())
        .take(TIMESTAMP_QUERY_COUNT as usize)
        .enumerate()
    {
        let mut raw = [0u8; 8];
        raw.copy_from_slice(chunk);
        ticks[index] = u64::from_le_bytes(raw);
    }
    drop(mapped);
    buf.unmap();

    let ns = |delta: u64| -> Result<u64, BackendError> {
        rounded_f64_to_u64(
            (delta as f64) * f64::from(profile.timestamp_period_ns),
            "GPU timestamp delta nanoseconds",
        )
    };
    let dispatch_ns = ns(timestamp_delta(
        ticks[1],
        ticks[0],
        "dispatch timestamp delta",
    )?)?;
    let copy_ns = ns(timestamp_delta(ticks[3], ticks[2], "copy timestamp delta")?)?;
    let gpu_total_ns = ns(timestamp_delta(
        ticks[3],
        ticks[0],
        "total GPU timestamp delta",
    )?)?;
    tracing::info!(
        target: "vyre.wgpu.timestamps",
        host_upload_us = profile.host_upload_us,
        dispatch_ns,
        copy_ns,
        gpu_total_ns,
        timestamp_period_ns = profile.timestamp_period_ns,
        "wgpu dispatch timestamp profile"
    );
    Ok(Some(TimestampProfile {
        dispatch_ns,
        copy_ns,
        gpu_total_ns,
    }))
}

fn timestamp_delta(end: u64, start: u64, label: &'static str) -> Result<u64, BackendError> {
    end.checked_sub(start).ok_or_else(|| {
        BackendError::new(format!(
            "{label} underflowed because the end timestamp {end} is earlier than start timestamp {start}. Fix: verify query write order and timestamp resolve layout.",
        ))
    })
}
