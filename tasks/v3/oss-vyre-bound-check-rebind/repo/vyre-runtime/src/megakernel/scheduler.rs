//! Work scheduler  -  priority-aware slot scanning for the persistent megakernel.
//!
//! Extends the base slot-claim logic with priority partitioning:
//! each priority level occupies a contiguous partition of the ring buffer.
//! Workers scan from highest priority (0=CRITICAL) to lowest (4=IDLE),
//! claiming the first PUBLISHED slot found. This ensures latency-sensitive
//! work is processed before background tasks without true preemption.
//!
//! ## Slot Layout Extension
//!
//! The priority is encoded in `ring_buffer[slot_base + PRIORITY_WORD]`.
//! The host sets this when publishing; the scheduler reads it to
//! sort work into the right scan order.
//!
//! ## Starvation Guard
//!
//! After `STARVATION_THRESHOLD` consecutive high-priority claims, the
//! scheduler forcibly scans lower-priority partitions for one iteration.
//! This prevents priority inversion where a flood of CRITICAL slots
//! starves NORMAL/"background" work indefinitely.

use super::ir_util::{atomic_load_relaxed, atomic_store_relaxed};
use super::protocol::*;
use vyre_foundation::ir::{Expr, Node};

mod offsets;
pub use offsets::{
    default_priority_offsets, default_priority_offsets_array, try_default_priority_offsets,
    write_default_priority_offsets,
};

/// Number of priority levels the scheduler supports.
pub const PRIORITY_LEVELS: u32 = 5;

/// Priority discriminants.
pub mod priority {
    /// Highest priority  -  interactive/latency-critical work.
    pub const CRITICAL: u32 = 0;
    /// High priority  -  important but not latency-critical.
    pub const HIGH: u32 = 1;
    /// Normal priority  -  the default for all work.
    pub const NORMAL: u32 = 2;
    /// Low priority  -  background, non-urgent work.
    pub const LOW: u32 = 3;
    /// Idle priority  -  processed only when no other work exists.
    pub const IDLE: u32 = 4;
}

/// After this many consecutive claims at the same (or higher) priority,
/// the scheduler forcibly scans lower-priority partitions for one iteration.
pub const STARVATION_THRESHOLD: u32 = 16;

/// After this many claims by a single tenant in a single worker's "epoch",
/// the tenant is considered "greedy" and may be throttled.
pub const TENANT_FAIRNESS_THRESHOLD: u32 = 64;

/// Control word storing the priority partition offsets.
/// `control[PRIORITY_OFFSETS_BASE + pri]` = first slot index for priority `pri`.
/// `control[PRIORITY_OFFSETS_BASE + PRIORITY_LEVELS]` = total slot count (sentinel).
pub const PRIORITY_OFFSETS_BASE: u32 = control::PRIORITY_OFFSETS_BASE;

/// Control word storing consecutive high-priority claims.
pub const PRIORITY_STARVATION_COUNTER: u32 = control::PRIORITY_STARVATION_COUNTER;

/// Policy helper: select the next slot to probe within a partition.
///
/// Offsetting the start by `lane_id` reduces CAS contention on the first
/// few slots of a partition when many workers wake up simultaneously.
#[must_use]
pub fn policy_offset_start(partition_start: Expr, partition_end: Expr, lane_id: Expr) -> Expr {
    let range = Expr::sub(partition_end.clone(), partition_start.clone());
    let nonzero_range = Expr::max(range, Expr::u32(1));
    Expr::add(partition_start, Expr::rem(lane_id, nonzero_range))
}

/// Number of strided probes each lane needs to cover a priority partition.
///
/// The scheduler has `worker_width` lanes scanning one partition in lockstep.
/// Bounding this as a ceiling division keeps the generated scan work linear in
/// slot count instead of priority_levels * total_slots.
#[must_use]
pub fn priority_partition_probe_count(partition_slots: u32, worker_width: u32) -> u32 {
    if partition_slots == 0 {
        return 0;
    }
    let width = worker_width.max(1);
    partition_slots.div_ceil(width)
}

