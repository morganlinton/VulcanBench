//! Multi-tenant megakernel multiplexing.
//!
//! A single persistent megakernel per GPU can service many producer
//! tools without each one paying the dispatch-setup cost. The
//! `tenant_id` field already lives in the ring-slot protocol
//! (`protocol::TENANT_WORD`); this module owns the host-side
//! bookkeeping that hands each producer a stable id, reserves an
//! opcode-range per producer, and gates publish operations against a
//! per-tenant mask so one producer cannot accidentally drive another
//! producer's opcodes.
//!
//! ## Tenants and opcodes
//!
//! Every tenant owns an opcode range `[base, base + cap)` where the
//! whole range sits inside the user-extension space reserved by
//! `vyre_runtime::megakernel::protocol::opcode` (≥ `0x4000_0000`).
//! When [`TenantRegistry::register`] returns a [`TenantHandle`],
//! callers publish into slot args `[rule_local_opcode, ...]` and
//! the registry maps that to `(tenant_base + rule_local_opcode)`
//! before writing into the ring. A tenant that tries to publish an
//! opcode outside its own range fails with a structured error.
//!
//! ## Draining
//!
//! Unregistering a tenant revokes future publishes but does NOT
//! revoke in-flight slots  -  the GPU is still going to execute any
//! slot it already CAS-claimed. Callers that need hard draining
//! drive [`TenantHandle::quiesce`] which spins on the megakernel
//! DONE_COUNT until every slot the tenant published has been
//! acknowledged.
//!
//! ## Daemon surface
//!
//! The registry is the reusable piece. A full `MegakernelDaemon`
//! (listening on a Unix socket, vending handles over RPC) is a thin
//! wrapper that we can ship alongside the runtime  -  the registry
//! here already handles the interesting concurrency.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;

use crate::megakernel::protocol::opcode::SHUTDOWN;
use crate::megakernel::Megakernel;
use crate::PipelineError;

/// First opcode the tenant registry hands out. Sits inside the
/// user-extension range reserved by the megakernel protocol so fused
/// rule documents compose with tenant allocation without colliding
/// with built-in opcodes.
pub const TENANT_OPCODE_BASE: u32 = 0x4000_0000;

/// Upper bound on the tenant-id space. `tenant_id == TENANT_ID_MAX`
/// is reserved as an invalid / revoked sentinel.
pub const TENANT_ID_MAX: u32 = u32::MAX - 1;

/// Size of the opcode window reserved per tenant. 1 << 20 = 1 MiB
/// of opcodes  -  well over any realistic rule count per producer
/// while still allowing ~4094 simultaneous tenants inside the u32
/// opcode range.
pub const OPCODE_RANGE_PER_TENANT: u32 = 1 << 20;

const QUIESCE_SPIN_POLLS: u64 = 64;
const QUIESCE_MIN_PARK: Duration = Duration::from_micros(2);
const QUIESCE_MAX_PARK: Duration = Duration::from_micros(50);
const QUIESCE_BACKOFF_SHIFT_CAP: u64 = 5;

#[allow(clippy::unnecessary_min_or_max)]
fn quiesce_backoff_duration(poll: u64) -> Duration {
    let parked_poll = poll.checked_sub(QUIESCE_SPIN_POLLS).unwrap_or(0);
    let shift = parked_poll.min(QUIESCE_BACKOFF_SHIFT_CAP) as u32;
    let multiplier = 1_u32 << shift;
    QUIESCE_MIN_PARK
        .checked_mul(multiplier)
        .unwrap_or(QUIESCE_MAX_PARK)
        .min(QUIESCE_MAX_PARK)
}

fn quiesce_idle(poll: u64) {
    if poll < QUIESCE_SPIN_POLLS {
        std::hint::spin_loop();
    } else {
        std::thread::park_timeout(quiesce_backoff_duration(poll));
    }
}

fn tenant_registry_retry_idle(retry: u64) {
    if retry < QUIESCE_SPIN_POLLS {
        std::hint::spin_loop();
    } else {
        std::thread::park_timeout(quiesce_backoff_duration(retry));
    }
}

/// Errors surfaced by the tenant registry.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TenantError {
    /// The registry ran out of tenant ids. Unregister unused tenants
    /// or raise the range per tenant.
    #[error("tenant registry exhausted after {issued} registrations. Fix: shrink OPCODE_RANGE_PER_TENANT or recycle tenants.")]
    RegistryFull {
        /// Number of tenants already issued when exhaustion hit.
        issued: u32,
    },
    /// Tried to publish an opcode outside the tenant's reserved
    /// range. Almost always a caller bug.
    #[error(
        "tenant {tenant_id} published local opcode {local_opcode}; out of range [0, {cap}). \
         Fix: caller must stay inside the opcode window returned by `register()`."
    )]
    OpcodeOutOfRange {
        /// Tenant id that tripped.
        tenant_id: u32,
        /// Local opcode the caller supplied.
        local_opcode: u32,
        /// Cap on the tenant's local opcode range.
        cap: u32,
    },
    /// Tenant was unregistered concurrently; its handle is stale.
    #[error("tenant {tenant_id} was revoked; handle is stale. Fix: acquire a fresh handle from the registry.")]
    Revoked {
        /// Tenant id that was revoked.
        tenant_id: u32,
    },
    /// Quiesce timed out with inflight slots still outstanding.
    #[error(
        "tenant {tenant_id} quiesce timed out with {outstanding} inflight slots. \
         Fix: ensure the megakernel is making progress (check DONE_COUNT) or raise the timeout."
    )]
    QuiesceTimeout {
        /// Tenant id whose quiesce tripped.
        tenant_id: u32,
        /// Number of slots still inflight at timeout.
        outstanding: u64,
    },
    /// Tenant has reached its configured outstanding-slot cap.
    #[error(
        "tenant {tenant_id} has {outstanding} outstanding slots, cap {cap}. \
         Fix: wait for drain progress or register the tenant with a larger bounded backlog."
    )]
    Backpressure {
        /// Tenant id whose backlog is full.
        tenant_id: u32,
        /// Current host-visible outstanding slots.
        outstanding: u64,
        /// Configured outstanding-slot cap.
        cap: u64,
    },
    /// Tenant has reached its configured staging-byte cap.
    #[error(
        "tenant {tenant_id} requested {requested} staging bytes with {used} already reserved, cap {cap}. \
         Fix: release staging reservations after publish/readback progress or register the tenant with a larger bounded staging budget."
    )]
    StagingBackpressure {
        /// Tenant id whose staging byte budget is full.
        tenant_id: u32,
        /// New bytes requested.
        requested: u64,
        /// Current reserved staging bytes.
        used: u64,
        /// Configured staging byte cap.
        cap: u64,
    },
    /// Tenant has reached its configured resident-handle cap.
    #[error(
        "tenant {tenant_id} requested {requested} resident handles with {used} already reserved, cap {cap}. \
         Fix: release resident handles when backend ownership ends or register the tenant with a larger bounded resident-handle budget."
    )]
    ResidentHandleBackpressure {
        /// Tenant id whose resident handle budget is full.
        tenant_id: u32,
        /// New handles requested.
        requested: u64,
        /// Current reserved resident handles.
        used: u64,
        /// Configured resident handle cap.
        cap: u64,
    },
    /// Tenant resource accounting would underflow.
    #[error(
        "tenant {tenant_id} released {requested} {resource} with only {used} reserved. \
         Fix: pair every tenant resource release with a successful reservation."
    )]
    ResourceUnderflow {
        /// Tenant id whose counter would underflow.
        tenant_id: u32,
        /// Resource counter being released.
        resource: &'static str,
        /// Release count requested.
        requested: u64,
        /// Current reserved count.
        used: u64,
    },
    /// Protocol error bubbled up from `Megakernel::publish_slot`.
    #[error("{0}")]
    Pipeline(#[from] PipelineError),
}

