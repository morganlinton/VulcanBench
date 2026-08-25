//! Resident task queue ABI for pause, resume, requeue, and priority aging.

use vyre_driver::backend::BackendError;

use super::planner::MegakernelWorkItem;
use super::policy::MegakernelLaunchRequest;

/// Number of `u32` words in one continuation task slot.
pub const TASK_SLOT_WORDS: usize = 16;

/// Number of bytes in one continuation task slot.
pub const TASK_SLOT_BYTES: usize = TASK_SLOT_WORDS * core::mem::size_of::<u32>();

/// Lowest flag bit set when a task voluntarily paused at a continuation point.
pub const TASK_FLAG_PAUSED: u32 = 1 << 0;

/// Flag bit set when a task yielded so another task can run on the same worker.
pub const TASK_FLAG_YIELDED: u32 = 1 << 1;

/// Flag bit set when a task asked the scheduler to publish it again.
pub const TASK_FLAG_REQUEUE_REQUESTED: u32 = 1 << 2;

/// Flag bit set when a paused task is eligible to resume.
pub const TASK_FLAG_RESUME_READY: u32 = 1 << 3;

/// GPU-visible lifecycle state for one continuation task slot.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// Slot is empty and may be reused.
    Empty = 0,
    /// Slot is published and may be claimed by a GPU worker.
    Ready = 1,
    /// Slot is currently owned by a GPU worker.
    Running = 2,
    /// Slot finished successfully.
    Done = 3,
    /// Slot is paused until an external device-visible condition is met.
    Paused = 4,
    /// Slot yielded voluntarily and should remain schedulable.
    Yielded = 5,
    /// Slot should be placed back into its priority partition.
    Requeued = 6,
    /// Slot faulted and must not be executed again without repair.
    Faulted = 7,
}

impl TaskState {
    /// Decode a raw ABI word into a task state.
    #[must_use]
    pub const fn from_word(word: u32) -> Option<Self> {
        match word {
            0 => Some(Self::Empty),
            1 => Some(Self::Ready),
            2 => Some(Self::Running),
            3 => Some(Self::Done),
            4 => Some(Self::Paused),
            5 => Some(Self::Yielded),
            6 => Some(Self::Requeued),
            7 => Some(Self::Faulted),
            _ => None,
        }
    }

    /// Encode this state as the raw ABI word written by the GPU scheduler.
    #[must_use]
    pub const fn word(self) -> u32 {
        self as u32
    }

    /// Return true when this state is eligible for GPU scheduling.
    #[must_use]
    pub const fn is_schedulable(self) -> bool {
        matches!(self, Self::Ready | Self::Yielded | Self::Requeued)
    }
}

/// Priority partition for a continuation task slot.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskPriority {
    /// Highest priority partition for latency-critical work.
    Critical = 0,
    /// High priority partition for urgent work.
    High = 1,
    /// Default priority partition.
    #[default]
    Normal = 2,
    /// Low priority partition for background work.
    Low = 3,
    /// Idle partition processed only when higher priorities are empty.
    Idle = 4,
}

impl TaskPriority {
    /// Decode a raw ABI word into a task priority.
    #[must_use]
    pub const fn from_word(word: u32) -> Option<Self> {
        match word {
            0 => Some(Self::Critical),
            1 => Some(Self::High),
            2 => Some(Self::Normal),
            3 => Some(Self::Low),
            4 => Some(Self::Idle),
            _ => None,
        }
    }

    /// Encode this priority as the raw ABI word used by the priority scheduler.
    #[must_use]
    pub const fn word(self) -> u32 {
        self as u32
    }
}

/// One device-visible continuation task slot.
///
/// The first four words match the persistent ring header:
/// status, opcode, tenant, priority. The remaining twelve words are the slot
/// payload. Words 4..6 preserve the legacy [`MegakernelWorkItem`] payload; words 7..15
/// carry continuation and scheduler state.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TaskWorkItem {
    /// Raw [`TaskState`] word.
    pub state: u32,
    /// Stable op id index into the dialect registry.
    pub op_handle: u32,
    /// Tenant id checked by the runtime scheduler.
    pub tenant_id: u32,
    /// Raw [`TaskPriority`] word.
    pub priority: u32,
    /// Input-buffer handle.
    pub input_handle: u32,
    /// Output-buffer handle.
    pub output_handle: u32,
    /// Per-item parameter word.
    pub param: u32,
    /// Program counter or block id where the worker should resume.
    pub continuation_pc: u32,
    /// Opaque continuation-local scratch word.
    pub continuation_data: u32,
    /// Device-visible epoch at which the task may resume.
    pub resume_epoch: u32,
    /// Stable task id used to join yielded/requeued continuations.
    pub task_id: u32,
    /// Parent task id for split or fan-out work; zero when absent.
    pub parent_task_id: u32,
    /// Scheduler age ticks accumulated while waiting.
    pub age_ticks: u32,
    /// Number of times this task has been requeued.
    pub requeue_count: u32,
    /// Number of times this task has yielded.
    pub yield_count: u32,
    /// Bitset of `TASK_FLAG_*` continuation flags.
    pub flags: u32,
}

