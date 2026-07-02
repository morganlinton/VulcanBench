After a broker reconnect, a prefork worker can stall: new tasks get written to a worker that is already busy running a long task, even while other workers sit idle.

When the broker connection drops and comes back, the consumer calls `AsynPool.flush()` to reset in-flight write state. The prefork pool tracks which workers are mid-task in `_busy_workers` (keyed by each worker's inqueue write file descriptor) so the fair scheduler avoids dispatching a new task to a worker that is still executing one.

The bug: `flush()` clears `_busy_workers` outright. A worker that is genuinely still running an accepted task is now considered idle, so the scheduler writes a new task onto its inqueue. That task blocks behind the long-running one, and the worker appears stalled even though another worker is free.

Fix `flush()` so that a worker still executing an accepted (running) job stays in `_busy_workers` across the flush, while workers not running an accepted job are removed. Identify the still-busy worker fd from the running job (its executing process once the body has been written, falling back to the process it was dispatched to). The flush must only ever remove fds from the busy set based on current reality, never fabricate a busy entry for a worker that is not running an accepted job.