/// Per-tenant resource quota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TenantQuota {
    /// Maximum host-visible ring slots the tenant may keep outstanding.
    pub max_outstanding_slots: u64,
    /// Maximum staging bytes the tenant may reserve for pending work.
    pub max_staging_bytes: u64,
    /// Maximum resident handles the tenant may hold at once.
    pub max_resident_handles: u64,
}

impl TenantQuota {
    /// Unbounded tenant quota for compatibility with the legacy registration
    /// API. Individual fields are still normalized to at least one resource
    /// slot during registration.
    #[must_use]
    pub const fn unbounded() -> Self {
        Self {
            max_outstanding_slots: u64::MAX,
            max_staging_bytes: u64::MAX,
            max_resident_handles: u64::MAX,
        }
    }

    /// Build a bounded tenant quota.
    #[must_use]
    pub const fn bounded(
        max_outstanding_slots: u64,
        max_staging_bytes: u64,
        max_resident_handles: u64,
    ) -> Self {
        Self {
            max_outstanding_slots,
            max_staging_bytes,
            max_resident_handles,
        }
    }
}

/// One tenant's accounting state. Lives inside an `Arc` so handles
/// stay valid after the registry borrow drops.
struct TenantState {
    id: u32,
    base_opcode: u32,
    opcode_cap: u32,
    /// Number of slots this tenant has ever published.
    published_count: AtomicU64,
    /// Maximum host-visible slots this tenant may keep outstanding.
    max_outstanding_slots: u64,
    /// Number of staging bytes currently reserved by this tenant.
    staging_bytes: AtomicU64,
    /// Maximum staging bytes this tenant may reserve.
    max_staging_bytes: u64,
    /// Number of resident handles currently reserved by this tenant.
    resident_handles: AtomicU64,
    /// Maximum resident handles this tenant may reserve.
    max_resident_handles: u64,
    /// Number of slots the GPU has reported DONE for this tenant.
    /// Advanced by [`TenantHandle::note_drained`].
    drained_count: AtomicU64,
    /// Number of quiesce calls completed or timed out for this tenant.
    quiesce_calls: AtomicU64,
    /// Number of quiesce calls that timed out before the tenant drained.
    quiesce_timeouts: AtomicU64,
    /// Cumulative host-observed drain wait across quiesce calls.
    quiesce_wait_ns: AtomicU64,
    /// Set to 1 on `unregister`; publishes reject afterwards.
    revoked: AtomicU32,
    /// Stable label for diagnostics (for example, `"scanner-a"`, `"scanner-b"`).
    label: String,
}

/// Stable handle returned by [`TenantRegistry::register`]. Clones
/// share the same underlying state, so multiple producer threads
/// inside one tenant can publish through their own handles.
#[derive(Clone)]
pub struct TenantHandle {
    state: Arc<TenantState>,
}

/// Host-visible tenant runtime counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TenantRuntimeCounters {
    /// Tenant id.
    pub tenant_id: u32,
    /// Number of slots ever published by this tenant.
    pub published_count: u64,
    /// Number of slots observed drained for this tenant.
    pub drained_count: u64,
    /// Current host-visible backlog (`published_count - drained_count`).
    pub outstanding_slots: u64,
    /// Configured outstanding-slot cap for this tenant.
    pub max_outstanding_slots: u64,
    /// Number of quiesce calls recorded for this tenant.
    pub quiesce_calls: u64,
    /// Number of quiesce calls that timed out.
    pub quiesce_timeouts: u64,
    /// Cumulative nanoseconds spent waiting for this tenant to drain.
    pub quiesce_wait_ns: u64,
}

/// Host-visible tenant quota counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TenantQuotaCounters {
    /// Tenant id.
    pub tenant_id: u32,
    /// Current reserved staging bytes.
    pub staging_bytes: u64,
    /// Configured staging byte cap.
    pub max_staging_bytes: u64,
    /// Current reserved resident handle count.
    pub resident_handles: u64,
    /// Configured resident handle cap.
    pub max_resident_handles: u64,
}

impl std::fmt::Debug for TenantHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TenantHandle")
            .field("id", &self.state.id)
            .field("label", &self.state.label)
            .field("base_opcode", &self.state.base_opcode)
            .field(
                "published_count",
                &self.state.published_count.load(Ordering::Relaxed),
            )
            .field("max_outstanding_slots", &self.state.max_outstanding_slots)
            .field(
                "staging_bytes",
                &self.state.staging_bytes.load(Ordering::Relaxed),
            )
            .field("max_staging_bytes", &self.state.max_staging_bytes)
            .field(
                "resident_handles",
                &self.state.resident_handles.load(Ordering::Relaxed),
            )
            .field("max_resident_handles", &self.state.max_resident_handles)
            .field(
                "drained_count",
                &self.state.drained_count.load(Ordering::Relaxed),
            )
            .field(
                "revoked",
                &(self.state.revoked.load(Ordering::Acquire) != 0),
            )
            .finish()
    }
}

