//! Host-side async copy stream scheduling.
//!
//! wgpu command submission already lets copies and compute live in one GPU
//! queue. This module models the higher-level stream contract exposed by
//! `Node::AsyncLoad { tag }` and `Node::AsyncWait { tag }`: copy staging work
//! is started on a separate host worker and joined only when the matching wait
//! is reached, so CPU memcpy/staging can overlap compute preparation.
//!
//! Backing worker policy: uses `tokio::task::spawn_blocking` when a tokio
//! runtime is active, otherwise uses the crate-global bounded worker pool.
//! The non-tokio path must not spawn one OS thread per AsyncLoad tag: that
//! pattern thrashes the scheduler under heavy streaming workloads. The bounded
//! pool caps worker count and queue depth while preserving overlap semantics.

use std::sync::{mpsc, LazyLock};

use crossbeam_channel::{bounded, Receiver as CrossbeamReceiver, Sender as CrossbeamSender};
use rustc_hash::FxHashMap;
use vyre_driver::BackendError;

use crate::thread_pool::{BoundedWorkerJob, BoundedWorkerPool};

/// Completion reported by a tokio blocking worker.
enum TokioBlockingCompletion {
    Returned(Result<(), BackendError>),
    Panicked(String),
}

/// Handle to the work backing an in-flight tag. Stored in the scheduler until
/// the matching `async_wait` call. The tokio variant carries a plain blocking
/// receiver so `async_wait` never has to construct an emergency runtime just to
/// join a task after the caller's runtime moved or shut down.
enum InFlight {
    Pool {
        completion: CrossbeamReceiver<Result<(), BackendError>>,
    },
    TokioBlocking {
        completion: mpsc::Receiver<TokioBlockingCompletion>,
        task: tokio::task::JoinHandle<()>,
    },
}

struct AsyncCopyJob {
    copy: Box<dyn FnOnce() -> Result<(), BackendError> + Send + 'static>,
    response: CrossbeamSender<Result<(), BackendError>>,
}

impl BoundedWorkerJob for AsyncCopyJob {
    type Output = ();

    fn response(&self) -> &CrossbeamSender<Result<Self::Output, BackendError>> {
        &self.response
    }

    fn run(self) -> Result<Self::Output, BackendError> {
        (self.copy)()
    }
}

struct AsyncCopyPool {
    pool: BoundedWorkerPool<AsyncCopyJob>,
}

impl AsyncCopyPool {
    fn global() -> Result<&'static Self, BackendError> {
        static POOL: LazyLock<Result<AsyncCopyPool, BackendError>> =
            LazyLock::new(AsyncCopyPool::new);
        POOL.as_ref()
            .map_err(|error| BackendError::new(error.to_string()))
    }

    fn new() -> Result<Self, BackendError> {
        Ok(Self {
            pool: BoundedWorkerPool::new(
                256,
                "vyre-wgpu-async-copy",
                "inspect async copy staging buffer ownership and copy closure invariants.",
                "reduce process thread count or increase system nproc limit.",
            )?,
        })
    }

    fn submit<F>(
        &self,
        copy: F,
    ) -> Result<CrossbeamReceiver<Result<(), BackendError>>, BackendError>
    where
        F: FnOnce() -> Result<(), BackendError> + Send + 'static,
    {
        let (response, completion) = bounded(1);
        self.pool.submit_blocking(
            AsyncCopyJob {
                copy: Box::new(copy),
                response,
            },
            "recreate the process; the async-copy worker pool only closes during shutdown.",
        )?;
        Ok(completion)
    }
}

/// Async copy scheduler keyed by IR stream tags.
#[derive(Default)]
pub struct AsyncCopyStreams {
    in_flight: FxHashMap<String, InFlight>,
}

