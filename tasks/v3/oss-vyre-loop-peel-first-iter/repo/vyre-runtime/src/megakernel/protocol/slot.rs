/// The slot is free; the host may publish into it.
pub const EMPTY: u32 = 0;
/// The host finished writing; the GPU may claim it.
pub const PUBLISHED: u32 = 1;
/// A lane won the CAS; it now owns the slot.
pub const CLAIMED: u32 = 2;
/// The lane finished executing; the host may recycle the slot.
pub const DONE: u32 = 3;
/// The slot is waiting for an asynchronous IO continuation.
pub const WAIT_IO: u32 = 4;
/// The slot yielded execution back to the scheduler.
pub const YIELD: u32 = 5;
/// The slot is heavily contested and has been requeued.
pub const REQUEUE: u32 = 6;
/// The slot hit a hardware or software fault constraint.
pub const FAULT: u32 = 7;
/// Priority: critical; kernel checks these slots first on each iteration.
pub const PRIORITY_CRITICAL: u32 = 0;
/// Priority: high-priority foreground work.
pub const PRIORITY_HIGH: u32 = 1;
/// Priority: normal scheduling (default).
pub const PRIORITY_NORMAL: u32 = 2;
/// Priority: low-priority background work.
pub const PRIORITY_LOW: u32 = 3;
/// Priority: idle maintenance work.
pub const PRIORITY_IDLE: u32 = 4;