impl TenantHandle {
    /// Stable tenant id; maps onto the ring-slot `TENANT_WORD`.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.state.id
    }

    /// Human-readable label supplied at registration time.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.state.label
    }

    /// First opcode this tenant owns.
    #[must_use]
    pub fn base_opcode(&self) -> u32 {
        self.state.base_opcode
    }

    /// Convert a tenant-local opcode to the global opcode used in
    /// the ring slot. Caller enforces `local < opcode_cap()`.
    ///
    /// # Errors
    ///
    /// Returns [`TenantError::OpcodeOutOfRange`] when the local
    /// value is outside the reserved window.
    pub fn global_opcode(&self, local: u32) -> Result<u32, TenantError> {
        self.ensure_not_revoked()?;
        if local >= self.state.opcode_cap {
            return Err(TenantError::OpcodeOutOfRange {
                tenant_id: self.id(),
                local_opcode: local,
                cap: self.state.opcode_cap,
            });
        }
        let global = self.state.base_opcode + local;
        if let Err(e) = crate::megakernel::protocol::opcode::validate_user_opcode(global) {
            return Err(TenantError::Pipeline(PipelineError::Backend(format!(
                "tenant registry produced invalid global opcode {global}: {e}. Fix: repair tenant opcode window allocation before publishing."
            ))));
        }
        Ok(global)
    }

    /// Publish a slot into the tenant's ring with a tenant-local
    /// opcode. Convenience wrapper that composes
    /// [`Megakernel::publish_slot`] with tenant bookkeeping.
    ///
    /// # Errors
    ///
    /// - [`TenantError::Revoked`] if the tenant was unregistered.
    /// - [`TenantError::OpcodeOutOfRange`] if `local_opcode` is
    ///   outside the tenant's window.
    /// - [`TenantError::Pipeline`] when the underlying
    ///   `publish_slot` rejects (e.g., slot still in-flight).
    pub fn publish_slot(
        &self,
        ring_bytes: &mut [u8],
        slot_idx: u32,
        local_opcode: u32,
        args: &[u32],
    ) -> Result<(), TenantError> {
        self.ensure_not_revoked()?;
        let global = self.global_opcode(local_opcode)?;
        self.reserve_publish_slot()?;
        if let Err(error) =
            Megakernel::publish_slot(ring_bytes, slot_idx, self.state.id, global, args)
        {
            saturating_atomic_sub_u64(&self.state.published_count, 1, "tenant published rollback");
            return Err(error.into());
        }
        Ok(())
    }

    fn ensure_not_revoked(&self) -> Result<(), TenantError> {
        if self.state.revoked.load(Ordering::Acquire) != 0 {
            return Err(TenantError::Revoked {
                tenant_id: self.state.id,
            });
        }
        Ok(())
    }

    fn reserve_publish_slot(&self) -> Result<(), TenantError> {
        let cap = self.state.max_outstanding_slots;
        vyre_driver::accounting::checked_atomic_update_u64_with_order(
            &self.state.published_count,
            Ordering::Acquire,
            Ordering::AcqRel,
            Ordering::Acquire,
            |published| {
                let drained = self.state.drained_count.load(Ordering::Acquire);
                let outstanding = vyre_driver::accounting::checked_sub_u64_lazy(
                    published,
                    drained,
                    || {
                        TenantError::Pipeline(PipelineError::QueueFull {
                            queue: "tenant",
                            fix: "tenant drained_count exceeded published_count; rebuild tenant accounting state",
                        })
                    },
                )?;
                if outstanding >= cap {
                    return Err(TenantError::Backpressure {
                        tenant_id: self.state.id,
                        outstanding,
                        cap,
                    });
                }
                vyre_driver::accounting::checked_add_u64_lazy(published, 1, || {
                    TenantError::Pipeline(PipelineError::QueueFull {
                        queue: "tenant",
                        fix: "tenant published_count overflowed u64; quiesce or recreate the tenant before publishing more slots",
                    })
                })
            },
            |_, _| Ok(()),
        )?;
        Ok(())
    }

    /// Number of slots this tenant has ever published.
    #[must_use]
    pub fn published_count(&self) -> u64 {
        self.state.published_count.load(Ordering::Relaxed)
    }

    /// Number of slots this tenant has observed drained (via
    /// [`note_drained`](Self::note_drained)).
    #[must_use]
    pub fn drained_count(&self) -> u64 {
        self.state.drained_count.load(Ordering::Relaxed)
    }

    /// Maximum host-visible slots this tenant may keep outstanding.
    #[must_use]
    pub fn max_outstanding_slots(&self) -> u64 {
        self.state.max_outstanding_slots
    }

    /// Reserve staging bytes against this tenant's quota.
    pub fn reserve_staging_bytes(&self, byte_count: u64) -> Result<(), TenantError> {
        self.ensure_not_revoked()?;
        reserve_resource_quota(
            &self.state.staging_bytes,
            byte_count,
            self.state.max_staging_bytes,
            || {
                TenantError::StagingBackpressure {
                    tenant_id: self.state.id,
                    requested: byte_count,
                    used: self.state.staging_bytes.load(Ordering::Acquire),
                    cap: self.state.max_staging_bytes,
                }
            },
            "tenant staging byte reservation overflowed u64; release staging reservations or recreate the tenant before reserving more bytes",
        )
    }

    /// Release staging bytes previously reserved by this tenant.
    pub fn release_staging_bytes(&self, byte_count: u64) -> Result<(), TenantError> {
        release_resource_quota(
            &self.state.staging_bytes,
            byte_count,
            self.state.id,
            "staging bytes",
        )
    }

    /// Reserve resident handles against this tenant's quota.
    pub fn reserve_resident_handles(&self, handle_count: u64) -> Result<(), TenantError> {
        self.ensure_not_revoked()?;
        reserve_resource_quota(
            &self.state.resident_handles,
            handle_count,
            self.state.max_resident_handles,
            || {
                TenantError::ResidentHandleBackpressure {
                    tenant_id: self.state.id,
                    requested: handle_count,
                    used: self.state.resident_handles.load(Ordering::Acquire),
                    cap: self.state.max_resident_handles,
                }
            },
            "tenant resident handle reservation overflowed u64; release resident handles or recreate the tenant before reserving more handles",
        )
    }

    /// Release resident handles previously reserved by this tenant.
    pub fn release_resident_handles(&self, handle_count: u64) -> Result<(), TenantError> {
        release_resource_quota(
            &self.state.resident_handles,
            handle_count,
            self.state.id,
            "resident handles",
        )
    }

    /// Snapshot quota counters for this tenant.
    #[must_use]
    pub fn quota_counters(&self) -> TenantQuotaCounters {
        TenantQuotaCounters {
            tenant_id: self.state.id,
            staging_bytes: self.state.staging_bytes.load(Ordering::Acquire),
            max_staging_bytes: self.state.max_staging_bytes,
            resident_handles: self.state.resident_handles.load(Ordering::Acquire),
            max_resident_handles: self.state.max_resident_handles,
        }
    }

    fn release_all_resource_reservations(&self) {
        self.state.staging_bytes.store(0, Ordering::Release);
        self.state.resident_handles.store(0, Ordering::Release);
    }

    /// Snapshot host-visible runtime counters for this tenant.
    #[must_use]
    pub fn runtime_counters(&self) -> TenantRuntimeCounters {
        let published_count = self.state.published_count.load(Ordering::Acquire);
        let drained_count = self.state.drained_count.load(Ordering::Acquire);
        TenantRuntimeCounters {
            tenant_id: self.state.id,
            published_count,
            drained_count,
            outstanding_slots: published_count.saturating_sub(drained_count),
            max_outstanding_slots: self.state.max_outstanding_slots,
            quiesce_calls: self.state.quiesce_calls.load(Ordering::Acquire),
            quiesce_timeouts: self.state.quiesce_timeouts.load(Ordering::Acquire),
            quiesce_wait_ns: self.state.quiesce_wait_ns.load(Ordering::Acquire),
        }
    }

    /// Mark `count` slots as drained. The host pump that observes
    /// DONE_COUNT calls this when it sees the global counter
    /// advance past the tenant's last-published cursor.
    pub fn note_drained(&self, count: u64) {
        saturating_atomic_add_u64(&self.state.drained_count, count, "tenant drained_count");
    }

    /// Block-style quiesce: bounded backoff until every published
    /// slot has been drained or `max_spins` polls elapse.
    ///
    /// # Errors
    ///
    /// Returns [`TenantError::QuiesceTimeout`] when `max_spins`
    /// iterations pass without full drain. The outstanding count
    /// at timeout is included for diagnostics.
    pub fn quiesce(&self, max_spins: u64) -> Result<(), TenantError> {
        let started = Instant::now();
        for poll in 0..max_spins {
            let pub_count = self.state.published_count.load(Ordering::Acquire);
            let drained = self.state.drained_count.load(Ordering::Acquire);
            if drained >= pub_count {
                self.record_quiesce(started, false);
                return Ok(());
            }
            quiesce_idle(poll);
        }
        let pub_count = self.state.published_count.load(Ordering::Acquire);
        let drained = self.state.drained_count.load(Ordering::Acquire);
        self.record_quiesce(started, true);
        Err(TenantError::QuiesceTimeout {
            tenant_id: self.state.id,
            outstanding: vyre_driver::accounting::checked_sub_u64_lazy(pub_count, drained, || {
                TenantError::Pipeline(PipelineError::QueueFull {
                    queue: "tenant",
                    fix: "tenant drained_count exceeded published_count during quiesce; rebuild tenant accounting state",
                })
            })?,
        })
    }

    fn record_quiesce(&self, started: Instant, timed_out: bool) {
        saturating_atomic_add_u64(&self.state.quiesce_calls, 1, "tenant quiesce_calls");
        if timed_out {
            saturating_atomic_add_u64(&self.state.quiesce_timeouts, 1, "tenant quiesce_timeouts");
        }
        let elapsed_ns = match u64::try_from(started.elapsed().as_nanos()) {
            Ok(elapsed_ns) => elapsed_ns,
            Err(_) => u64::MAX,
        };
        saturating_atomic_add_u64(
            &self.state.quiesce_wait_ns,
            elapsed_ns,
            "tenant quiesce_wait_ns",
        );
    }
}

