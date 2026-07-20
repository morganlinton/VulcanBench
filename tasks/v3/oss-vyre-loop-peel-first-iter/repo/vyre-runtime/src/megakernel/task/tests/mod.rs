use super::*;

fn legacy_item(op_handle: u32) -> MegakernelWorkItem {
    MegakernelWorkItem {
        op_handle,
        input_handle: 11,
        output_handle: 12,
        param: 13,
    }
}

#[test]
fn task_work_item_is_one_ring_slot_and_preserves_legacy_payload() {
    assert_eq!(core::mem::size_of::<TaskWorkItem>(), TASK_SLOT_BYTES);
    assert_eq!(
        core::mem::align_of::<TaskWorkItem>(),
        core::mem::align_of::<u32>()
    );

    let task = TaskWorkItem::from_work_item(7, 3, TaskPriority::High, legacy_item(99));
    assert_eq!(task.state, TaskState::Ready.word());
    assert_eq!(task.priority, TaskPriority::High.word());
    assert_eq!(task.task_id, 7);
    assert_eq!(task.tenant_id, 3);
    assert_eq!(task.work_item(), legacy_item(99));
}

#[test]
fn transitions_encode_pause_resume_yield_requeue_without_host_side_state() {
    let task = TaskWorkItem::from_work_item(1, 0, TaskPriority::Normal, legacy_item(10))
        .paused(44, 55, 66);
    assert_eq!(task.task_state(), Some(TaskState::Paused));
    assert_eq!(task.continuation_pc, 44);
    assert_eq!(task.continuation_data, 55);
    assert_eq!(task.resume_epoch, 66);
    assert_eq!(task.flags & TASK_FLAG_PAUSED, TASK_FLAG_PAUSED);
    assert!(!task.is_schedulable());

    let task = task.resumed().yielded(77, 88);
    assert_eq!(task.task_state(), Some(TaskState::Yielded));
    assert_eq!(task.yield_count, 1);
    assert_eq!(task.flags & TASK_FLAG_YIELDED, TASK_FLAG_YIELDED);
    assert!(task.is_schedulable());

    let task = task.requeued(99, 100, TaskPriority::Critical);
    assert_eq!(task.task_state(), Some(TaskState::Requeued));
    assert_eq!(task.task_priority(), Some(TaskPriority::Critical));
    assert_eq!(task.requeue_count, 1);
    assert_eq!(task.age_ticks, 1);
    assert_eq!(
        task.flags & TASK_FLAG_REQUEUE_REQUESTED,
        TASK_FLAG_REQUEUE_REQUESTED
    );
    assert!(task.is_schedulable());
}

#[test]
fn snapshot_feeds_launch_request_from_schedulable_continuations() {
    let ready = TaskWorkItem::from_work_item(1, 0, TaskPriority::Normal, legacy_item(1));
    let paused = ready.paused(10, 20, 30);
    let yielded = ready.yielded(11, 21);
    let requeued =
        ready
            .requeued(12, 22, TaskPriority::High)
            .requeued(13, 23, TaskPriority::Critical);
    let done = ready.completed();

    let snapshot = TaskQueueSnapshot::from_tasks(&[ready, paused, yielded, requeued, done])
        .expect("Fix: valid task states must snapshot");
    assert_eq!(snapshot.ready_count, 1);
    assert_eq!(snapshot.paused_count, 1);
    assert_eq!(snapshot.yielded_count, 1);
    assert_eq!(snapshot.requeued_count, 1);
    assert_eq!(snapshot.schedulable_count(), 3);
    assert_eq!(snapshot.total_requeues, 2);
    assert_eq!(snapshot.max_priority_age, 2);

    let request = snapshot.apply_to_launch_request(MegakernelLaunchRequest::direct(99, 64, 256));
    assert_eq!(request.queue_len, 3);
    assert_eq!(request.requeue_count, 4);
    assert_eq!(request.max_priority_age, 2);
}

#[test]
fn snapshot_rejects_unknown_state_word() {
    let mut task = TaskWorkItem::from_work_item(1, 0, TaskPriority::Normal, legacy_item(1));
    task.state = 99;

    let err =
        TaskQueueSnapshot::from_tasks(&[task]).expect_err("unknown ABI state word must reject");
    assert!(
        format!("{err}").contains("unknown state word 99"),
        "error must name the invalid state; got: {err}"
    );
}

#[test]
fn fallible_transitions_reject_terminal_state_rewrites() {
    let ready = TaskWorkItem::from_work_item(1, 0, TaskPriority::Normal, legacy_item(1));
    let done = ready.completed();

    let err = done
        .try_requeued(1, 2, TaskPriority::High)
        .expect_err("Fix: completed tasks must not be requeued");
    assert!(
        err.to_string().contains("cannot requeue from state Done"),
        "Fix: terminal transition errors must name the action and state: {err}"
    );

    let mut unknown = ready;
    unknown.state = 99;
    let err = unknown
        .try_yielded(1, 2)
        .expect_err("Fix: unknown task states must not transition");
    assert!(
        err.to_string().contains("unknown(99)"),
        "Fix: unknown-state transition errors must preserve the raw word: {err}"
    );
}

#[test]
fn fallible_transitions_reject_counter_overflow_without_panic() {
    let mut ready = TaskWorkItem::from_work_item(1, 0, TaskPriority::Normal, legacy_item(1));
    ready.yield_count = u32::MAX;
    let err = ready
        .try_yielded(1, 2)
        .expect_err("Fix: yield_count overflow must return BackendError");
    assert!(
        err.to_string().contains("yield_count overflowed u32"),
        "Fix: yield_count overflow errors must name the counter: {err}"
    );

    let mut ready = TaskWorkItem::from_work_item(1, 0, TaskPriority::Normal, legacy_item(1));
    ready.requeue_count = u32::MAX;
    let err = ready
        .try_requeued(1, 2, TaskPriority::High)
        .expect_err("Fix: requeue_count overflow must return BackendError");
    assert!(
        err.to_string().contains("requeue_count overflowed u32"),
        "Fix: requeue_count overflow errors must name the counter: {err}"
    );

    let mut ready = TaskWorkItem::from_work_item(1, 0, TaskPriority::Normal, legacy_item(1));
    ready.age_ticks = u32::MAX;
    let err = ready
        .try_requeued(1, 2, TaskPriority::High)
        .expect_err("Fix: age_ticks overflow must return BackendError");
    assert!(
        err.to_string().contains("age_ticks overflowed u32"),
        "Fix: age_ticks overflow errors must name the counter: {err}"
    );
}
