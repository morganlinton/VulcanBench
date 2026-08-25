//! CUDA stream/event ownership and pending-dispatch handles.

use std::ptr::NonNull;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crossbeam_queue::ArrayQueue;
use cudarc::driver::{
    sys::{CUevent, CUevent_flags, CUresult, CUstream, CUstream_flags, CUstream_st},
    CudaContext,
};
use vyre_driver::{backend::private, BackendError, PendingDispatch};

use crate::backend::telemetry::CudaTelemetry;
use crate::backend::{cuda_check, DispatchAllocations, HostTransferAllocations, ResidentUseGuard};

/// RAII owner for a CUDA stream.
#[derive(Debug)]
pub(crate) struct CudaStream {
    raw: CUstream,
}

unsafe impl Send for CudaStream {}
unsafe impl Sync for CudaStream {}

impl CudaStream {
    /// Create a non-blocking CUDA stream.
    pub(crate) fn non_blocking() -> Result<Self, BackendError> {
        let raw = create_non_blocking_raw_stream("cuStreamCreate")?;
        Ok(Self { raw: raw.as_ptr() })
    }

    /// Raw CUDA stream handle.
    #[must_use]
    pub(crate) fn raw(&self) -> CUstream {
        self.raw
    }

    /// Block until stream work has completed.
    pub(crate) fn synchronize(&self) -> Result<(), BackendError> {
        synchronize_raw_stream(self.raw, "cuStreamSynchronize")
    }
}

/// Create a non-blocking raw CUDA stream and reject impossible null-success
/// driver responses before callers can accidentally fall back to stream 0.
pub(crate) fn create_non_blocking_raw_stream(
    label: &'static str,
) -> Result<NonNull<CUstream_st>, BackendError> {
    let mut raw = std::ptr::null_mut();
    // SAFETY: raw is a valid CUDA stream out-pointer; cuda_check converts
    // non-success CUresult values into BackendError.
    unsafe {
        cuda_check(
            cudarc::driver::sys::cuStreamCreate(
                &mut raw,
                CUstream_flags::CU_STREAM_NON_BLOCKING as u32,
            ),
            label,
        )?;
    }
    NonNull::new(raw).ok_or_else(|| BackendError::DispatchFailed {
        code: None,
        message: format!(
            "{label} returned a null stream after reporting success. Fix: update the CUDA driver or disable the CUDA path using this stream."
        ),
    })
}

pub(crate) fn destroy_raw_stream(stream: CUstream, label: &'static str) {
    if stream.is_null() {
        return;
    }
    // SAFETY: stream is a CUDA stream handle owned by the caller; destroy is
    // best-effort because this function is used from Drop paths.
    unsafe {
        let result = cudarc::driver::sys::cuStreamDestroy_v2(stream);
        if result != CUresult::CUDA_SUCCESS {
            tracing::error!(
                "Fix: {label} failed during CUDA stream drop with {result:?}; ensure pending work is synchronized before dropping dispatch resources."
            );
        }
    }
}

/// Query a raw CUDA stream without falling back to CUDA's legacy null-stream
/// semantics.
pub(crate) fn query_raw_stream_ready(
    stream: CUstream,
    label: &'static str,
) -> Result<bool, BackendError> {
    if stream.is_null() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} received a null CUDA stream; use a backend-owned non-blocking stream instead of querying CUDA's legacy default stream."
            ),
        });
    }
    // SAFETY: CUDA validates the opaque stream handle and reports readiness
    // through CUresult.
    let result = unsafe { cudarc::driver::sys::cuStreamQuery(stream) };
    match result {
        CUresult::CUDA_SUCCESS => Ok(true),
        CUresult::CUDA_ERROR_NOT_READY => Ok(false),
        other => cuda_check(other, label).map(|()| true),
    }
}

/// Synchronize a raw CUDA stream without ever falling through to the legacy
/// null-stream global fence.
pub(crate) fn synchronize_raw_stream(
    stream: CUstream,
    label: &'static str,
) -> Result<(), BackendError> {
    if stream.is_null() {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {label} received a null CUDA stream; use a backend-owned non-blocking stream instead of the legacy default stream."
            ),
        });
    }
    // SAFETY: CUDA validates the opaque stream handle and returns a CUresult;
    // `cuda_check` converts non-success into a typed backend error.
    unsafe { cuda_check(cudarc::driver::sys::cuStreamSynchronize(stream), label) }
}

impl Drop for CudaStream {
    fn drop(&mut self) {
        destroy_raw_stream(self.raw, "cuStreamDestroy_v2");
    }
}

/// RAII owner for a CUDA event used as the completion fence.
#[derive(Debug)]
pub(crate) struct CudaEvent {
    raw: CUevent,
}

unsafe impl Send for CudaEvent {}
unsafe impl Sync for CudaEvent {}

impl CudaEvent {
    /// Create a timing-disabled CUDA event.
    pub(crate) fn completion() -> Result<Self, BackendError> {
        let raw = create_raw_event(
            CUevent_flags::CU_EVENT_DISABLE_TIMING as u32,
            "cuEventCreate",
        )?;
        Ok(Self { raw })
    }

    /// Create a CUDA event with timing enabled.
    pub(crate) fn timing() -> Result<Self, BackendError> {
        let raw = create_raw_event(0, "cuEventCreate")?;
        Ok(Self { raw })
    }