/// Thread-safe tenant registry. One per megakernel instance.

pub struct TenantRegistry {
    tenants: DashMap<u32, TenantHandle>,
    next_id: AtomicU32,
}

impl Default for TenantRegistry {
    fn default() -> Self {
        Self {
            tenants: DashMap::new(),
            next_id: AtomicU32::new(0),
        }
    }
}

/// Caller-owned scratch for repeated concurrent-tenant selection.
#[derive(Debug, Default)]
pub struct TenantSelectionScratch {
    active_ids: Vec<u32>,
    selected_indices: Vec<usize>,
}

impl TenantSelectionScratch {
    /// Construct empty tenant-selection scratch.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active_ids: Vec::new(),
            selected_indices: Vec::new(),
        }
    }
}

fn saturating_atomic_add_u64(counter: &AtomicU64, value: u64, _label: &'static str) {
    let mut current = counter.load(Ordering::Acquire);
    loop {
        let next = current.saturating_add(value);
        match counter.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return,
            Err(observed) => current = observed,
        }
    }
}

fn saturating_atomic_sub_u64(counter: &AtomicU64, value: u64, _label: &'static str) {
    let mut current = counter.load(Ordering::Acquire);
    loop {
        let next = current.saturating_sub(value);
        match counter.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return,
            Err(observed) => current = observed,
        }
    }
}