impl TaskWorkItem {
    /// Construct a ready continuation task from the compact legacy work item.
    #[must_use]
    pub const fn from_work_item(
        task_id: u32,
        tenant_id: u32,
        priority: TaskPriority,
        item: MegakernelWorkItem,
    ) -> Self {
        Self {
            state: TaskState::Ready.word(),
            op_handle: item.op_handle,
            tenant_id,
            priority: priority.word(),
            input_handle: item.input_handle,
            output_handle: item.output_handle,
            param: item.param,
            continuation_pc: 0,
            continuation_data: 0,
            resume_epoch: 0,
            task_id,
            parent_task_id: 0,
            age_ticks: 0,
            requeue_count: 0,
            yield_count: 0,
            flags: 0,
        }
    }

    /// Return the compact legacy work item payload carried by this task.
    #[must_use]
    pub const fn work_item(&self) -> MegakernelWorkItem {
        MegakernelWorkItem {
            op_handle: self.op_handle,
            input_handle: self.input_handle,
            output_handle: self.output_handle,
            param: self.param,
        }
    }

    /// Decode the task state word.
    #[must_use]
    pub const fn task_state(&self) -> Option<TaskState> {
        TaskState::from_word(self.state)
    }

    /// Decode the task priority word.
    #[must_use]
    pub const fn task_priority(&self) -> Option<TaskPriority> {
        TaskPriority::from_word(self.priority)
    }

    /// Return true when the task is eligible to be claimed by a worker.
    #[must_use]
    pub const fn is_schedulable(&self) -> bool {
        match self.task_state() {
            Some(state) => state.is_schedulable(),
            None => false,
        }
    }

    /// Encode a pause at `continuation_pc` until `resume_epoch`.
    #[must_use]
    pub fn try_paused(
        mut self,
        continuation_pc: u32,
        continuation_data: u32,
        resume_epoch: u32,
    ) -> Result<Self, BackendError> {
        self.ensure_transitionable("pause")?;
        self.state = TaskState::Paused.word();
        self.continuation_pc = continuation_pc;
        self.continuation_data = continuation_data;
        self.resume_epoch = resume_epoch;
        self.flags = (self.flags | TASK_FLAG_PAUSED) & !TASK_FLAG_RESUME_READY;
        Ok(self)
    }