    /// Record this event onto a stream.
    pub(crate) fn record(&self, stream: CUstream) -> Result<(), BackendError> {
        if self.raw.is_null() {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: cuEventRecord received a null CUDA event; acquire a backend-owned event before recording completion.".to_string(),
            });
        }
        if stream.is_null() {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: cuEventRecord received a null CUDA stream; record events on a backend-owned non-blocking stream instead of CUDA's legacy default stream.".to_string(),
            });
        }
        // SAFETY: stream / event handles are owned by &self; cuStream*/cuEvent* calls
        // operate on those owned handles and the result is checked via cuda_check.
        unsafe {
            cuda_check(
                cudarc::driver::sys::cuEventRecord(self.raw, stream),
                "cuEventRecord",
            )
        }
    }

    /// Return whether all prior work in the stream has completed.
    pub(crate) fn query_ready(&self) -> Result<bool, BackendError> {
        if self.raw.is_null() {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: cuEventQuery received a null CUDA event; pending dispatches must own a recorded completion event before readiness polling.".to_string(),
            });
        }
        // SAFETY: event handle is owned by &self and non-null. CUDA reports
        // readiness or a typed driver error via CUresult.
        let result = unsafe { cudarc::driver::sys::cuEventQuery(self.raw) };
        match result {
            CUresult::CUDA_SUCCESS => Ok(true),
            CUresult::CUDA_ERROR_NOT_READY => Ok(false),
            other => cuda_check(other, "cuEventQuery").map(|()| true),
        }
    }

    /// Block until the event completes.
    pub(crate) fn synchronize(&self) -> Result<(), BackendError> {
        if self.raw.is_null() {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: cuEventSynchronize received a null CUDA event; pending dispatches must own a recorded completion event before synchronization.".to_string(),
            });
        }
        // SAFETY: stream / event handles are owned by &self; cuStream*/cuEvent* calls
        // operate on those owned handles and the result is checked via cuda_check.
        unsafe {
            cuda_check(
                cudarc::driver::sys::cuEventSynchronize(self.raw),
                "cuEventSynchronize",
            )
        }
    }

    /// Elapsed time between two timing-enabled events, in nanoseconds.
    pub(crate) fn elapsed_time_ns(&self, end: &CudaEvent) -> Result<u64, BackendError> {
        if self.raw.is_null() || end.raw.is_null() {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: cuEventElapsedTime received a null CUDA timing event; record both timing events before reading elapsed time.".to_string(),
            });
        }
        let mut elapsed_ms = 0.0f32;
        // SAFETY: both events are owned, valid CUDA event handles. CUDA returns an
        // error if either event was not recorded or timing was disabled.
        unsafe {
            cuda_check(
                cudarc::driver::sys::cuEventElapsedTime(
                    (&mut elapsed_ms) as *mut f32,
                    self.raw,
                    end.raw,
                ),
                "cuEventElapsedTime",
            )?;
        }
        let elapsed_ns = f64::from(elapsed_ms) * 1_000_000.0;
        if !elapsed_ns.is_finite() || elapsed_ns < 0.0 || elapsed_ns > u64::MAX as f64 {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: CUDA event elapsed time {elapsed_ms} ms cannot fit u64 nanoseconds; inspect CUDA event timing and split the dispatch before telemetry overflows."
                ),
            });
        }
        crate::numeric::CUDA_NUMERIC.rounded_f64_to_u64(elapsed_ns, "event elapsed nanoseconds")
    }
}

impl Drop for CudaEvent {
    fn drop(&mut self) {
        destroy_raw_event(self.raw, "cuEventDestroy_v2");
    }
}

fn create_raw_event(flags: u32, label: &'static str) -> Result<CUevent, BackendError> {
    let mut raw = std::ptr::null_mut();
    // SAFETY: raw is a valid CUDA event out-pointer; cuda_check converts
    // non-success CUresult values into BackendError.
    unsafe {
        cuda_check(cudarc::driver::sys::cuEventCreate(&mut raw, flags), label)?;
    }
    if raw.is_null() {
        return Err(BackendError::DispatchFailed {
            code: None,
            message: format!(
                "{label} returned a null event after reporting success. Fix: update the CUDA driver or disable event-backed CUDA dispatch for this device."
            ),
        });
    }
    Ok(raw)
}

fn destroy_raw_event(event: CUevent, label: &'static str) {
    if event.is_null() {
        return;
    }
    // SAFETY: event is a CUDA event handle owned by the caller; destroy is
    // best-effort because this function is used from Drop paths.
    unsafe {
        let result = cudarc::driver::sys::cuEventDestroy_v2(event);
        if result != CUresult::CUDA_SUCCESS {
            tracing::error!(
                "Fix: {label} failed during CUDA event drop with {result:?}; ensure pending work is synchronized before dropping dispatch resources."
            );
        }
    }
}

/// Cached CUDA launch resources for repeated dispatches.
#[derive(Debug)]
pub(crate) struct CudaLaunchResourcePool {
    streams: ArrayQueue<CudaStream>,
    events: ArrayQueue<CudaEvent>,
    timing_events: ArrayQueue<CudaEvent>,
}