fn reserve_resource_quota(
    counter: &AtomicU64,
    value: u64,
    cap: u64,
    backpressure: impl Fn() -> TenantError,
    overflow_fix: &'static str,
) -> Result<(), TenantError> {
    vyre_driver::accounting::checked_atomic_update_u64_with_order(
        counter,
        Ordering::Acquire,
        Ordering::AcqRel,
        Ordering::Acquire,
        |used| {
            let next = vyre_driver::accounting::checked_add_u64_lazy(used, value, || {
                TenantError::Pipeline(PipelineError::QueueFull {
                    queue: "tenant resource quota",
                    fix: overflow_fix,
                })
            })?;
            if next > cap {
                return Err(backpressure());
            }
            Ok(next)
        },
        |_, _| Ok(()),
    )?;
    Ok(())
}

fn release_resource_quota(
    counter: &AtomicU64,
    value: u64,
    tenant_id: u32,
    resource: &'static str,
) -> Result<(), TenantError> {
    vyre_driver::accounting::checked_atomic_update_u64_with_order(
        counter,
        Ordering::Acquire,
        Ordering::AcqRel,
        Ordering::Acquire,
        |used| {
            used.checked_sub(value)
                .ok_or(TenantError::ResourceUnderflow {
                    tenant_id,
                    resource,
                    requested: value,
                    used,
                })
        },
        |_, _| Ok(()),
    )?;
    Ok(())
}

impl TenantRegistry {
    /// Fresh registry with no tenants.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new tenant with the given diagnostic label.
    /// Returns a handle whose opcode range is reserved until
    /// [`unregister`](Self::unregister) is called.
    ///
    /// # Errors
    ///
    /// Returns [`TenantError::RegistryFull`] when the tenant id or
    /// opcode space is exhausted.
    pub fn register(&self, label: impl Into<String>) -> Result<TenantHandle, TenantError> {
        self.register_with_backpressure(label, u64::MAX)
    }

    /// Register a new tenant with a bounded outstanding-slot budget.
    ///
    /// # Errors
    ///
    /// Returns [`TenantError::RegistryFull`] when the tenant id or opcode space
    /// is exhausted.
    pub fn register_with_backpressure(
        &self,
        label: impl Into<String>,
        max_outstanding_slots: u64,
    ) -> Result<TenantHandle, TenantError> {
        self.register_with_quotas(
            label,
            TenantQuota {
                max_outstanding_slots,
                ..TenantQuota::unbounded()
            },
        )
    }

    /// Register a tenant with explicit ring-slot, staging-byte, and
    /// resident-handle quotas.
    ///
    /// # Errors
    ///
    /// Returns [`TenantError::RegistryFull`] when the tenant id or opcode space
    /// is exhausted.
    pub fn register_with_quotas(
        &self,
        label: impl Into<String>,
        quota: TenantQuota,
    ) -> Result<TenantHandle, TenantError> {
        let mut registration_retries = 0u64;
        let issued = vyre_driver::accounting::checked_atomic_update_u32_with_order(
            &self.next_id,
            Ordering::Relaxed,
            Ordering::SeqCst,
            Ordering::Relaxed,
            |current| {
                if current >= TENANT_ID_MAX {
                    return Err(TenantError::RegistryFull { issued: current });
                }
                let id = current.max(1);
                id.checked_add(1)
                    .ok_or(TenantError::RegistryFull { issued: current })
            },
            |_, _| {
                tenant_registry_retry_idle(registration_retries);
                registration_retries = vyre_driver::accounting::checked_add_u64_lazy(
                    registration_retries,
                    1,
                    || {
                        TenantError::Pipeline(PipelineError::QueueFull {
                            queue: "tenant",
                            fix: "tenant registration retry counter overflowed u64; retry registration later",
                        })
                    },
                )?;
                Ok(())
            },
        )?;
        let id = issued.max(1);

        let tenant_offset = vyre_driver::accounting::checked_mul_u32_value(
            id,
            OPCODE_RANGE_PER_TENANT,
            TenantError::RegistryFull { issued },
        )?;
        let base_opcode = vyre_driver::accounting::checked_add_u32_value(
            TENANT_OPCODE_BASE,
            tenant_offset,
            TenantError::RegistryFull { issued },
        )?;
        let top_opcode = vyre_driver::accounting::checked_add_u32_value(
            base_opcode,
            OPCODE_RANGE_PER_TENANT,
            TenantError::RegistryFull { issued },
        )?;
        if top_opcode == SHUTDOWN {
            return Err(TenantError::RegistryFull { issued });
        }
        let handle = TenantHandle {
            state: Arc::new(TenantState {
                id,
                base_opcode,
                opcode_cap: OPCODE_RANGE_PER_TENANT,
                published_count: AtomicU64::new(0),
                max_outstanding_slots: quota.max_outstanding_slots.max(1),
                staging_bytes: AtomicU64::new(0),
                max_staging_bytes: quota.max_staging_bytes.max(1),
                resident_handles: AtomicU64::new(0),
                max_resident_handles: quota.max_resident_handles.max(1),
                drained_count: AtomicU64::new(0),
                quiesce_calls: AtomicU64::new(0),
                quiesce_timeouts: AtomicU64::new(0),
                quiesce_wait_ns: AtomicU64::new(0),
                revoked: AtomicU32::new(0),
                label: label.into(),
            }),
        };
        self.tenants.insert(id, handle.clone());
        Ok(handle)
    }

    /// Unregister a tenant. Future publishes on the handle fail
    /// with [`TenantError::Revoked`]. In-flight slots already on
    /// the GPU still execute  -  the host is responsible for
    /// quiescing before unregister if it needs that guarantee.
    pub fn unregister(&self, tenant_id: u32) -> Option<TenantHandle> {
        let (_, handle) = self.tenants.remove(&tenant_id)?;
        handle.state.revoked.store(1, Ordering::Release);
        handle.release_all_resource_reservations();
        Some(handle)
    }

    /// Snapshot of active tenants for observability / diagnostics.
    #[must_use]
    pub fn active_tenants(&self) -> Vec<TenantHandle> {
        let mut out = Vec::with_capacity(self.tenants.len());
        out.extend(self.tenants.iter().map(|entry| entry.value().clone()));
        out.sort_by_key(TenantHandle::id);
        out
    }

    /// Snapshot active tenants into caller-owned storage.
    pub fn active_tenants_into(&self, out: &mut Vec<TenantHandle>) {
        out.clear();
        out.reserve(self.tenants.len());
        self.tenants
            .iter()
            .for_each(|entry| out.push(entry.value().clone()));
        out.sort_by_key(TenantHandle::id);
    }

