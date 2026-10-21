//! Autonomous IO loop for persistent megakernel.
//!
//! This module implements Innovation I.5: host-side pump thread that
//! polls the GPU's `io_queue` for requests and services them via
//! io_uring. This removes the CPU from the dispatch critical path.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::megakernel::io::{claim_io_requests_into, complete_io_request, io_op};
use crate::uring::stream::AsyncUringStream;
use crate::PipelineError;

const IDLE_SPINS: u32 = 64;
const MIN_IDLE_PARK: Duration = Duration::from_micros(10);
const MAX_IDLE_PARK: Duration = Duration::from_micros(100);

/// Fixed-buffer destination registered with io_uring.
///
/// A megakernel IO request whose `dst_handle` matches `handle` can be serviced
/// with `IORING_OP_READ_FIXED`, avoiding per-request iovec allocation and
/// kernel-side iovec validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisteredIoDestination {
    /// Megakernel IO destination handle.
    pub handle: u32,
    /// io_uring registered-buffer index.
    pub buf_index: u16,
    /// Byte offset inside the registered GPU-visible buffer.
    pub target_offset: u64,
}

#[derive(Default)]
struct IdleBackoff {
    polls: u32,
}

impl IdleBackoff {
    fn reset(&mut self) {
        self.polls = 0;
    }

    fn wait(&mut self, shutdown: &AtomicBool) {
        if shutdown.load(Ordering::Acquire) {
            return;
        }
        self.polls = self.polls.saturating_add(1);
        if self.polls <= IDLE_SPINS {
            std::hint::spin_loop();
            return;
        }
        let shift = (self.polls - IDLE_SPINS).min(7);
        let multiplier = 1_u32 << shift;
        let park = MIN_IDLE_PARK
            .checked_mul(multiplier)
            .unwrap_or(MAX_IDLE_PARK)
            .min(MAX_IDLE_PARK);
        thread::park_timeout(park);
    }
}

/// Host-side pump that services GPU-driven IO requests.
pub struct MegakernelIoLoop {
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<Result<(), PipelineError>>>,
}

impl MegakernelIoLoop {
    /// Start a background thread that polls `io_queue_mapped`.
    ///
    /// READ requests require registered destinations; call
    /// [`Self::spawn_with_registered_destinations`] for production IO.
    pub fn spawn(stream: AsyncUringStream<'static>, io_queue_mapped: &'static mut [u8]) -> Self {
        Self::spawn_with_registered_destinations(stream, io_queue_mapped, Vec::new())
    }

    /// Start a background IO pump with a registered destination table.
    ///
    /// Requests whose `dst_handle` is present in `registered_destinations` use
    /// fixed-buffer reads. Unregistered READ destinations stop the pump with a
    /// host-visible error instead of silently taking a host-iovec compatibility
    /// route.
    pub fn spawn_with_registered_destinations(
        mut stream: AsyncUringStream<'static>,
        io_queue_mapped: &'static mut [u8],
        registered_destinations: Vec<RegisteredIoDestination>,
    ) -> Self {
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = Arc::clone(&shutdown);

        let handle = thread::spawn(move || {
            let mut backoff = IdleBackoff::default();
            let mut requests = Vec::new();
            let mut registered_destinations = registered_destinations;
            registered_destinations.sort_unstable_by_key(|destination| destination.handle);
            while !shutdown_clone.load(Ordering::Acquire) {
                while let Some(cqe) = stream.ring_state.peek_cqe() {
                    let res = cqe.res;
                    let slot_idx = cqe.user_data;
                    stream.ring_state.advance_cq();
                    stream.inflight =
                        stream
                            .inflight
                            .checked_sub(1)
                            .ok_or(PipelineError::QueueFull {
                                queue: "io_uring",
                                fix: "megakernel IO loop completion arrived with no inflight SQE; rebuild the IO stream state",
                            })?;
                    let slot_idx = u32::try_from(slot_idx).map_err(|error| {
                        PipelineError::QueueFull {
                            queue: "completion",
                            fix: match error {
                                _ => "io_uring completion user_data does not fit megakernel IO slot index; keep user_data in u32 slot-id range",
                            },
                        }
                    })?;
                    complete_io_request(io_queue_mapped, slot_idx, res >= 0)?;
                    backoff.reset();
                }

                // 1. Claim GPU-published IO requests exactly once.
                claim_io_requests_into(io_queue_mapped, &mut requests)?;

                if requests.is_empty() {
                    if stream.inflight() > 0 {
                        stream.flush_submissions()?;
                        stream.ring_state.enter(0, 1, 1)?;
                    } else {
                        backoff.wait(&shutdown_clone);
                    }
                    continue;
                }
                backoff.reset();

                for req in requests.iter().copied() {
                    match req.op_type {
                        // SAFETY: io_uring submission queue entry initialized in-place; the SQE
                        // memory is owned by the ring and lives for the duration of the submit.
                        io_op::READ => unsafe {
                            let fd = req.src_handle as i32;
                            if let Ok(destination_idx) = registered_destinations
                                .binary_search_by_key(&req.dst_handle, |destination| {
                                    destination.handle
                                })
                            {
                                let destination = registered_destinations[destination_idx];
                                // Bug fix: a submit_read_fixed_at error
                                // previously returned via `?` while the
                                // slot was still CLAIMED, hanging the
                                // GPU which never saw a completion.
                                // Mark the slot failed first, then
                                // propagate the error.
                                if let Err(e) = stream.submit_read_fixed_at(
                                    fd,
                                    req.offset,
                                    req.byte_count,
                                    destination.target_offset,
                                    destination.buf_index,
                                    u64::from(req.slot_idx),
                                ) {
                                    let _ =
                                        complete_io_request(io_queue_mapped, req.slot_idx, false);
                                    return Err(PipelineError::Backend(e.to_string()));
                                }
                            } else {
                                complete_io_request(io_queue_mapped, req.slot_idx, false)?;
                                return Err(PipelineError::Backend(format!(
                                    "megakernel IO READ requested unregistered GPU destination handle {} in slot {}. Fix: register the destination with MegakernelIoLoop::spawn_with_registered_destinations before publishing READ requests.",
                                    req.dst_handle, req.slot_idx
                                )));
                            }
                        },
                        io_op::FENCE => complete_io_request(io_queue_mapped, req.slot_idx, true)?,
                        io_op::WRITE => complete_io_request(io_queue_mapped, req.slot_idx, false)?,
                        _ => complete_io_request(io_queue_mapped, req.slot_idx, false)?,
                    }
                }
                // Bug fix: same hazard as the per-request submit error
                //  -  if flush_submissions fails, every slot we just
                // claimed is stranded in CLAIMED. Mark every still-
                // claimed slot failed before propagating.
                if let Err(e) = stream.flush_submissions() {
                    for req in requests.iter().copied() {
                        if req.op_type == io_op::READ {
                            let _ = complete_io_request(io_queue_mapped, req.slot_idx, false);
                        }
                    }
                    return Err(e);
                }
            }
            Ok(())
        });

        Self {
            shutdown,
            handle: Some(handle),
        }
    }

    /// Stop the pump thread.
    pub fn stop(&mut self) -> Result<(), PipelineError> {
        self.shutdown.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            handle.thread().unpark();
            handle
                .join()
                .map_err(|_| PipelineError::Backend("IO loop thread panicked".to_string()))?
        } else {
            Ok(())
        }
    }
}