/// Cached CUDA launch-resource counts retained for dispatch reuse.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CudaLaunchResourceCounts {
    /// Cached non-blocking CUDA streams.
    pub streams: usize,
    /// Cached completion-fence CUDA events.
    pub completion_events: usize,
    /// Cached timing-enabled CUDA events used by graph replay telemetry.
    pub timing_events: usize,
}

/// Owned lease for launch resources before they are transferred into a pending dispatch.
#[derive(Debug)]
pub(crate) struct CudaLaunchResourceLease {
    pool: Arc<CudaLaunchResourcePool>,
    stream: Option<CudaStream>,
    timing_events: Option<(CudaEvent, CudaEvent)>,
}

/// Owned lease for a timing-event pair used outside normal launch-resource ownership.
#[derive(Debug)]
pub(crate) struct CudaTimingEventPairLease {
    pool: Arc<CudaLaunchResourcePool>,
    timing_events: Option<(CudaEvent, CudaEvent)>,
    synchronized: bool,
}

impl CudaTimingEventPairLease {
    pub(crate) fn acquire(pool: Arc<CudaLaunchResourcePool>) -> Result<Self, BackendError> {
        let timing_events = pool.acquire_timing_event_pair()?;
        Ok(Self {
            pool,
            timing_events: Some(timing_events),
            synchronized: false,
        })
    }

    pub(crate) fn events(&self) -> Result<&(CudaEvent, CudaEvent), BackendError> {
        self.timing_events
            .as_ref()
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: "Fix: CUDA timing event pair lease was already consumed; acquire a fresh timing lease before recording graph replay events.".to_string(),
            })
    }

    pub(crate) fn mark_synchronized(&mut self) {
        self.synchronized = true;
    }
}

impl Drop for CudaTimingEventPairLease {
    fn drop(&mut self) {
        if let Some((start, end)) = self.timing_events.take() {
            if self.synchronized {
                self.pool.release_timing_event(start);
                self.pool.release_timing_event(end);
            } else {
                tracing::error!(
                    "Fix: leaking CUDA timing event pair lease because completion was not proven before drop."
                );
                std::mem::forget(start);
                std::mem::forget(end);
            }
        }
    }
}

impl CudaLaunchResourceLease {
    pub(crate) fn acquire(
        pool: Arc<CudaLaunchResourcePool>,
        capture_timing: bool,
    ) -> Result<Self, BackendError> {
        let stream = pool.acquire_stream()?;
        let timing_events = if capture_timing {
            match pool.acquire_timing_event_pair() {
                Ok(pair) => Some(pair),
                Err(error) => {
                    pool.release_stream(stream);
                    return Err(error);
                }
            }
        } else {
            None
        };
        Ok(Self {
            pool,
            stream: Some(stream),
            timing_events,
        })
    }

    pub(crate) fn stream_raw(&self) -> Result<CUstream, BackendError> {
        self.stream
            .as_ref()
            .map(CudaStream::raw)
            .ok_or_else(|| BackendError::InvalidProgram {
                fix: "Fix: CUDA launch resource lease stream was already consumed; acquire a fresh launch-resource lease before enqueueing CUDA work.".to_string(),
            })
    }

    pub(crate) fn timing_events(&self) -> Result<Option<&(CudaEvent, CudaEvent)>, BackendError> {
        if self.stream.is_none() {
            return Err(BackendError::InvalidProgram {
                fix: "Fix: CUDA launch resource lease timing events were queried after the stream was consumed; query timing events before transferring the lease into a pending dispatch.".to_string(),
            });
        }
        Ok(self.timing_events.as_ref())
    }

    pub(crate) fn into_parts(
        mut self,
    ) -> Result<(CudaStream, Option<(CudaEvent, CudaEvent)>), BackendError> {
        let stream = self.stream.take().ok_or_else(|| BackendError::InvalidProgram {
            fix: "Fix: CUDA launch resource lease stream was already consumed; pending dispatch ownership cannot be built twice from the same lease.".to_string(),
        })?;
        let timing_events = self.timing_events.take();
        Ok((stream, timing_events))
    }
}

impl Drop for CudaLaunchResourceLease {
    fn drop(&mut self) {
        let Some(stream) = self.stream.take() else {
            if let Some((start, end)) = self.timing_events.take() {
                self.pool.release_timing_event(start);
                self.pool.release_timing_event(end);
            }
            return;
        };
        if let Err(error) = stream.synchronize() {
            tracing::error!(
                "Fix: failed to synchronize CUDA launch resource lease during drop: {error}. In-flight lease resources will not be recycled."
            );
            if let Some((start, end)) = self.timing_events.take() {
                std::mem::forget(start);
                std::mem::forget(end);
            }
            std::mem::forget(stream);
            return;
        }
        if let Some((start, end)) = self.timing_events.take() {
            self.pool.release_timing_event(start);
            self.pool.release_timing_event(end);
        }
        self.pool.release_stream(stream);
    }
}

impl CudaLaunchResourcePool {
    pub(crate) fn new(max_cached: usize) -> Self {
        let max_cached = max_cached.max(1);
        Self {
            streams: ArrayQueue::new(max_cached),
            events: ArrayQueue::new(max_cached),
            timing_events: ArrayQueue::new(max_cached),
        }
    }