    /// Look up a tenant by id. Returns `None` if the id was
    /// unregistered.
    #[must_use]
    pub fn lookup(&self, tenant_id: u32) -> Option<TenantHandle> {
        self.tenants
            .get(&tenant_id)
            .map(|entry| entry.value().clone())
    }

    /// Snapshot runtime counters for every active tenant.
    #[must_use]
    pub fn runtime_counters(&self) -> Vec<TenantRuntimeCounters> {
        let mut out = Vec::with_capacity(self.tenants.len());
        self.tenants
            .iter()
            .map(|entry| entry.value().runtime_counters())
            .for_each(|counters| out.push(counters));
        out.sort_by_key(|counters| counters.tenant_id);
        out
    }

    /// Snapshot runtime counters into caller-owned storage.
    pub fn runtime_counters_into(&self, out: &mut Vec<TenantRuntimeCounters>) {
        out.clear();
        out.reserve(self.tenants.len());
        self.tenants
            .iter()
            .map(|entry| entry.value().runtime_counters())
            .for_each(|counters| out.push(counters));
        out.sort_by_key(|counters| counters.tenant_id);
    }

    /// Select a maximal independent subset of tenants for a fair
    /// schedule slot.
    ///
    /// `conflict_adj[i*n+j] != 0` means tenants `i` and `j` cannot
    /// share the same dispatch slot (e.g., both pinned to the same
    /// queue, or both holding mutually-exclusive opcode locks). The
    /// Returns a Vec of tenant ids in selection order. Empty if no
    /// tenants are active.
    #[must_use]
    pub fn select_concurrent_tenants(&self, conflict_adj: &[u32]) -> Vec<u32> {
        let mut out = Vec::new();
        let mut scratch = TenantSelectionScratch::new();
        self.select_concurrent_tenants_into(conflict_adj, &mut out, &mut scratch);
        out
    }