/// Number of lanes that should actively probe one priority partition.
///
/// Lanes outside `partition_slots` cannot discover additional work when the
/// worker set is wider than the partition; masking them avoids duplicate slot
/// probes across every priority band.
#[must_use]
pub fn priority_partition_active_lane_count(partition_slots: u32, worker_width: u32) -> u32 {
    partition_slots.min(worker_width.max(1))
}

/// Upper bound on slot status probes for one priority partition.
#[must_use]
pub fn priority_partition_probe_budget(partition_slots: u32, worker_width: u32) -> u32 {
    priority_partition_active_lane_count(partition_slots, worker_width)
        .saturating_mul(priority_partition_probe_count(partition_slots, worker_width))
}

/// Policy helper: check if a tenant has exceeded its fairness quota.
#[must_use]
pub fn check_tenant_fairness(tenant_id: Expr) -> Expr {
    let tenant_counter = Expr::rem(tenant_id, Expr::u32(control::TENANT_FAIRNESS_SLOTS));
    let count = atomic_load_relaxed(
        "control",
        Expr::add(Expr::u32(control::TENANT_FAIRNESS_BASE), tenant_counter),
    );
    Expr::lt(count, Expr::u32(TENANT_FAIRNESS_THRESHOLD))
}

/// Policy helper: check if a priority level has exceeded its fairness quota.
#[must_use]
pub fn check_priority_fairness(priority: Expr) -> Expr {
    let count = atomic_load_relaxed(
        "control",
        Expr::add(Expr::u32(control::PRIORITY_FAIRNESS_BASE), priority),
    );
    Expr::lt(count, Expr::u32(STARVATION_THRESHOLD))
}

/// Build the priority-aware scan loop as `Vec<Node>` for composition.
///
/// The scan checks priorities from `start_priority` to `PRIORITY_LEVELS - 1`.
/// For each priority level, it scans the corresponding ring partition
/// for a PUBLISHED slot. If found, claims it via CAS and yields
/// the slot base to the caller.
///
/// Variables set on success:
/// - `claimed_slot_base`: the slot_base of the claimed slot (u32::MAX if none found)
/// - `claimed_priority`: the priority level of the claimed slot
/// - `claimed_tenant`: the tenant id of the claimed slot
///
/// Requires `lane_id` and `workgroup_size_x` in scope.
#[must_use]
pub fn priority_scan_body(total_slots: u32) -> Vec<Node> {
    priority_scan_body_with_stride(total_slots, total_slots.max(1))
}