    pub(crate) fn acquire_stream(&self) -> Result<CudaStream, BackendError> {
        if let Some(stream) = self.streams.pop() {
            return Ok(stream);
        }
        CudaStream::non_blocking()
    }

    pub(crate) fn acquire_event(&self) -> Result<CudaEvent, BackendError> {
        if let Some(event) = self.events.pop() {
            return Ok(event);
        }
        CudaEvent::completion()
    }

    pub(crate) fn acquire_timing_event(&self) -> Result<CudaEvent, BackendError> {
        if let Some(event) = self.timing_events.pop() {
            return Ok(event);
        }
        CudaEvent::timing()
    }

    pub(crate) fn acquire_timing_event_pair(&self) -> Result<(CudaEvent, CudaEvent), BackendError> {
        let start = self.acquire_timing_event()?;
        match self.acquire_timing_event() {
            Ok(end) => Ok((start, end)),
            Err(error) => {
                self.release_timing_event(start);
                Err(error)
            }
        }
    }

    pub(crate) fn release_stream(&self, stream: CudaStream) {
        if let Err(stream) = self.streams.push(stream) {
            drop(stream);
        }
    }

    pub(crate) fn release_event(&self, event: CudaEvent) {
        if let Err(event) = self.events.push(event) {
            drop(event);
        }
    }

    pub(crate) fn release_timing_event(&self, event: CudaEvent) {
        if let Err(event) = self.timing_events.push(event) {
            drop(event);
        }
    }

    pub(crate) fn cached_counts(&self) -> Result<(usize, usize), BackendError> {
        Ok((self.streams.len(), self.events.len()))
    }

    pub(crate) fn cached_counts_detailed(&self) -> Result<CudaLaunchResourceCounts, BackendError> {
        Ok(CudaLaunchResourceCounts {
            streams: self.streams.len(),
            completion_events: self.events.len(),
            timing_events: self.timing_events.len(),
        })
    }

    pub(crate) fn clear(&self) -> Result<(), BackendError> {
        while self.streams.pop().is_some() {}
        while self.events.pop().is_some() {}
        while self.timing_events.pop().is_some() {}
        Ok(())
    }
}

/// CUDA-backed pending dispatch whose result is fenced by a CUDA event.
#[derive(Debug)]
pub(crate) struct CudaPendingDispatch {
    ctx: Arc<CudaContext>,
    pool: Arc<CudaLaunchResourcePool>,
    event: Option<CudaEvent>,
    stream: Option<CudaStream>,
    allocations: Option<DispatchAllocations>,
    resident_use: Option<ResidentUseGuard>,
    host_transfers: Option<HostTransferAllocations>,
    outputs: Vec<Vec<u8>>,
    timing_start: Option<CudaEvent>,
    timing_end: Option<CudaEvent>,
    ready_device_ns: Option<u64>,
    telemetry: Arc<CudaTelemetry>,
    completed: AtomicBool,
}

impl CudaPendingDispatch {
    /// Build an already-completed pending dispatch.
    pub(crate) fn new_ready(
        ctx: Arc<CudaContext>,
        pool: Arc<CudaLaunchResourcePool>,
        outputs: Vec<Vec<u8>>,
        telemetry: Arc<CudaTelemetry>,
    ) -> Self {
        Self {
            ctx,
            pool,
            event: None,
            stream: None,
            allocations: None,
            resident_use: None,
            host_transfers: None,
            outputs,
            timing_start: None,
            timing_end: None,
            ready_device_ns: None,
            telemetry,
            completed: AtomicBool::new(true),
        }
    }

    /// Build an already-completed pending dispatch with measured device time.
    pub(crate) fn new_ready_timed(
        ctx: Arc<CudaContext>,
        pool: Arc<CudaLaunchResourcePool>,
        outputs: Vec<Vec<u8>>,
        device_ns: Option<u64>,
        telemetry: Arc<CudaTelemetry>,
    ) -> Self {
        Self {
            ctx,
            pool,
            event: None,
            stream: None,
            allocations: None,
            resident_use: None,
            host_transfers: None,
            outputs,
            timing_start: None,
            timing_end: None,
            ready_device_ns: device_ns,
            telemetry,
            completed: AtomicBool::new(true),
        }
    }

