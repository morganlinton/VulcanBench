use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use crossbeam_channel::{bounded, Receiver, Sender};
use vyre_driver::BackendError;

pub(crate) trait BoundedWorkerJob: Send + 'static {
    type Output: Send + 'static;

    fn response(&self) -> &Sender<Result<Self::Output, BackendError>>;
    fn run(self) -> Result<Self::Output, BackendError>;
}

pub(crate) struct BoundedWorkerPool<J: BoundedWorkerJob> {
    sender: Sender<J>,
}

impl<J: BoundedWorkerJob> BoundedWorkerPool<J> {
    pub(crate) fn new(
        queue_capacity: usize,
        thread_name_prefix: &'static str,
        panic_fix: &'static str,
        spawn_fix: &'static str,
    ) -> Result<Self, BackendError> {
        let (sender, receiver) = bounded::<J>(queue_capacity);
        let workers = std::thread::available_parallelism()
            .map(usize::from)
            .unwrap_or(1)
            .clamp(1, 32);
        let panic_fix = Arc::<str>::from(panic_fix);
        for index in 0..workers {
            let receiver = receiver.clone();
            let panic_fix = Arc::clone(&panic_fix);
            std::thread::Builder::new()
                .name(format!("{thread_name_prefix}-{index}"))
                .spawn(move || worker_loop(receiver, panic_fix))
                .map_err(|error| {
                    BackendError::new(format!(
                        "failed to spawn {thread_name_prefix} worker {index}: {error}. Fix: {spawn_fix}"
                    ))
                })?;
        }
        Ok(Self { sender })
    }

    pub(crate) fn submit_blocking(&self, job: J, closed_fix: &str) -> Result<(), BackendError> {
        self.sender.send(job).map_err(|error| {
            BackendError::new(format!(
                "bounded worker pool is closed: {error}. Fix: {closed_fix}"
            ))
        })
    }
}

fn worker_loop<J: BoundedWorkerJob>(receiver: Receiver<J>, panic_fix: Arc<str>) {
    while let Ok(job) = receiver.recv() {
        let response = job.response().clone();
        let result = catch_unwind(AssertUnwindSafe(|| job.run())).unwrap_or_else(|_| {
            Err(BackendError::new(format!(
                "bounded worker panicked. Fix: {panic_fix}"
            )))
        });
        if let Err(error) = response.send(result) {
            tracing::error!(
                ?error,
                "bounded worker result was lost because the receiver dropped"
            );
        }
    }
}