    /// Encode a pause at `continuation_pc` until `resume_epoch`.
    #[must_use]
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn paused(self, continuation_pc: u32, continuation_data: u32, resume_epoch: u32) -> Self {
        self.try_paused(continuation_pc, continuation_data, resume_epoch)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    /// Mark a paused task ready for GPU-side resume.
    #[must_use]
    pub fn try_resumed(mut self) -> Result<Self, BackendError> {
        if self.task_state() != Some(TaskState::Paused) {
            return Err(invalid_task_transition("resume", self.state));
        }
        self.state = TaskState::Ready.word();
        self.flags =
            (self.flags | TASK_FLAG_RESUME_READY) & !(TASK_FLAG_PAUSED | TASK_FLAG_YIELDED);
        Ok(self)
    }

    /// Mark a paused task ready for GPU-side resume.
    #[must_use]
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn resumed(self) -> Self {
        self.try_resumed().unwrap_or_else(|error| panic!("{error}"))
    }

    /// Yield this task back to the scheduler at `continuation_pc`.
    #[must_use]
    pub fn try_yielded(
        mut self,
        continuation_pc: u32,
        continuation_data: u32,
    ) -> Result<Self, BackendError> {
        self.ensure_transitionable("yield")?;
        self.state = TaskState::Yielded.word();
        self.continuation_pc = continuation_pc;
        self.continuation_data = continuation_data;
        self.yield_count = checked_task_counter_increment(self.yield_count, "yield_count")?;
        self.flags |= TASK_FLAG_YIELDED;
        Ok(self)
    }

    /// Yield this task back to the scheduler at `continuation_pc`.
    #[must_use]
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn yielded(self, continuation_pc: u32, continuation_data: u32) -> Self {
        self.try_yielded(continuation_pc, continuation_data)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    /// Requeue this task, optionally changing its priority partition.
    #[must_use]
    pub fn try_requeued(
        mut self,
        continuation_pc: u32,
        continuation_data: u32,
        priority: TaskPriority,
    ) -> Result<Self, BackendError> {
        self.ensure_transitionable("requeue")?;
        self.state = TaskState::Requeued.word();
        self.priority = priority.word();
        self.continuation_pc = continuation_pc;
        self.continuation_data = continuation_data;
        self.requeue_count = checked_task_counter_increment(self.requeue_count, "requeue_count")?;
        self.age_ticks = checked_task_counter_increment(self.age_ticks, "age_ticks")?;
        self.flags |= TASK_FLAG_REQUEUE_REQUESTED;
        Ok(self)
    }

    /// Requeue this task, optionally changing its priority partition.
    #[must_use]
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn requeued(
        self,
        continuation_pc: u32,
        continuation_data: u32,
        priority: TaskPriority,
    ) -> Self {
        self.try_requeued(continuation_pc, continuation_data, priority)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    /// Mark this task completed.
    #[must_use]
    pub fn try_completed(mut self) -> Result<Self, BackendError> {
        self.ensure_transitionable("complete")?;
        self.state = TaskState::Done.word();
        self.flags = 0;
        Ok(self)
    }

    /// Mark this task completed.
    #[must_use]
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn completed(self) -> Self {
        self.try_completed()
            .unwrap_or_else(|error| panic!("{error}"))
    }

    /// Mark this task faulted with a compact fault code.
    #[must_use]
    pub fn try_faulted(mut self, fault_code: u32) -> Result<Self, BackendError> {
        self.ensure_transitionable("fault")?;
        self.state = TaskState::Faulted.word();
        self.continuation_data = fault_code;
        Ok(self)
    }

    /// Mark this task faulted with a compact fault code.
    #[must_use]
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn faulted(self, fault_code: u32) -> Self {
        self.try_faulted(fault_code)
            .unwrap_or_else(|error| panic!("{error}"))
    }

    fn ensure_transitionable(&self, action: &'static str) -> Result<(), BackendError> {
        match self.task_state() {
            Some(TaskState::Empty | TaskState::Done | TaskState::Faulted) | None => {
                Err(invalid_task_transition(action, self.state))
            }
            Some(
                TaskState::Ready
                | TaskState::Running
                | TaskState::Paused
                | TaskState::Yielded
                | TaskState::Requeued,
            ) => Ok(()),
        }
    }
}

/// Queue telemetry derived from device-visible continuation task slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TaskQueueSnapshot {
    /// Count of ready slots.
    pub ready_count: u32,
    /// Count of paused slots.
    pub paused_count: u32,
    /// Count of yielded slots.
    pub yielded_count: u32,
    /// Count of requeued slots.
    pub requeued_count: u32,
    /// Count of running slots.
    pub running_count: u32,
    /// Count of faulted slots.
    pub faulted_count: u32,
    /// Sum of per-slot requeue counters.
    pub total_requeues: u64,
    /// Maximum priority-aging tick observed in the queue.
    pub max_priority_age: u32,
}

impl TaskQueueSnapshot {
    /// Build a queue snapshot from continuation task slots.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the slice contains an unknown task state or
    /// a count cannot fit the u32 ABI.
    pub fn from_tasks(tasks: &[TaskWorkItem]) -> Result<Self, BackendError> {
        let mut snapshot = Self::default();
        for task in tasks {
            snapshot.max_priority_age = snapshot.max_priority_age.max(task.age_ticks);
            snapshot.total_requeues = snapshot
                .total_requeues
                .checked_add(u64::from(task.requeue_count))
                .ok_or_else(|| {
                    BackendError::new(
                        "megakernel task total_requeues overflowed u64. Fix: drain or shard the task ring before launch.",
                    )
                })?;
            match task.task_state() {
                Some(TaskState::Empty | TaskState::Done) => {}
                Some(TaskState::Ready) => checked_increment(&mut snapshot.ready_count)?,
                Some(TaskState::Paused) => checked_increment(&mut snapshot.paused_count)?,
                Some(TaskState::Yielded) => checked_increment(&mut snapshot.yielded_count)?,
                Some(TaskState::Requeued) => checked_increment(&mut snapshot.requeued_count)?,
                Some(TaskState::Running) => checked_increment(&mut snapshot.running_count)?,
                Some(TaskState::Faulted) => checked_increment(&mut snapshot.faulted_count)?,
                None => {
                    return Err(BackendError::new(format!(
                        "megakernel task slot has unknown state word {}. Fix: write a valid TaskState ABI word before publishing the slot.",
                        task.state
                    )));
                }
            }
        }
        Ok(snapshot)
    }

    /// Number of slots immediately eligible for GPU scheduling.
    #[must_use]
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn schedulable_count(&self) -> u32 {
        match self.try_schedulable_count() {
            Ok(value) => value,
            Err(error) => panic!("{error}"),
        }
    }

    /// Checked number of slots immediately eligible for GPU scheduling.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when the summed schedulable count exceeds the
    /// u32 launch ABI.
    pub fn try_schedulable_count(&self) -> Result<u32, BackendError> {
        self.ready_count
            .checked_add(self.yielded_count)
            .and_then(|value| value.checked_add(self.requeued_count))
            .ok_or_else(|| {
                BackendError::new(
                    "megakernel schedulable task count overflowed u32. Fix: shard the task ring before launch.",
                )
            })
    }

    /// Number of slots carrying continuation pressure.
    #[must_use]
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn continuation_pressure_count(&self) -> u64 {
        match self.try_continuation_pressure_count() {
            Ok(value) => value,
            Err(error) => panic!("{error}"),
        }
    }

    /// Checked number of slots carrying continuation pressure.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when continuation pressure exceeds u64.
    pub fn try_continuation_pressure_count(&self) -> Result<u64, BackendError> {
        u64::from(self.yielded_count)
            .checked_add(u64::from(self.requeued_count))
            .and_then(|value| value.checked_add(self.total_requeues))
            .ok_or_else(|| {
                BackendError::new(
                    "megakernel continuation pressure overflowed u64. Fix: drain or shard the task ring before launch.",
                )
            })
    }

    /// Build a Program that runs a one-shot persistent fixpoint over
    /// the queue snapshot's state-counter buffer, converging the
    /// counts to a stable equilibrium representing the queue's
    /// long-run distribution. Wires the self-substrate persistent
    /// fixpoint builder for observability collectors that want stable
    /// signals over transient queue jitter.
    ///
    /// `current_buffer` / `next_buffer` / `changed_buffer` are
    /// caller-supplied buffer names for the persistent_fixpoint
    /// ping-pong; `transfer_body` is the per-iteration body that
    /// reads `current` and writes `next`. Returns a Program suitable
    /// for one dispatch.
    ///
    /// P-RUNTIME-4: replaces a host-side jitter-smoothing loop with
    /// a single GPU-side fixpoint dispatch.
    #[must_use]
    #[cfg(feature = "self-substrate-adapters")]
    pub fn build_state_convergence_program(
        transfer_body: Vec<vyre_foundation::ir::Node>,
        current_buffer: &str,
        next_buffer: &str,
        changed_buffer: &str,
        words: u32,
        max_iterations: u32,
    ) -> vyre_foundation::ir::Program {
        vyre_self_substrate::persistent_fixpoint_program::persistent_fixpoint_program(
            transfer_body,
            current_buffer,
            next_buffer,
            changed_buffer,
            words,
            max_iterations,
        )
    }

    /// Merge this queue telemetry into a launch request.
    #[must_use]
    #[cfg(any(test, feature = "legacy-infallible"))]
    pub fn apply_to_launch_request(
        &self,
        mut request: MegakernelLaunchRequest,
    ) -> MegakernelLaunchRequest {
        request = match self.try_apply_to_launch_request(request) {
            Ok(request) => request,
            Err(error) => panic!("{error}"),
        };
        request
    }

    /// Checked merge of queue telemetry into a launch request.
    ///
    /// # Errors
    ///
    /// Returns [`BackendError`] when schedulable count or continuation pressure
    /// cannot fit the launch request ABI.
    pub fn try_apply_to_launch_request(
        &self,
        mut request: MegakernelLaunchRequest,
    ) -> Result<MegakernelLaunchRequest, BackendError> {
        request.queue_len = self.try_schedulable_count()?;
        request.requeue_count = request
            .requeue_count
            .checked_add(self.try_continuation_pressure_count()?)
            .ok_or_else(|| {
                BackendError::new(
                    "megakernel launch request requeue_count overflowed u64. Fix: drain or shard the task ring before launch.",
                )
            })?;
        request.max_priority_age = request.max_priority_age.max(self.max_priority_age);
        Ok(request)
    }
}

fn checked_increment(counter: &mut u32) -> Result<(), BackendError> {
    *counter = counter.checked_add(1).ok_or_else(|| {
        BackendError::new(
            "megakernel task queue count exceeds u32::MAX. Fix: shard the task ring before launch.",
        )
    })?;
    Ok(())
}

fn checked_task_counter_increment(value: u32, label: &'static str) -> Result<u32, BackendError> {
    value.checked_add(1).ok_or_else(|| {
        BackendError::new(format!(
            "megakernel task {label} overflowed u32. Fix: drain or shard the task ring before mutating continuation counters."
        ))
    })
}

fn invalid_task_transition(action: &'static str, state_word: u32) -> BackendError {
    let state = TaskState::from_word(state_word)
        .map(|state| format!("{state:?}"))
        .unwrap_or_else(|| format!("unknown({state_word})"));
    BackendError::new(format!(
        "megakernel task cannot {action} from state {state}. Fix: publish only legal task lifecycle transitions before mutating the task slot."
    ))
}

#[cfg(test)]
mod tests;