    /// Build a pending resident batch dispatch with no host output slots.
    ///
    /// Resident batch readback uses caller-owned resident handles; the pending
    /// dispatch only fences parameter uploads and kernel launches.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_resident_batch_pending(
        ctx: Arc<CudaContext>,
        pool: Arc<CudaLaunchResourcePool>,
        event: CudaEvent,
        stream: CudaStream,
        allocations: DispatchAllocations,
        resident_use: ResidentUseGuard,
        host_transfers: HostTransferAllocations,
        telemetry: Arc<CudaTelemetry>,
    ) -> Self {
        Self::new(
            ctx,
            pool,
            event,
            stream,
            allocations,
            Some(resident_use),
            Some(host_transfers),
            Vec::new(),
            telemetry,
        )
    }

    /// Build a pending dispatch after all GPU work has been enqueued.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        ctx: Arc<CudaContext>,
        pool: Arc<CudaLaunchResourcePool>,
        event: CudaEvent,
        stream: CudaStream,
        allocations: DispatchAllocations,
        resident_use: Option<ResidentUseGuard>,
        host_transfers: Option<HostTransferAllocations>,
        outputs: Vec<Vec<u8>>,
        telemetry: Arc<CudaTelemetry>,
    ) -> Self {
        Self {
            ctx,
            pool,
            event: Some(event),
            stream: Some(stream),
            allocations: Some(allocations),
            resident_use,
            host_transfers,
            outputs,
            timing_start: None,
            timing_end: None,
            ready_device_ns: None,
            telemetry,
            completed: AtomicBool::new(false),
        }
    }

    /// Build a pending dispatch with timing-enabled start/end events.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_timing(
        ctx: Arc<CudaContext>,
        pool: Arc<CudaLaunchResourcePool>,
        event: CudaEvent,
        stream: CudaStream,
        allocations: DispatchAllocations,
        resident_use: Option<ResidentUseGuard>,
        host_transfers: Option<HostTransferAllocations>,
        outputs: Vec<Vec<u8>>,
        timing_start: CudaEvent,
        timing_end: CudaEvent,
        telemetry: Arc<CudaTelemetry>,
    ) -> Self {
        Self {
            ctx,
            pool,
            event: Some(event),
            stream: Some(stream),
            allocations: Some(allocations),
            resident_use,
            host_transfers,
            outputs,
            timing_start: Some(timing_start),
            timing_end: Some(timing_end),
            ready_device_ns: None,
            telemetry,
            completed: AtomicBool::new(false),
        }
    }

    fn bind_context(&self) -> Result<(), BackendError> {
        self.ctx
            .bind_to_thread()
            .map_err(|e| BackendError::DispatchFailed {
                code: None,
                message: format!("CUDA context bind failed: {e}"),
            })
    }

    fn synchronize(&self) -> Result<(), BackendError> {
        if self.completed.load(Ordering::Acquire) {
            return Ok(());
        }
        self.bind_context()?;
        let event = self
            .event
            .as_ref()
            .ok_or_else(|| BackendError::DispatchFailed {
                code: None,
                message: "CUDA pending dispatch completion event was already released".to_string(),
            })?;
        event.synchronize()?;
        self.telemetry.record_sync_point();
        self.completed.store(true, Ordering::Release);
        Ok(())
    }

    fn release_launch_resources(&mut self) {
        if let Some(event) = self.event.take() {
            self.pool.release_event(event);
        }
        if let Some(event) = self.timing_start.take() {
            self.pool.release_timing_event(event);
        }
        if let Some(event) = self.timing_end.take() {
            self.pool.release_timing_event(event);
        }
        if let Some(stream) = self.stream.take() {
            self.pool.release_stream(stream);
        }
    }

    fn force_completion_on_drop(&mut self) -> bool {
        if self.completed.load(Ordering::Acquire) {
            return true;
        }
        if let Err(error) = self.ctx.bind_to_thread() {
            tracing::error!(
                "Fix: failed to bind CUDA context while dropping pending dispatch: {error}. In-flight CUDA resources will not be recycled."
            );
            return false;
        }
        let Some(stream) = self.stream.as_ref() else {
            tracing::error!(
                "Fix: pending CUDA dispatch lost its stream before drop-time synchronization. In-flight CUDA resources will not be recycled."
            );
            return false;
        };
        if let Err(error) = stream.synchronize() {
            tracing::error!(
                "Fix: failed to synchronize CUDA stream while dropping pending dispatch: {error}. In-flight CUDA resources will not be recycled."
            );
            return false;
        }
        self.telemetry.record_sync_point();
        self.completed.store(true, Ordering::Release);
        true
    }

    fn leak_inflight_resources_after_drop_sync_failure(&mut self) {
        tracing::error!(
            "Fix: leaking CUDA pending-dispatch resources because completion could not be proven during drop; await the dispatch result before dropping it."
        );
        std::mem::forget(Arc::clone(&self.ctx));
        if let Some(event) = self.event.take() {
            std::mem::forget(event);
        }
        if let Some(event) = self.timing_start.take() {
            std::mem::forget(event);
        }
        if let Some(event) = self.timing_end.take() {
            std::mem::forget(event);
        }
        if let Some(stream) = self.stream.take() {
            std::mem::forget(stream);
        }
        if let Some(allocations) = self.allocations.take() {
            std::mem::forget(allocations);
        }
        if let Some(resident_use) = self.resident_use.take() {
            std::mem::forget(resident_use);
        }
        if let Some(host_transfers) = self.host_transfers.take() {
            std::mem::forget(host_transfers);
        }
    }

    /// Await completion and return output buffers plus device elapsed time.
    pub(crate) fn await_timed_result(
        mut self,
    ) -> Result<(Vec<Vec<u8>>, Option<u64>), BackendError> {
        self.synchronize()?;
        let device_ns = match self.ready_device_ns.take() {
            Some(device_ns) => Some(device_ns),
            None => match (self.timing_start.as_ref(), self.timing_end.as_ref()) {
                (Some(start), Some(end)) => Some(start.elapsed_time_ns(end)?),
                _ => None,
            },
        };
        self.release_launch_resources();
        self.allocations.take();
        self.resident_use.take();
        let outputs = self.collect_outputs()?;
        self.host_transfers.take();
        Ok((outputs, device_ns))
    }

    fn collect_outputs(&mut self) -> Result<Vec<Vec<u8>>, BackendError> {
        if let Some(transfers) = self.host_transfers.as_ref() {
            let mut outputs = std::mem::take(&mut self.outputs);
            transfers.collect_outputs_into(&mut outputs)?;
            Ok(outputs)
        } else {
            Ok(std::mem::take(&mut self.outputs))
        }
    }

    fn collect_outputs_into(&mut self, outputs: &mut Vec<Vec<u8>>) -> Result<(), BackendError> {
        if let Some(transfers) = self.host_transfers.as_ref() {
            transfers.collect_outputs_into(outputs)?;
        } else {
            vyre_driver::replace_output_buffers_preserving_slots(
                std::mem::take(&mut self.outputs),
                outputs,
            );
        }
        Ok(())
    }
}