    /// Select a maximal independent tenant subset into caller-owned storage.
    pub fn select_concurrent_tenants_into(
        &self,
        conflict_adj: &[u32],
        out: &mut Vec<u32>,
        scratch: &mut TenantSelectionScratch,
    ) {
        out.clear();
        scratch.active_ids.clear();
        scratch.active_ids.reserve(self.tenants.len());
        self.tenants
            .iter()
            .map(|entry| entry.value().id())
            .for_each(|id| scratch.active_ids.push(id));
        scratch.active_ids.sort_unstable();
        let n = scratch.active_ids.len();
        if n == 0 {
            return;
        }
        if vyre_driver::accounting::checked_mul_usize_lazy(n, n, || ()).ok()
            != Some(conflict_adj.len())
        {
            // Degenerate: caller didn't supply a matching adjacency.
            // Default to all-tenants-can-run (no conflicts).
            out.reserve(n);
            out.extend(scratch.active_ids.iter().copied());
            return;
        }
        if conflict_adj.iter().all(|conflict| *conflict == 0) {
            out.reserve(n);
            out.extend(scratch.active_ids.iter().copied());
            return;
        }
        scratch.selected_indices.clear();
        scratch.selected_indices.reserve(n);
        'candidate: for candidate_idx in 0..n {
            for &selected_idx in &scratch.selected_indices {
                if conflict_adj[candidate_idx * n + selected_idx] != 0
                    || conflict_adj[selected_idx * n + candidate_idx] != 0
                {
                    continue 'candidate;
                }
            }
            scratch.selected_indices.push(candidate_idx);
        }
        out.reserve(scratch.selected_indices.len());
        for &index in &scratch.selected_indices {
            if let Some(&id) = scratch.active_ids.get(index) {
                out.push(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_tenants_get_distinct_id_and_opcode_ranges() {
        let reg = TenantRegistry::new();
        let a = reg
            .register("scanner-a")
            .expect("Fix: register a; restore this invariant before continuing.");
        let b = reg
            .register("scanner-b")
            .expect("Fix: register b; restore this invariant before continuing.");
        assert_ne!(a.id(), b.id());
        assert!(a.base_opcode() + OPCODE_RANGE_PER_TENANT <= b.base_opcode());
        assert_eq!(a.label(), "scanner-a");
        assert_eq!(b.label(), "scanner-b");
    }

    #[test]
    fn global_opcode_rejects_out_of_range_local() {
        let reg = TenantRegistry::new();
        let t = reg.register("soleno").unwrap();
        let err = t
            .global_opcode(OPCODE_RANGE_PER_TENANT)
            .expect_err("oversized local opcode must reject");
        assert!(matches!(err, TenantError::OpcodeOutOfRange { .. }));

        let ok = t
            .global_opcode(42)
            .expect("Fix: 42 < cap; restore this invariant before continuing.");
        assert_eq!(ok, t.base_opcode() + 42);
    }

    #[test]
    fn publish_slot_writes_with_tenant_id_and_bumps_counter() {
        let reg = TenantRegistry::new();
        let t = reg.register("warpscan").unwrap();
        let mut ring = Megakernel::try_encode_empty_ring(4).unwrap();

        t.publish_slot(
            &mut ring,
            /* slot = */ 0,
            /* local = */ 7,
            &[1, 2, 3],
        )
        .expect("Fix: publish; restore this invariant before continuing.");
        assert_eq!(t.published_count(), 1);

        // Slot 0 should carry tenant=t.id(), opcode=t.base_opcode()+7.
        let tenant_off = super::super::megakernel::protocol::TENANT_WORD as usize * 4;
        let opcode_off = super::super::megakernel::protocol::OPCODE_WORD as usize * 4;
        let stored_tenant =
            u32::from_le_bytes(ring[tenant_off..tenant_off + 4].try_into().unwrap());
        let stored_opcode =
            u32::from_le_bytes(ring[opcode_off..opcode_off + 4].try_into().unwrap());
        assert_eq!(stored_tenant, t.id());
        assert_eq!(stored_opcode, t.base_opcode() + 7);
    }

    #[test]
    fn unregister_blocks_future_publishes() {
        let reg = TenantRegistry::new();
        let t = reg.register("vein").unwrap();
        let tenant_id = t.id();
        let mut ring = Megakernel::try_encode_empty_ring(2).unwrap();
        t.publish_slot(&mut ring, 0, 0, &[0, 0, 0])
            .expect("Fix: first publish ok; restore this invariant before continuing.");
        reg.unregister(tenant_id)
            .expect("Fix: unregister; restore this invariant before continuing.");
        let err = t
            .publish_slot(&mut ring, 1, 0, &[0, 0, 0])
            .expect_err("publish after unregister must reject");
        assert!(matches!(err, TenantError::Revoked { .. }));
        assert!(reg.lookup(tenant_id).is_none());
    }

    #[test]
    fn quiesce_returns_when_drained_catches_up() {
        let reg = TenantRegistry::new();
        let t = reg.register("t1").unwrap();
        let mut ring = Megakernel::try_encode_empty_ring(2).unwrap();
        t.publish_slot(&mut ring, 0, 0, &[1, 2, 3]).unwrap();
        t.publish_slot(&mut ring, 1, 0, &[4, 5, 6]).unwrap();
        assert_eq!(t.published_count(), 2);
        t.note_drained(2);
        t.quiesce(1)
            .expect("Fix: drained == published after note_drained; restore this invariant before continuing.");
        let counters = t.runtime_counters();
        assert_eq!(counters.published_count, 2);
        assert_eq!(counters.drained_count, 2);
        assert_eq!(counters.outstanding_slots, 0);
        assert_eq!(counters.quiesce_calls, 1);
        assert_eq!(counters.quiesce_timeouts, 0);
    }

    #[test]
    fn quiesce_times_out_when_drain_stalled() {
        let reg = TenantRegistry::new();
        let t = reg.register("t2").unwrap();
        let mut ring = Megakernel::try_encode_empty_ring(1).unwrap();
        t.publish_slot(&mut ring, 0, 0, &[0, 0, 0]).unwrap();
        // Never note_drained → quiesce must time out.
        let err = t.quiesce(4).expect_err("stalled quiesce must time out");
        assert!(matches!(
            err,
            TenantError::QuiesceTimeout { outstanding: 1, .. }
        ));
        let counters = t.runtime_counters();
        assert_eq!(counters.outstanding_slots, 1);
        assert_eq!(counters.quiesce_calls, 1);
        assert_eq!(counters.quiesce_timeouts, 1);
    }

    #[test]
    fn bounded_tenant_backpressure_rejects_unbounded_publish_backlog() {
        let reg = TenantRegistry::new();
        let t = reg.register_with_backpressure("bounded", 2).unwrap();
        let mut ring = Megakernel::try_encode_empty_ring(4).unwrap();

        t.publish_slot(&mut ring, 0, 0, &[1]).unwrap();
        t.publish_slot(&mut ring, 1, 0, &[2]).unwrap();
        let err = t
            .publish_slot(&mut ring, 2, 0, &[3])
            .expect_err("third outstanding publish must hit tenant backpressure");
        assert!(matches!(
            err,
            TenantError::Backpressure {
                outstanding: 2,
                cap: 2,
                ..
            }
        ));
        assert_eq!(t.published_count(), 2);
        let counters = t.runtime_counters();
        assert_eq!(counters.max_outstanding_slots, 2);
        assert_eq!(counters.outstanding_slots, 2);
    }

    #[test]
    fn tenant_backpressure_reopens_after_drain_progress() {
        let reg = TenantRegistry::new();
        let t = reg.register_with_backpressure("bounded", 1).unwrap();
        let mut ring = Megakernel::try_encode_empty_ring(2).unwrap();

        t.publish_slot(&mut ring, 0, 0, &[1]).unwrap();
        assert!(matches!(
            t.publish_slot(&mut ring, 1, 0, &[2]).unwrap_err(),
            TenantError::Backpressure { .. }
        ));
        t.note_drained(1);
        t.publish_slot(&mut ring, 1, 0, &[2])
            .expect("Fix: drain progress must reopen the bounded tenant queue; restore this invariant before continuing.");
        assert_eq!(t.published_count(), 2);
        assert_eq!(t.runtime_counters().outstanding_slots, 1);
    }

    #[test]
    fn tenant_resource_quotas_reject_overcommit_and_cleanup_on_unregister() {
        let reg = TenantRegistry::new();
        let t = reg
            .register_with_quotas("quota", TenantQuota::bounded(2, 16, 1))
            .unwrap();

        t.reserve_staging_bytes(8).unwrap();
        let staging_error = t
            .reserve_staging_bytes(9)
            .expect_err("staging byte quota must reject overcommit");
        assert!(matches!(
            staging_error,
            TenantError::StagingBackpressure {
                requested: 9,
                cap: 16,
                ..
            }
        ));
        assert_eq!(t.quota_counters().staging_bytes, 8);

        t.release_staging_bytes(4).unwrap();
        t.reserve_staging_bytes(12).unwrap();
        assert_eq!(t.quota_counters().staging_bytes, 16);
        let underflow = t
            .release_staging_bytes(17)
            .expect_err("staging release must reject underflow");
        assert!(matches!(
            underflow,
            TenantError::ResourceUnderflow {
                resource: "staging bytes",
                requested: 17,
                used: 16,
                ..
            }
        ));

        t.reserve_resident_handles(1).unwrap();
        let handle_error = t
            .reserve_resident_handles(1)
            .expect_err("resident handle quota must reject overcommit");
        assert!(matches!(
            handle_error,
            TenantError::ResidentHandleBackpressure {
                requested: 1,
                cap: 1,
                ..
            }
        ));
        assert_eq!(t.quota_counters().resident_handles, 1);

        let removed = reg.unregister(t.id()).unwrap();
        assert_eq!(removed.quota_counters().staging_bytes, 0);
        assert_eq!(removed.quota_counters().resident_handles, 0);
        assert!(matches!(
            t.reserve_staging_bytes(1).unwrap_err(),
            TenantError::Revoked { .. }
        ));
        assert!(matches!(
            t.reserve_resident_handles(1).unwrap_err(),
            TenantError::Revoked { .. }
        ));
    }

    #[test]
    fn tenant_registry_registration_retry_uses_adaptive_idle_not_unbounded_spin() {
        for retry in [0, 1, 2, QUIESCE_SPIN_POLLS - 1, QUIESCE_SPIN_POLLS] {
            tenant_registry_retry_idle(retry);
        }
        assert_eq!(
            quiesce_backoff_duration(QUIESCE_SPIN_POLLS),
            QUIESCE_MIN_PARK
        );
        assert_eq!(quiesce_backoff_duration(u64::MAX), QUIESCE_MAX_PARK);
    }

    #[test]
    fn quiesce_backoff_is_bounded_and_monotonic() {
        let samples = [
            quiesce_backoff_duration(0),
            quiesce_backoff_duration(1),
            quiesce_backoff_duration(2),
            quiesce_backoff_duration(8),
            quiesce_backoff_duration(64),
        ];
        assert_eq!(samples[0], QUIESCE_MIN_PARK);
        for pair in samples.windows(2) {
            assert!(pair[0] <= pair[1], "quiesce backoff must not shrink");
            assert!(pair[1] <= QUIESCE_MAX_PARK, "quiesce backoff must cap");
        }
        assert_eq!(quiesce_backoff_duration(u64::MAX), QUIESCE_MAX_PARK);
    }

    #[test]
    fn active_tenants_tracks_registrations() {
        let reg = TenantRegistry::new();
        let a = reg.register("a").unwrap();
        let b = reg.register("b").unwrap();
        let active: Vec<u32> = reg.active_tenants().iter().map(|t| t.id()).collect();
        assert!(active.contains(&a.id()));
        assert!(active.contains(&b.id()));
        reg.unregister(a.id());
        let after: Vec<u32> = reg.active_tenants().iter().map(|t| t.id()).collect();
        assert!(!after.contains(&a.id()));
        assert!(after.contains(&b.id()));
        let counters: Vec<u32> = reg
            .runtime_counters()
            .iter()
            .map(|tenant| tenant.tenant_id)
            .collect();
        assert_eq!(counters, vec![b.id()]);
    }

    #[test]
    fn tenant_snapshots_reuse_caller_storage() {
        let reg = TenantRegistry::new();
        let a = reg.register("a").unwrap();
        let b = reg.register("b").unwrap();
        let mut active = Vec::with_capacity(2);
        let mut counters = Vec::with_capacity(2);

        reg.active_tenants_into(&mut active);
        reg.runtime_counters_into(&mut counters);
        let active_ptr = active.as_ptr();
        let counters_ptr = counters.as_ptr();
        reg.active_tenants_into(&mut active);
        reg.runtime_counters_into(&mut counters);

        assert_eq!(active.as_ptr(), active_ptr);
        assert_eq!(counters.as_ptr(), counters_ptr);
        assert!(active.iter().any(|tenant| tenant.id() == a.id()));
        assert!(active.iter().any(|tenant| tenant.id() == b.id()));
        assert!(counters.iter().any(|tenant| tenant.tenant_id == a.id()));
        assert!(counters.iter().any(|tenant| tenant.tenant_id == b.id()));
    }

    #[test]
    fn concurrent_tenant_selection_reuses_scratch_and_output() {
        let reg = TenantRegistry::new();
        let a = reg.register("a").unwrap();
        let b = reg.register("b").unwrap();
        let c = reg.register("c").unwrap();
        let n = 3;
        let mut conflicts = vec![0_u32; n * n];
        conflicts[0 * n + 1] = 1;
        conflicts[1 * n + 0] = 1;
        let mut out = Vec::with_capacity(3);
        let mut scratch = TenantSelectionScratch::new();

        reg.select_concurrent_tenants_into(&conflicts, &mut out, &mut scratch);
        let out_ptr = out.as_ptr();
        let active_ids_ptr = scratch.active_ids.as_ptr();
        let selected_ptr = scratch.selected_indices.as_ptr();
        reg.select_concurrent_tenants_into(&conflicts, &mut out, &mut scratch);

        assert_eq!(out.as_ptr(), out_ptr);
        assert_eq!(scratch.active_ids.as_ptr(), active_ids_ptr);
        assert_eq!(scratch.selected_indices.as_ptr(), selected_ptr);
        assert!(out.contains(&a.id()) || out.contains(&b.id()));
        assert!(!(out.contains(&a.id()) && out.contains(&b.id())));
        assert!(out.contains(&c.id()));
    }

    #[test]
    fn concurrent_tenant_selection_fast_paths_all_zero_conflicts() {
        let reg = TenantRegistry::new();
        let a = reg.register("a").unwrap();
        let b = reg.register("b").unwrap();
        let c = reg.register("c").unwrap();
        let mut out = Vec::with_capacity(8);
        let mut scratch = TenantSelectionScratch::new();
        let conflicts = vec![0_u32; 9];
        let out_ptr = out.as_ptr();

        reg.select_concurrent_tenants_into(&conflicts, &mut out, &mut scratch);

        assert_eq!(out, vec![a.id(), b.id(), c.id()]);
        assert_eq!(
            out.as_ptr(),
            out_ptr,
            "all-zero conflict fast path must reuse caller-owned output storage"
        );
        assert!(
            scratch.selected_indices.is_empty(),
            "all-zero conflict fast path must not populate pairwise selection scratch"
        );
    }

    #[test]
    fn concurrent_tenant_selection_respects_conflicts() {
        let reg = TenantRegistry::new();
        let a = reg.register("a").unwrap();
        let b = reg.register("b").unwrap();
        let c = reg.register("c").unwrap();
        let n = 3;
        let mut conflicts = vec![0_u32; n * n];
        conflicts[0 * n + 1] = 1;
        conflicts[1 * n + 0] = 1;

        let selected = reg.select_concurrent_tenants(&conflicts);

        assert!(selected.contains(&a.id()) || selected.contains(&b.id()));
        assert!(!(selected.contains(&a.id()) && selected.contains(&b.id())));
        assert!(selected.contains(&c.id()));
    }

    #[test]
    fn concurrent_registration_assigns_unique_ids() {
        use std::thread;
        let reg = Arc::new(TenantRegistry::new());
        let mut handles = Vec::new();
        for i in 0..32 {
            let reg = Arc::clone(&reg);
            handles.push(thread::spawn(move || {
                reg.register(format!("t{i}")).unwrap().id()
            }));
        }
        let ids: Vec<u32> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "concurrent ids must be unique");
    }
}