impl AsyncCopyStreams {
    /// Create an empty stream scheduler.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Start copy work associated with `tag`.
    ///
    /// If a tokio runtime handle is available on the current thread the
    /// closure is dispatched to `tokio::task::spawn_blocking` so the
    /// runtime's blocking-thread pool amortizes scheduling cost. Otherwise the
    /// crate-global bounded async-copy pool is used.
    ///
    /// # Errors
    ///
    /// Returns a backend error if the tag is already in flight.
    pub fn async_load<F>(&mut self, tag: impl Into<String>, copy: F) -> Result<(), BackendError>
    where
        F: FnOnce() -> Result<(), BackendError> + Send + 'static,
    {
        let tag = tag.into();
        if self.in_flight.contains_key(&tag) {
            return Err(BackendError::new(format!(
                "async copy tag `{tag}` is already in flight. Fix: wait before reusing a stream tag."
            )));
        }
        let handle = match tokio::runtime::Handle::try_current() {
            Ok(rt) => {
                let (completion_tx, completion_rx) = mpsc::sync_channel(1);
                let task = rt.spawn_blocking(move || {
                    let completion =
                        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(copy)) {
                            Ok(result) => TokioBlockingCompletion::Returned(result),
                            Err(payload) => {
                                TokioBlockingCompletion::Panicked(panic_payload(&*payload).into())
                            }
                        };
                    if completion_tx.send(completion).is_err() {
                        tracing::warn!(
                            "Fix: async-copy completion receiver was dropped before the blocking copy reported completion."
                        );
                    }
                });
                InFlight::TokioBlocking {
                    completion: completion_rx,
                    task,
                }
            }
            Err(_) => InFlight::Pool {
                completion: AsyncCopyPool::global()?.submit(copy)?,
            },
        };
        self.in_flight.insert(tag, handle);
        Ok(())
    }

    /// Wait for a copy previously started by [`Self::async_load`].
    ///
    /// # Errors
    ///
    /// Returns a backend error if the tag is unknown, the worker panicked, or
    /// the copy closure returned an error.
    pub fn async_wait(&mut self, tag: &str) -> Result<(), BackendError> {
        let handle = self.in_flight.remove(tag).ok_or_else(|| {
            BackendError::new(format!(
                "async copy tag `{tag}` has no matching AsyncLoad. Fix: emit AsyncLoad before AsyncWait."
            ))
        })?;
        match handle {
            InFlight::Pool { completion } => completion.recv().map_err(|error| {
                BackendError::new(format!(
                    "async copy worker for `{tag}` exited without publishing completion: {error}. Fix: inspect staging buffer ownership and copy closure invariants."
                ))
            })?,
            InFlight::TokioBlocking { completion, task } => {
                let completion = completion.recv().map_err(|_| {
                    BackendError::new(format!(
                        "async copy worker for `{tag}` exited without publishing completion. Fix: inspect staging buffer ownership and copy closure invariants."
                    ))
                })?;
                drop(task);
                match completion {
                    TokioBlockingCompletion::Returned(result) => result,
                    TokioBlockingCompletion::Panicked(payload) => Err(BackendError::new(
                        format!(
                            "async copy worker for `{tag}` panicked: {payload}. Fix: inspect staging buffer ownership and copy closure invariants."
                        ),
                    )),
                }
            }
        }
    }

    /// Start copy work, run compute work, then wait for the copy tag.
    ///
    /// # Errors
    ///
    /// Propagates copy or compute failures with their original context.
    pub fn overlap_copy_compute<C, G>(
        &mut self,
        tag: impl Into<String>,
        copy: C,
        compute: G,
    ) -> Result<(), BackendError>
    where
        C: FnOnce() -> Result<(), BackendError> + Send + 'static,
        G: FnOnce() -> Result<(), BackendError>,
    {
        let tag = tag.into();
        self.async_load(tag.clone(), copy)?;
        compute()?;
        self.async_wait(&tag)
    }
}

impl Drop for AsyncCopyStreams {
    fn drop(&mut self) {
        for (_, handle) in self.in_flight.drain() {
            match handle {
                InFlight::Pool { .. } => {}
                InFlight::TokioBlocking { task, .. } => {
                    // Abort if the blocking task has not started. If it is
                    // already running, tokio lets it finish; the completion
                    // channel is dropped so the task cannot retain scheduler
                    // state after completion.
                    task.abort();
                }
            }
        }
    }
}

fn panic_payload<'a>(payload: &'a (dyn std::any::Any + Send + 'static)) -> &'a str {
    payload
        .downcast_ref::<&'static str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("<non-string panic payload>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn async_copy_compute_overlap_is_synchronized_without_sleep() {
        let (copy_started_tx, copy_started_rx) = mpsc::sync_channel(1);
        let (release_copy_tx, release_copy_rx) = mpsc::sync_channel(1);
        let mut streams = AsyncCopyStreams::new();
        streams
            .overlap_copy_compute(
                "stage-0",
                move || {
                    copy_started_tx
                        .send(())
                        .expect("Fix: compute side must stay alive until copy starts");
                    release_copy_rx
                        .recv()
                        .expect("Fix: compute side must release copy before wait");
                    Ok(())
                },
                move || {
                    copy_started_rx
                        .recv_timeout(Duration::from_secs(5))
                        .expect("Fix: copy work must start before compute can release it");
                    release_copy_tx
                        .send(())
                        .expect("Fix: copy side must stay alive until compute releases it");
                    Ok(())
                },
            )
            .expect("Fix: async copy and compute should complete");
    }

    #[test]
    fn tokio_blocking_wait_does_not_need_live_runtime() {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("Fix: tokio runtime must build for async copy test");
        let mut streams = AsyncCopyStreams::new();
        {
            let _guard = runtime.enter();
            streams
                .async_load("stage-0", || Ok(()))
                .expect("Fix: async load must enqueue on active tokio runtime");
        }
        drop(runtime);

        streams
            .async_wait("stage-0")
            .expect("Fix: async wait must join through completion channel without a live runtime");
    }
}