impl private::Sealed for CudaPendingDispatch {}

impl PendingDispatch for CudaPendingDispatch {
    fn is_ready(&self) -> bool {
        if self.completed.load(Ordering::Acquire) {
            return true;
        }
        if self.bind_context().is_err() {
            return false;
        }
        let Some(event) = self.event.as_ref() else {
            return true;
        };
        let ready = match event.query_ready() {
            Ok(ready) => ready,
            Err(error) => {
                tracing::error!(
                    "Fix: CUDA pending dispatch readiness query failed: {error}. Await the dispatch to surface synchronization failure details."
                );
                false
            }
        };
        if ready {
            self.completed.store(true, Ordering::Release);
        }
        ready
    }

    fn await_result(mut self: Box<Self>) -> Result<Vec<Vec<u8>>, BackendError> {
        self.synchronize()?;
        self.release_launch_resources();
        self.allocations.take();
        self.resident_use.take();
        let outputs = self.collect_outputs()?;
        self.host_transfers.take();
        Ok(outputs)
    }

    fn await_result_into(
        mut self: Box<Self>,
        outputs: &mut Vec<Vec<u8>>,
    ) -> Result<(), BackendError> {
        self.synchronize()?;
        self.release_launch_resources();
        self.allocations.take();
        self.resident_use.take();
        self.collect_outputs_into(outputs)?;
        self.host_transfers.take();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{query_raw_stream_ready, synchronize_raw_stream, CudaLaunchResourcePool};

    #[test]
    fn launch_resource_leases_do_not_panic_on_consumed_state() {
        let source = include_str!("stream.rs");
        assert!(
            !source.contains(concat!(".expect", "(\"Fix: CUDA launch resource lease stream was already consumed")),
            "Fix: CUDA launch resource leases must return typed backend errors when consumed twice, not panic."
        );
        assert!(
            !source.contains(concat!(".expect", "(\"Fix: CUDA timing event pair lease was already consumed")),
            "Fix: CUDA graph replay timing leases must return typed backend errors when consumed twice, not panic."
        );
    }

    #[test]
    fn launch_resource_counts_include_timing_events() {
        let pool = CudaLaunchResourcePool::new(8);
        let counts = pool
            .cached_counts_detailed()
            .expect("Fix: empty launch resource pool counts should be readable");

        assert_eq!(counts.streams, 0);
        assert_eq!(counts.completion_events, 0);
        assert_eq!(counts.timing_events, 0);

        let source = include_str!("stream.rs");
        assert!(
            source.contains("pub struct CudaLaunchResourceCounts")
                && source.contains("pub timing_events: usize")
                && source.contains("cached_counts_detailed"),
            "Fix: CUDA launch-resource telemetry must expose timing-event cache pressure, not just streams and completion events."
        );
    }

    #[test]
    fn timing_event_pair_lease_requires_completion_before_reuse() {
        let source = include_str!("stream.rs");
        let lease_impl = source
            .split("impl CudaTimingEventPairLease")
            .nth(1)
            .expect("Fix: CUDA timing event pair lease implementation must exist.")
            .split("impl Drop for CudaTimingEventPairLease")
            .next()
            .expect("Fix: timing event pair lease methods must precede Drop.");
        assert!(
            source.contains("synchronized: bool")
                && source.contains("synchronized: false")
                && lease_impl.contains("pub(crate) fn mark_synchronized(&mut self)")
                && lease_impl.contains("self.synchronized = true;"),
            "Fix: CUDA timing event pair leases must default to unproven completion and require an explicit synchronized mark before reuse."
        );

        let drop_impl = source
            .split("impl Drop for CudaTimingEventPairLease")
            .nth(1)
            .expect("Fix: CUDA timing event pair lease must own Drop.")
            .split("impl CudaLaunchResourceLease")
            .next()
            .expect("Fix: timing event pair Drop must precede launch-resource leases.");
        assert!(
            drop_impl.contains("if self.synchronized")
                && drop_impl.contains("self.pool.release_timing_event(start);")
                && drop_impl.contains("self.pool.release_timing_event(end);")
                && drop_impl.contains("completion was not proven before drop")
                && drop_impl.contains("std::mem::forget(start);")
                && drop_impl.contains("std::mem::forget(end);"),
            "Fix: CUDA timing event pair lease Drop must leak unsynchronized events instead of returning them to the pool."
        );
    }

    #[test]
    fn launch_resource_lease_drop_synchronizes_before_recycling_resources() {
        let source = include_str!("stream.rs");
        let drop_impl = source
            .split("impl Drop for CudaLaunchResourceLease")
            .nth(1)
            .expect("Fix: CUDA launch-resource lease must own a Drop implementation.")
            .split("impl CudaLaunchResourcePool")
            .next()
            .expect(
                "Fix: launch-resource lease Drop must precede the resource pool implementation.",
            );
        let sync_pos = drop_impl.find("stream.synchronize()").expect(
            "Fix: CUDA launch-resource lease Drop must synchronize before recycling a stream.",
        );
        let post_sync_drop = &drop_impl[sync_pos..];
        let release_timing_pos = sync_pos
            + post_sync_drop
            .find("self.pool.release_timing_event(start);")
            .expect("Fix: CUDA launch-resource lease Drop must release timing events after successful synchronization.");
        let release_stream_pos = sync_pos
            + post_sync_drop
            .find("self.pool.release_stream(stream);")
            .expect("Fix: CUDA launch-resource lease Drop must release streams after successful synchronization.");

        assert!(
            sync_pos < release_timing_pos && release_timing_pos < release_stream_pos,
            "Fix: CUDA launch-resource lease Drop must prove stream completion before timing-event or stream reuse."
        );
        assert!(
            drop_impl.contains("Err(error)")
                && drop_impl.contains("In-flight lease resources will not be recycled.")
                && drop_impl.contains("std::mem::forget(start);")
                && drop_impl.contains("std::mem::forget(end);")
                && drop_impl.contains("std::mem::forget(stream);")
                && !drop_impl.contains("self.pool.release_stream(stream);\n        if let Err"),
            "Fix: CUDA launch-resource lease Drop must leak resources instead of pooling them when drop-time synchronization fails."
        );
    }

    #[test]
    fn raw_stream_sync_rejects_null_default_stream() {
        let err = synchronize_raw_stream(std::ptr::null_mut(), "unit sync")
            .expect_err("Fix: raw stream sync must reject the legacy null stream");
        assert!(
            err.to_string().contains("null CUDA stream"),
            "raw sync diagnostic must explain the default-stream hazard: {err}"
        );
    }

    #[test]
    fn raw_stream_query_rejects_null_default_stream() {
        let err = query_raw_stream_ready(std::ptr::null_mut(), "unit query")
            .expect_err("Fix: raw stream query must reject the legacy null stream");
        assert!(
            err.to_string().contains("null CUDA stream"),
            "raw query diagnostic must explain the default-stream hazard: {err}"
        );
    }

    #[test]
    fn event_record_rejects_null_event_before_ffi() {
        let event = super::CudaEvent {
            raw: std::ptr::null_mut(),
        };
        let err = event
            .record(std::ptr::null_mut())
            .expect_err("Fix: event recording must reject invalid event handles before FFI");
        assert!(
            err.to_string().contains("null CUDA event"),
            "event record diagnostic must explain the null-event hazard: {err}"
        );
    }

    #[test]
    fn event_record_rejects_null_default_stream_before_ffi() {
        let event = std::mem::ManuallyDrop::new(super::CudaEvent {
            raw: std::ptr::NonNull::<cudarc::driver::sys::CUevent_st>::dangling().as_ptr(),
        });
        let err = event
            .record(std::ptr::null_mut())
            .expect_err("Fix: event recording must reject CUDA's legacy null stream before FFI");
        assert!(
            err.to_string().contains("null CUDA stream"),
            "event record diagnostic must explain the default-stream hazard: {err}"
        );
    }

    #[test]
    fn event_query_and_sync_reject_null_event_before_ffi() {
        let event = super::CudaEvent {
            raw: std::ptr::null_mut(),
        };
        let query_err = event
            .query_ready()
            .expect_err("Fix: event readiness query must reject null events before FFI");
        assert!(
            query_err.to_string().contains("null CUDA event"),
            "event query diagnostic must explain the null-event hazard: {query_err}"
        );

        let sync_err = event
            .synchronize()
            .expect_err("Fix: event synchronize must reject null events before FFI");
        assert!(
            sync_err.to_string().contains("null CUDA event"),
            "event sync diagnostic must explain the null-event hazard: {sync_err}"
        );
    }

    #[test]
    fn event_elapsed_time_rejects_null_timing_event_before_ffi() {
        let event = super::CudaEvent {
            raw: std::ptr::null_mut(),
        };
        let err = event
            .elapsed_time_ns(&event)
            .expect_err("Fix: elapsed timing must reject null events before FFI");
        assert!(
            err.to_string().contains("null CUDA timing event"),
            "event elapsed diagnostic must explain the null-event hazard: {err}"
        );
    }

    #[test]
    fn stream_lifecycle_ffi_is_single_sourced_for_graph_capture() {
        let stream = include_str!("stream.rs");
        let cuda_graph = include_str!("backend/cuda_graph.rs");
        let create_ffi = concat!("cudarc::driver::sys::", "cuStreamCreate(");
        let destroy_ffi = concat!("cudarc::driver::sys::", "cuStreamDestroy_v2(");

        assert_eq!(
            stream.matches(create_ffi).count(),
            1,
            "Fix: raw CUDA stream creation must stay behind create_non_blocking_raw_stream."
        );
        assert_eq!(
            stream.matches(destroy_ffi).count(),
            1,
            "Fix: raw CUDA stream destruction must stay behind destroy_raw_stream."
        );
        assert_eq!(
            cuda_graph.matches(create_ffi).count() + cuda_graph.matches(destroy_ffi).count(),
            0,
            "Fix: cudaGraph capture must use the shared stream lifecycle helpers instead of direct stream FFI."
        );
        assert!(
            stream.contains("fn create_non_blocking_raw_stream(")
                && stream.contains("returned a null stream after reporting success")
                && cuda_graph.contains("create_non_blocking_raw_stream"),
            "Fix: shared CUDA stream creation must reject null-success handles and be used by cudaGraph."
        );
    }

    #[test]
    fn event_lifecycle_ffi_is_single_sourced() {
        let stream = include_str!("stream.rs");
        let create_ffi = concat!("cudarc::driver::sys::", "cuEventCreate(");
        let destroy_ffi = concat!("cudarc::driver::sys::", "cuEventDestroy_v2(");

        assert_eq!(
            stream.matches(create_ffi).count(),
            1,
            "Fix: raw CUDA event creation must stay behind create_raw_event."
        );
        assert_eq!(
            stream.matches(destroy_ffi).count(),
            1,
            "Fix: raw CUDA event destruction must stay behind destroy_raw_event."
        );
        assert!(
            stream.contains("fn create_raw_event(")
                && stream.contains("returned a null event after reporting success")
                && stream.contains("fn destroy_raw_event(")
                && stream.contains("CudaEvent::completion")
                && stream.contains("CudaEvent::timing"),
            "Fix: CUDA event lifecycle must use shared create/destroy helpers with null-success validation."
        );
    }

    #[test]
    fn graph_replay_uses_shared_stream_query_helper() {
        let stream = include_str!("stream.rs");
        let graph_replay = include_str!("backend/cuda_graph_replay.rs");
        let query_ffi = concat!("cudarc::driver::sys::", "cuStreamQuery(");

        assert_eq!(
            stream.matches(query_ffi).count(),
            1,
            "Fix: raw CUDA stream query must stay behind query_raw_stream_ready."
        );
        assert_eq!(
            graph_replay.matches(query_ffi).count(),
            0,
            "Fix: CUDA graph replay must use query_raw_stream_ready instead of raw cuStreamQuery."
        );
        assert!(
            graph_replay.contains("query_raw_stream_ready")
                && stream.contains("fn query_raw_stream_ready("),
            "Fix: graph replay polling must use the shared stream query helper."
        );
    }

    #[test]
    fn pending_dispatch_drop_leaks_resources_when_completion_is_unproven() {
        let source = include_str!("stream.rs");
        let drop_impl = source
            .rsplit("impl Drop for CudaPendingDispatch")
            .next()
            .expect("Fix: CudaPendingDispatch must own a drop implementation.");
        assert!(
            drop_impl.contains("if !self.force_completion_on_drop()")
                && drop_impl.contains("self.leak_inflight_resources_after_drop_sync_failure();")
                && drop_impl.contains("return;")
                && drop_impl.contains("self.release_launch_resources();"),
            "Fix: CUDA pending-dispatch drop must not recycle launch resources unless completion was forced or already proven."
        );

        let force_completion = source
            .split("fn force_completion_on_drop(")
            .nth(1)
            .expect("Fix: pending-dispatch drop must route synchronization through a helper.")
            .split("fn leak_inflight_resources_after_drop_sync_failure(")
            .next()
            .expect("Fix: completion helper must precede the leak helper.");
        assert!(
            force_completion.contains("return false;")
                && force_completion.contains("stream.synchronize()")
                && force_completion.contains("self.completed.store(true, Ordering::Release);"),
            "Fix: drop-time CUDA completion must fail closed on bind/synchronize errors and mark completion only after stream synchronization succeeds."
        );

        let leak_helper = source
            .split("fn leak_inflight_resources_after_drop_sync_failure(")
            .nth(1)
            .expect("Fix: pending-dispatch drop must have an explicit leak-on-sync-failure helper.")
            .split("/// Await completion and return output buffers plus device elapsed time.")
            .next()
            .expect("Fix: leak helper must be local to the pending-dispatch implementation.");
        for field in [
            "event",
            "timing_start",
            "timing_end",
            "stream",
            "allocations",
            "resident_use",
            "host_transfers",
        ] {
            assert!(
                leak_helper.contains(&format!("self.{field}.take()")),
                "Fix: CUDA pending-dispatch drop sync failure must remove {field} from normal Drop ownership."
            );
        }
        assert!(
            leak_helper.contains("std::mem::forget(Arc::clone(&self.ctx));")
                && leak_helper.matches("std::mem::forget(").count() >= 8,
            "Fix: CUDA pending-dispatch drop sync failure must keep the CUDA context alive and leak in-flight CUDA resources instead of dropping or pooling them."
        );
    }
}

impl Drop for CudaPendingDispatch {
    fn drop(&mut self) {
        if !self.force_completion_on_drop() {
            self.leak_inflight_resources_after_drop_sync_failure();
            return;
        }
        self.release_launch_resources();
        self.allocations.take();
        self.resident_use.take();
        self.host_transfers.take();
    }
}