/// Build the priority-aware scan loop with an explicit global worker stride.
///
/// Each lane probes its own congruence class inside each priority partition.
/// Across all launched workers this changes the scan from every worker probing
/// every slot to the worker set covering the partition once per priority pass.
#[must_use]
pub fn priority_scan_body_with_stride(total_slots: u32, worker_stride: u32) -> Vec<Node> {
    let worker_stride = worker_stride.max(1);
    vec![
        // Initialize output: no slot claimed
        Node::let_bind("claimed_slot_base", Expr::u32(u32::MAX)),
        Node::let_bind("claimed_priority", Expr::u32(u32::MAX)),
        Node::let_bind("claimed_tenant", Expr::u32(u32::MAX)),
        Node::let_bind(
            "priority_starvation_count",
            atomic_load_relaxed("control", Expr::u32(PRIORITY_STARVATION_COUNTER)),
        ),
        Node::let_bind(
            "priority_force_lower",
            Expr::ge(
                Expr::var("priority_starvation_count"),
                Expr::u32(STARVATION_THRESHOLD),
            ),
        ),
        // Scan each priority level in order
        Node::loop_for(
            "scan_pri",
            Expr::u32(0),
            Expr::u32(PRIORITY_LEVELS),
            vec![
                // Skip if we already claimed a slot
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("claimed_slot_base"), Expr::u32(u32::MAX)),
                        Expr::or(
                            Expr::not(Expr::var("priority_force_lower")),
                            Expr::gt(Expr::var("scan_pri"), Expr::u32(priority::HIGH)),
                        ),
                    ),
                    vec![
                        // Load partition boundaries from control buffer
                        Node::let_bind(
                            "part_start",
                            atomic_load_relaxed(
                                "control",
                                Expr::add(Expr::u32(PRIORITY_OFFSETS_BASE), Expr::var("scan_pri")),
                            ),
                        ),
                        Node::let_bind(
                            "part_end",
                            atomic_load_relaxed(
                                "control",
                                Expr::add(
                                    Expr::u32(PRIORITY_OFFSETS_BASE),
                                    Expr::add(Expr::var("scan_pri"), Expr::u32(1)),
                                ),
                            ),
                        ),
                        Node::let_bind(
                            "part_len",
                            Expr::sub(Expr::var("part_end"), Expr::var("part_start")),
                        ),
                        Node::let_bind(
                            "probe_count",
                            Expr::div(
                                Expr::add(
                                    Expr::var("part_len"),
                                    Expr::u32(worker_stride.saturating_sub(1)),
                                ),
                                Expr::u32(worker_stride),
                            ),
                        ),
                        // Scan slots within this priority partition
                        Node::if_then(
                            Expr::gt(Expr::var("part_len"), Expr::u32(0)),
                            vec![
                                Node::let_bind(
                                    "partition_lane",
                                    Expr::rem(Expr::var("lane_id"), Expr::u32(worker_stride)),
                                ),
                                Node::if_then(
                                    Expr::lt(Expr::var("partition_lane"), Expr::var("part_len")),
                                    vec![Node::loop_for(
                                        "scan_idx",
                                        Expr::u32(0),
                                        Expr::var("probe_count"),
                                        vec![
                                            Node::let_bind(
                                                "scan_slot",
                                                Expr::add(
                                                    Expr::var("part_start"),
                                                    Expr::rem(
                                                        Expr::add(
                                                            Expr::var("partition_lane"),
                                                            Expr::mul(
                                                                Expr::var("scan_idx"),
                                                                Expr::u32(worker_stride),
                                                            ),
                                                        ),
                                                        Expr::var("part_len"),
                                                    ),
                                                ),
                                            ),
                                            Node::if_then(
                                                Expr::and(
                                                    Expr::eq(
                                                        Expr::var("claimed_slot_base"),
                                                        Expr::u32(u32::MAX),
                                                    ),
                                                    Expr::lt(
                                                        Expr::var("scan_slot"),
                                                        Expr::u32(total_slots),
                                                    ),
                                                ),
                                                vec![
                                                    Node::let_bind(
                                                        "probe_base",
                                                        Expr::mul(
                                                            Expr::var("scan_slot"),
                                                            Expr::u32(SLOT_WORDS),
                                                        ),
                                                    ),
                                                    Node::let_bind(
                                                        "probe_status",
                                                        atomic_load_relaxed(
                                                            "ring_buffer",
                                                            Expr::var("probe_base"),
                                                        ),
                                                    ),
                                                    Node::let_bind(
                                                        "probe_schedulable",
                                                        Expr::or(
                                                            Expr::eq(
                                                                Expr::var("probe_status"),
                                                                Expr::u32(slot::PUBLISHED),
                                                            ),
                                                            Expr::or(
                                                                Expr::eq(
                                                                    Expr::var("probe_status"),
                                                                    Expr::u32(slot::YIELD),
                                                                ),
                                                                Expr::eq(
                                                                    Expr::var("probe_status"),
                                                                    Expr::u32(slot::REQUEUE),
                                                                ),
                                                            ),
                                                        ),
                                                    ),
                                                    Node::if_then(
                                                        Expr::var("probe_schedulable"),
                                                        vec![
                                                            Node::let_bind(
                                                                "probe_tenant",
                                                                Expr::load(
                                                                    "ring_buffer",
                                                                    Expr::add(
                                                                        Expr::var("probe_base"),
                                                                        Expr::u32(TENANT_WORD),
                                                                    ),
                                                                ),
                                                            ),
                                                            Node::let_bind(
                                                                "probe_tenant_base",
                                                                atomic_load_relaxed(
                                                                    "control",
                                                                    Expr::u32(
                                                                        control::TENANT_BASE,
                                                                    ),
                                                                ),
                                                            ),
                                                            Node::let_bind(
                                                                "probe_tenant_mask",
                                                                atomic_load_relaxed(
                                                                    "control",
                                                                    Expr::add(
                                                                        Expr::var(
                                                                            "probe_tenant_base",
                                                                        ),
                                                                        Expr::var("probe_tenant"),
                                                                    ),
                                                                ),
                                                            ),
                                                            Node::if_then(
                                                                Expr::ne(
                                                                    Expr::var(
                                                                        "probe_tenant_mask",
                                                                    ),
                                                                    Expr::u32(0),
                                                                ),
                                                                vec![
                                                                    Node::let_bind(
                                                                        "probe_expected",
                                                                        Expr::var("probe_status"),
                                                                    ),
                                                                    Node::let_bind(
                                                                        "probe_prev",
                                                                        Expr::atomic_compare_exchange(
                                                                            "ring_buffer",
                                                                            Expr::var("probe_base"),
                                                                            Expr::var("probe_expected"),
                                                                            Expr::u32(slot::CLAIMED),
                                                                        ),
                                                                    ),
                                                                    Node::if_then(
                                                                        Expr::eq(
                                                                            Expr::var("probe_prev"),
                                                                            Expr::var("probe_expected"),
                                                                        ),
                                                                        vec![
                                                                    Node::assign(
                                                                        "claimed_slot_base",
                                                                        Expr::var("probe_base"),
                                                                    ),
                                                                    Node::assign(
                                                                        "claimed_priority",
                                                                        Expr::var("scan_pri"),
                                                                    ),
                                                                    Node::assign(
                                                                        "claimed_tenant",
                                                                        Expr::var("probe_tenant"),
                                                                    ),
                                                                ],
                                                                    ),
                                                                ],
                                                            ),
                                                        ],
                                                    ),
                                                ],
                                            ),
                                        ],
                                    )],
                                ),
                            ],
                        ),
                    ],
                ),
            ],
        ),
        // Post-claim: Update fairness accounting
        Node::if_then(
            Expr::ne(Expr::var("claimed_priority"), Expr::u32(u32::MAX)),
            vec![
                // Update priority starvation counter atomically
                Node::if_then_else(
                    Expr::le(Expr::var("claimed_priority"), Expr::u32(priority::HIGH)),
                    vec![Node::let_bind(
                        "priority_starvation_prev",
                        Expr::atomic_add(
                            "control",
                            Expr::u32(PRIORITY_STARVATION_COUNTER),
                            Expr::u32(1),
                        ),
                    )],
                    vec![atomic_store_relaxed(
                        "priority_starvation_prev",
                        "control",
                        Expr::u32(PRIORITY_STARVATION_COUNTER),
                        Expr::u32(0),
                    )],
                ),
                // Update per-tenant fairness counter
                Node::let_bind(
                    "tenant_fairness_prev",
                    Expr::atomic_add(
                        "control",
                        Expr::add(
                            Expr::u32(control::TENANT_FAIRNESS_BASE),
                            Expr::rem(
                                Expr::var("claimed_tenant"),
                                Expr::u32(control::TENANT_FAIRNESS_SLOTS),
                            ),
                        ),
                        Expr::u32(1),
                    ),
                ),
                // Update per-priority fairness counter (telemetry)
                Node::let_bind(
                    "priority_fairness_prev",
                    Expr::atomic_add(
                        "control",
                        Expr::add(
                            Expr::u32(control::PRIORITY_FAIRNESS_BASE),
                            Expr::var("claimed_priority"),
                        ),
                        Expr::u32(1),
                    ),
                ),
            ],
        ),
    ]
}

#[cfg(test)]
mod tests;
