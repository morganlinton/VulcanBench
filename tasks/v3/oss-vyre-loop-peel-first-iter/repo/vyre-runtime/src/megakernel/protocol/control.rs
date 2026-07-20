/// Non-zero signals the kernel to exit on the next iteration.
pub const SHUTDOWN: u32 = 0;
/// Kernel atomic-adds 1 here every time it drains a slot.
pub const DONE_COUNT: u32 = 1;
/// Word index in `control` where the tenant-mask table begins.
pub const TENANT_BASE: u32 = 2;
/// Word index in `control` where the tenant quota table begins.
pub const TENANT_QUOTA_BASE: u32 = 32;
/// Word index in `control` where the tenant fairness counters begin.
pub const TENANT_FAIRNESS_BASE: u32 = 64;
/// Number of tenant fairness counters reserved in the control buffer.
pub const TENANT_FAIRNESS_SLOTS: u32 = 32;
/// Metrics region start. Per-opcode execution counters live here.
pub const METRICS_BASE: u32 = TENANT_FAIRNESS_BASE + TENANT_FAIRNESS_SLOTS;
/// Total number of tracked opcode metric slots.
pub const METRICS_SLOTS: u32 = 32;
/// Epoch counter; host increments on each publish batch.
pub const EPOCH: u32 = METRICS_BASE + METRICS_SLOTS;
/// Word index in `control` where priority partition offsets begin.
pub const PRIORITY_OFFSETS_BASE: u32 = EPOCH + 1;
/// Number of priority partition offset words, including sentinel.
pub const PRIORITY_OFFSETS_SLOTS: u32 = 6;
/// Starvation counter word used by the priority scheduler.
pub const PRIORITY_STARVATION_COUNTER: u32 = PRIORITY_OFFSETS_BASE + PRIORITY_OFFSETS_SLOTS;
/// Word index in `control` where per-priority fairness counters begin.
pub const PRIORITY_FAIRNESS_BASE: u32 = PRIORITY_STARVATION_COUNTER + 1;
/// Number of priority fairness counters reserved in the control buffer.
pub const PRIORITY_FAIRNESS_SLOTS: u32 = 5;
/// First observable result word; opcodes write user-visible results here.
pub const OBSERVABLE_BASE: u32 = 160;

const _: () = {
    assert!(TENANT_BASE > DONE_COUNT);
    assert!(TENANT_QUOTA_BASE > TENANT_BASE);
    assert!(TENANT_FAIRNESS_BASE > TENANT_QUOTA_BASE);
    assert!(METRICS_BASE >= TENANT_FAIRNESS_BASE + TENANT_FAIRNESS_SLOTS);
    assert!(EPOCH >= METRICS_BASE + METRICS_SLOTS);
    assert!(PRIORITY_OFFSETS_BASE > EPOCH);
    assert!(PRIORITY_STARVATION_COUNTER >= PRIORITY_OFFSETS_BASE + PRIORITY_OFFSETS_SLOTS);
    assert!(PRIORITY_FAIRNESS_BASE > PRIORITY_STARVATION_COUNTER);
    assert!(OBSERVABLE_BASE > PRIORITY_FAIRNESS_BASE + PRIORITY_FAIRNESS_SLOTS);
};
