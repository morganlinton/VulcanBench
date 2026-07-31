//! Driver-tier observability surface (P-OBS-1).
//!
//! Single entry point for metrics consumers (Prometheus,
//! OpenTelemetry, Datadog, custom dashboards). Aggregates:
//!
//! - Substrate-call counters from
//!   `vyre_self_substrate::observability`.
//! - Cache hit/miss rates (when caches expose them).
//! - Substrate-decision telemetry (which math chose what).
//!
//! Backends extend this surface with
//! backend-specific gauges via the
//! [`crate::observability::BackendObservabilityProvider`] trait.

#[cfg(feature = "self-substrate-adapters")]
use vyre_self_substrate::decision_telemetry as decision_obs;
#[cfg(feature = "self-substrate-adapters")]
use vyre_self_substrate::observability as substrate_obs;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

const TRACE_EVENT_CAPACITY: usize = 256;

/// Human-readable optimization event emitted when `VYRE_TRACE=1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubstrateAuditEvent {
    /// Substrate or policy that fired.
    pub substrate: &'static str,
    /// Action selected by the substrate.
    pub action: &'static str,
    /// Predicted or measured savings in nanoseconds.
    pub saved_ns: u128,
    /// Static context string suitable for logs and tests.
    pub detail: &'static str,
}

/// Snapshot of every driver-tier metric at a single instant.
///
/// Cheap to construct (atomic loads + flat `Vec` allocations).
/// Callers serialize via `serde` or convert to their dashboard's
/// metric format.
#[derive(Debug, Clone)]
pub struct DriverObservability {
    /// Per-substrate-module call counts.
    pub substrate_calls: Vec<(&'static str, u64)>,
    /// Sum across all substrate counters  -  single-number health signal.
    pub substrate_total_calls: u64,
    /// Substrate-decision histogram buckets (fusion / eviction /
    /// provenance) from
    /// `vyre_self_substrate::decision_telemetry`.
    pub decision_buckets: Vec<(&'static str, u64)>,
    /// Bounded recent audit events emitted by substrate decisions
    /// while `VYRE_TRACE=1` is active.
    pub audit_events: Vec<SubstrateAuditEvent>,
    /// Backend-neutral dispatch counters captured at the shared runtime
    /// boundary.
    pub dispatch: DispatchTelemetry,
}

/// Backend-neutral dispatch counters for runtime performance evidence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DispatchTelemetry {
    /// Dispatch submissions observed by the shared driver boundary.
    pub launches: u64,
    /// Input bytes presented to dispatch.
    pub input_bytes: u64,
    /// Output bytes read back to host-visible buffers.
    pub output_bytes: u64,
    /// Output slots written by dispatches.
    pub output_slots: u64,
    /// Output slots whose retained allocation was reused.
    pub output_slots_reused: u64,
    /// Output slots that moved an oversized incoming allocation into place.
    pub output_slots_moved: u64,
    /// New output slots appended because the caller-owned output vector was too
    /// short.
    pub output_slots_appended: u64,
    /// Incoming output bytes presented to caller-owned slot replacement.
    pub output_slot_incoming_bytes: u64,
    /// Output bytes copied into retained caller-owned slots.
    pub output_slot_copied_bytes: u64,
    /// Output bytes moved into place by swapping oversized incoming slots.
    pub output_slot_moved_bytes: u64,
    /// Output bytes appended beyond the previous output vector length.
    pub output_slot_appended_bytes: u64,
    /// Retained output-slot capacity observed after replacement.
    pub output_slot_retained_capacity_bytes: u64,
    /// Programs split because the backend lacked native grid-sync support.
    pub grid_sync_splits: u64,
    /// Total segment dispatches produced by grid-sync splitting.
    pub grid_sync_segments: u64,
    /// Total logical grid synchronization points split out of programs.
    pub grid_sync_points: u64,
}

struct DispatchTelemetryCounters {
    launches: AtomicU64,
    input_bytes: AtomicU64,
    output_bytes: AtomicU64,
    output_slots: AtomicU64,
    output_slots_reused: AtomicU64,
    output_slots_moved: AtomicU64,
    output_slots_appended: AtomicU64,
    output_slot_incoming_bytes: AtomicU64,
    output_slot_copied_bytes: AtomicU64,
    output_slot_moved_bytes: AtomicU64,
    output_slot_appended_bytes: AtomicU64,
    output_slot_retained_capacity_bytes: AtomicU64,
    grid_sync_splits: AtomicU64,
    grid_sync_segments: AtomicU64,
    grid_sync_points: AtomicU64,
}

impl DispatchTelemetryCounters {
    const fn new() -> Self {
        Self {
            launches: AtomicU64::new(0),
            input_bytes: AtomicU64::new(0),
            output_bytes: AtomicU64::new(0),
            output_slots: AtomicU64::new(0),
            output_slots_reused: AtomicU64::new(0),
            output_slots_moved: AtomicU64::new(0),
            output_slots_appended: AtomicU64::new(0),
            output_slot_incoming_bytes: AtomicU64::new(0),
            output_slot_copied_bytes: AtomicU64::new(0),
            output_slot_moved_bytes: AtomicU64::new(0),
            output_slot_appended_bytes: AtomicU64::new(0),
            output_slot_retained_capacity_bytes: AtomicU64::new(0),
            grid_sync_splits: AtomicU64::new(0),
            grid_sync_segments: AtomicU64::new(0),
            grid_sync_points: AtomicU64::new(0),
        }
    }

    fn snapshot(&self) -> DispatchTelemetry {
        DispatchTelemetry {
            launches: self.launches.load(Ordering::Relaxed),
            input_bytes: self.input_bytes.load(Ordering::Relaxed),
            output_bytes: self.output_bytes.load(Ordering::Relaxed),
            output_slots: self.output_slots.load(Ordering::Relaxed),
            output_slots_reused: self.output_slots_reused.load(Ordering::Relaxed),
            output_slots_moved: self.output_slots_moved.load(Ordering::Relaxed),
            output_slots_appended: self.output_slots_appended.load(Ordering::Relaxed),
            output_slot_incoming_bytes: self.output_slot_incoming_bytes.load(Ordering::Relaxed),
            output_slot_copied_bytes: self.output_slot_copied_bytes.load(Ordering::Relaxed),
            output_slot_moved_bytes: self.output_slot_moved_bytes.load(Ordering::Relaxed),
            output_slot_appended_bytes: self.output_slot_appended_bytes.load(Ordering::Relaxed),
            output_slot_retained_capacity_bytes: self
                .output_slot_retained_capacity_bytes
                .load(Ordering::Relaxed),
            grid_sync_splits: self.grid_sync_splits.load(Ordering::Relaxed),
            grid_sync_segments: self.grid_sync_segments.load(Ordering::Relaxed),
            grid_sync_points: self.grid_sync_points.load(Ordering::Relaxed),
        }
    }
}

static DISPATCH_TELEMETRY: DispatchTelemetryCounters = DispatchTelemetryCounters::new();

impl DriverObservability {
    /// Take a snapshot of all driver-tier metrics now.
    #[must_use]
    pub fn snapshot() -> Self {
        #[cfg(feature = "self-substrate-adapters")]
        {
            return Self::try_snapshot().unwrap_or_else(|_| Self {
                substrate_calls: Vec::new(),
                substrate_total_calls: 0,
                decision_buckets: Vec::new(),
                audit_events: Vec::new(),
                dispatch: snapshot_dispatch_telemetry(),
            });
        }
        #[cfg(not(feature = "self-substrate-adapters"))]
        {
            Self {
                substrate_calls: Vec::new(),
                substrate_total_calls: 0,
                decision_buckets: Vec::new(),
                audit_events: Vec::new(),
                dispatch: snapshot_dispatch_telemetry(),
            }
        }
    }

    /// Fallibly take a snapshot of all driver-tier metrics.
    ///
    /// # Errors
    ///
    /// Returns [`crate::backend::BackendError`] when substrate telemetry is not
    /// compiled in. Callers that require substrate counters should use this
    /// method instead of treating the compatibility [`Self::snapshot`] fallback
    /// as a full observability view.
    pub fn try_snapshot() -> Result<Self, crate::backend::BackendError> {
        #[cfg(feature = "self-substrate-adapters")]
        {
            Ok(Self {
                substrate_calls: substrate_obs::snapshot_counters(),
                substrate_total_calls: substrate_obs::total_calls(),
                decision_buckets: decision_obs::snapshot_decisions(),
                audit_events: snapshot_trace_events(),
                dispatch: snapshot_dispatch_telemetry(),
            })
        }
        #[cfg(not(feature = "self-substrate-adapters"))]
        {
            Err(crate::backend::BackendError::new(
                "vyre-driver observability substrate telemetry requires the self-substrate-adapters feature. Fix: enable the feature for substrate counters, or use DriverObservability::snapshot for dispatch-only compatibility telemetry."
                    .to_string(),
            ))
        }
    }

    /// Format the snapshot as Prometheus text-exposition format.
    /// Counter metrics use `vyre_driver_substrate_calls_total{module="<name>"}`.
    #[must_use]
    pub fn to_prometheus(&self) -> String {
        let mut out = String::with_capacity(prometheus_capacity(
            self.substrate_calls.len(),
            self.decision_buckets.len(),
            self.audit_events.len(),
        ));
        out.push_str(
            "# HELP vyre_driver_substrate_calls_total Total substrate-consumer calls per module\n",
        );
        out.push_str("# TYPE vyre_driver_substrate_calls_total counter\n");
        for (module, count) in &self.substrate_calls {
            // Strip the trailing _calls suffix from the module name
            // for a cleaner Prometheus label.
            let module_label = module.trim_end_matches("_calls");
            use std::fmt::Write;
            let _ = writeln!(
                out,
                "vyre_driver_substrate_calls_total{{module=\"{module_label}\"}} {count}"
            );
        }
        out.push_str(
            "# HELP vyre_driver_substrate_total_calls Sum of all substrate-consumer calls\n",
        );
        out.push_str("# TYPE vyre_driver_substrate_total_calls counter\n");
        let _ = std::fmt::Write::write_fmt(
            &mut out,
            format_args!(
                "vyre_driver_substrate_total_calls {}\n",
                self.substrate_total_calls
            ),
        );
        out.push_str("# HELP vyre_driver_substrate_decisions_total Substrate-decision histogram (fusion/eviction/provenance buckets)\n");
        out.push_str("# TYPE vyre_driver_substrate_decisions_total counter\n");
        for (bucket, count) in &self.decision_buckets {
            use std::fmt::Write;
            let _ = writeln!(
                out,
                "vyre_driver_substrate_decisions_total{{bucket=\"{bucket}\"}} {count}"
            );
        }
        out.push_str("# HELP vyre_driver_substrate_audit_saved_ns Predicted or measured savings per optimization event\n");
        out.push_str("# TYPE vyre_driver_substrate_audit_saved_ns gauge\n");
        for event in &self.audit_events {
            use std::fmt::Write;
            let _ = writeln!(
                out,
                "vyre_driver_substrate_audit_saved_ns{{substrate=\"{}\",action=\"{}\",detail=\"{}\"}} {}",
                event.substrate, event.action, event.detail, event.saved_ns
            );
        }
        out.push_str("# HELP vyre_driver_dispatch_launches_total Dispatch submissions observed by the shared driver boundary\n");
        out.push_str("# TYPE vyre_driver_dispatch_launches_total counter\n");
        let _ = std::fmt::Write::write_fmt(
            &mut out,
            format_args!(
                "vyre_driver_dispatch_launches_total {}\n",
                self.dispatch.launches
            ),
        );
        out.push_str(
            "# HELP vyre_driver_dispatch_bytes_total Host-visible dispatch bytes by direction\n",
        );
        out.push_str("# TYPE vyre_driver_dispatch_bytes_total counter\n");
        let _ = std::fmt::Write::write_fmt(
            &mut out,
            format_args!(
                "vyre_driver_dispatch_bytes_total{{direction=\"input\"}} {}\nvyre_driver_dispatch_bytes_total{{direction=\"output\"}} {}\n",
                self.dispatch.input_bytes,
                self.dispatch.output_bytes
            ),
        );
        out.push_str(
            "# HELP vyre_driver_dispatch_output_slots_total Output slot handling by kind\n",
        );
        out.push_str("# TYPE vyre_driver_dispatch_output_slots_total counter\n");
        let _ = std::fmt::Write::write_fmt(
            &mut out,
            format_args!(
                "vyre_driver_dispatch_output_slots_total{{kind=\"total\"}} {}\nvyre_driver_dispatch_output_slots_total{{kind=\"reused\"}} {}\nvyre_driver_dispatch_output_slots_total{{kind=\"moved\"}} {}\nvyre_driver_dispatch_output_slots_total{{kind=\"appended\"}} {}\n",
                self.dispatch.output_slots,
                self.dispatch.output_slots_reused,
                self.dispatch.output_slots_moved,
                self.dispatch.output_slots_appended
            ),
        );
        out.push_str("# HELP vyre_driver_dispatch_output_slot_bytes_total Output slot byte pressure by kind\n");
        out.push_str("# TYPE vyre_driver_dispatch_output_slot_bytes_total counter\n");
        let _ = std::fmt::Write::write_fmt(
            &mut out,
            format_args!(
                "vyre_driver_dispatch_output_slot_bytes_total{{kind=\"incoming\"}} {}\nvyre_driver_dispatch_output_slot_bytes_total{{kind=\"copied\"}} {}\nvyre_driver_dispatch_output_slot_bytes_total{{kind=\"moved\"}} {}\nvyre_driver_dispatch_output_slot_bytes_total{{kind=\"appended\"}} {}\nvyre_driver_dispatch_output_slot_bytes_total{{kind=\"retained_capacity\"}} {}\n",
                self.dispatch.output_slot_incoming_bytes,
                self.dispatch.output_slot_copied_bytes,
                self.dispatch.output_slot_moved_bytes,
                self.dispatch.output_slot_appended_bytes,
                self.dispatch.output_slot_retained_capacity_bytes
            ),
        );
        out.push_str("# HELP vyre_driver_grid_sync_splits_total Grid-sync split events and produced synchronization structure\n");
        out.push_str("# TYPE vyre_driver_grid_sync_splits_total counter\n");
        let _ = std::fmt::Write::write_fmt(
            &mut out,
            format_args!(
                "vyre_driver_grid_sync_splits_total{{kind=\"programs\"}} {}\nvyre_driver_grid_sync_splits_total{{kind=\"segments\"}} {}\nvyre_driver_grid_sync_splits_total{{kind=\"sync_points\"}} {}\n",
                self.dispatch.grid_sync_splits,
                self.dispatch.grid_sync_segments,
                self.dispatch.grid_sync_points
            ),
        );
        out
    }

    /// Format recent substrate audit events as line-oriented text.
    #[must_use]
    pub fn to_audit_log(&self) -> String {
        let mut out = String::with_capacity(audit_log_capacity(self.audit_events.len()));
        for event in &self.audit_events {
            use std::fmt::Write;
            let _ = writeln!(
                out,
                "{} {} saved={}ns {}",
                event.substrate, event.action, event.saved_ns, event.detail
            );
        }
        out
    }
}

fn prometheus_capacity(
    substrate_calls: usize,
    decision_buckets: usize,
    audit_events: usize,
) -> usize {
    let substrate_capacity =
        checked_capacity_mul(substrate_calls, 96, "substrate call metrics").unwrap_or(usize::MAX);
    let decision_capacity =
        checked_capacity_mul(decision_buckets, 112, "decision bucket metrics").unwrap_or(usize::MAX);
    let audit_capacity =
        checked_capacity_mul(audit_events, 128, "audit event metrics").unwrap_or(usize::MAX);
    checked_capacity_add(
        384,
        substrate_capacity,
        "prometheus substrate call capacity",
    )
    .and_then(|capacity| {
        checked_capacity_add(
            capacity,
            decision_capacity,
            "prometheus decision bucket capacity",
        )
    })
    .and_then(|capacity| {
        checked_capacity_add(capacity, audit_capacity, "prometheus audit event capacity")
    })
    .unwrap_or(usize::MAX)
}

fn audit_log_capacity(audit_events: usize) -> usize {
    checked_capacity_mul(audit_events, 96, "audit log events").unwrap_or(usize::MAX)
}

fn checked_capacity_mul(
    count: usize,
    bytes_per_entry: usize,
    label: &str,
) -> Result<usize, String> {
    count.checked_mul(bytes_per_entry).ok_or_else(|| {
        format!(
            "{label} capacity estimate overflowed: count={count}, bytes_per_entry={bytes_per_entry}. Fix: page observability output instead of silently clamping allocation size."
        )
    })
}

fn checked_capacity_add(left: usize, right: usize, label: &str) -> Result<usize, String> {
    left.checked_add(right).ok_or_else(|| {
        format!(
            "{label} capacity estimate overflowed: left={left}, right={right}. Fix: page observability output instead of silently clamping allocation size."
        )
    })
}

/// Record one completed dispatch's host-visible input and output volume.
pub fn record_dispatch_io(inputs: &[&[u8]], outputs: &[Vec<u8>]) {
    DISPATCH_TELEMETRY.launches.fetch_add(1, Ordering::Relaxed);
    DISPATCH_TELEMETRY
        .input_bytes
        .fetch_add(sum_input_bytes(inputs), Ordering::Relaxed);
    DISPATCH_TELEMETRY
        .output_bytes
        .fetch_add(sum_output_bytes(outputs), Ordering::Relaxed);
}

/// Record how caller-owned output slots were populated.
pub fn record_output_slot_stats(stats: crate::backend::OutputSlotStats) {
    DISPATCH_TELEMETRY
        .output_slots
        .fetch_add(stats.total_slots as u64, Ordering::Relaxed);
    DISPATCH_TELEMETRY
        .output_slots_reused
        .fetch_add(stats.reused_slots as u64, Ordering::Relaxed);
    DISPATCH_TELEMETRY
        .output_slots_moved
        .fetch_add(stats.moved_slots as u64, Ordering::Relaxed);
    DISPATCH_TELEMETRY
        .output_slots_appended
        .fetch_add(stats.appended_slots as u64, Ordering::Relaxed);
}

/// Record full output replacement accounting, including byte pressure.
pub fn record_output_replacement_stats(stats: crate::backend::OutputReplacementStats) {
    record_output_slot_stats(stats.slots);
    record_output_slot_byte_stats(stats.bytes);
}

/// Record byte-pressure accounting from caller-owned output slot replacement.
pub fn record_output_slot_byte_stats(stats: crate::backend::OutputSlotByteStats) {
    DISPATCH_TELEMETRY
        .output_slot_incoming_bytes
        .fetch_add(stats.incoming_bytes as u64, Ordering::Relaxed);
    DISPATCH_TELEMETRY
        .output_slot_copied_bytes
        .fetch_add(stats.copied_bytes as u64, Ordering::Relaxed);
    DISPATCH_TELEMETRY
        .output_slot_moved_bytes
        .fetch_add(stats.moved_bytes as u64, Ordering::Relaxed);
    DISPATCH_TELEMETRY
        .output_slot_appended_bytes
        .fetch_add(stats.appended_bytes as u64, Ordering::Relaxed);
    DISPATCH_TELEMETRY
        .output_slot_retained_capacity_bytes
        .fetch_add(stats.retained_capacity_bytes as u64, Ordering::Relaxed);
}

/// Record that one program was split into multiple dispatch segments because
/// the selected backend lacks native grid-sync support.
pub fn record_grid_sync_split(segment_count: usize) {
    DISPATCH_TELEMETRY
        .grid_sync_splits
        .fetch_add(1, Ordering::Relaxed);
    DISPATCH_TELEMETRY
        .grid_sync_segments
        .fetch_add(segment_count as u64, Ordering::Relaxed);
    let sync_points = segment_count.saturating_sub(1);
    DISPATCH_TELEMETRY
        .grid_sync_points
        .fetch_add(sync_points as u64, Ordering::Relaxed);
}

/// Snapshot backend-neutral dispatch telemetry.
#[must_use]
pub fn snapshot_dispatch_telemetry() -> DispatchTelemetry {
    DISPATCH_TELEMETRY.snapshot()
}

fn sum_input_bytes(inputs: &[&[u8]]) -> u64 {
    inputs.iter().map(|input| input.len() as u64).sum()
}

fn sum_output_bytes(outputs: &[Vec<u8>]) -> u64 {
    outputs.iter().map(|output| output.len() as u64).sum()
}

/// Trait every backend implements to surface backend-specific metrics
/// alongside the common driver-tier ones. Optional  -  backends not
/// implementing it still get the substrate-counter view.
pub trait BackendObservabilityProvider {
    /// Backend-specific metrics, formatted as a flat list of
    /// `(metric_name, value)`. The driver core combines these with
    /// the substrate counters into a unified snapshot.
    fn backend_metrics(&self) -> Vec<(&'static str, u64)>;
}

fn trace_events() -> &'static Mutex<VecDeque<SubstrateAuditEvent>> {
    static EVENTS: OnceLock<Mutex<VecDeque<SubstrateAuditEvent>>> = OnceLock::new();
    EVENTS.get_or_init(|| Mutex::new(VecDeque::with_capacity(TRACE_EVENT_CAPACITY)))
}

fn trace_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("VYRE_TRACE")
            .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false)
    })
}

/// Record one substrate audit event when `VYRE_TRACE=1`.
///
/// This is intentionally a no-op when trace is disabled so dispatch
/// policies can call it without allocating on normal hot paths.
pub fn record_substrate_audit_event(event: SubstrateAuditEvent) {
    if !trace_enabled() {
        return;
    }
    if let Ok(mut events) = trace_events().lock() {
        if events.len() == TRACE_EVENT_CAPACITY {
            events.pop_front();
        }
        tracing::info!(
            target: "vyre_driver::substrate_audit",
            substrate = event.substrate,
            action = event.action,
            saved_ns = event.saved_ns,
            detail = event.detail,
            "vyre substrate optimization fired"
        );
        events.push_back(event);
    }
}

#[cfg(feature = "self-substrate-adapters")]
fn snapshot_trace_events() -> Vec<SubstrateAuditEvent> {
    trace_events()
        .lock()
        .map(|events| {
            let mut snapshot = Vec::new();
            let _ = snapshot.try_reserve_exact(events.len());
            snapshot.extend(events.iter().cloned());
            snapshot
        })
        .unwrap_or_default()
}

#[cfg(test)]
pub(crate) fn record_substrate_audit_event_for_test(event: SubstrateAuditEvent) {
    if let Ok(mut events) = trace_events().lock() {
        if events.len() == TRACE_EVENT_CAPACITY {
            events.pop_front();
        }
        events.push_back(event);
    }
}

#[cfg(test)]
pub(crate) fn snapshot_for_test() -> DriverObservability {
    let audit_events = trace_events()
        .lock()
        .map(|events| events.iter().cloned().collect())
        .unwrap_or_default();
    DriverObservability {
        substrate_calls: Vec::new(),
        substrate_total_calls: 0,
        decision_buckets: Vec::new(),
        audit_events,
        dispatch: snapshot_dispatch_telemetry(),
    }
}

#[cfg(test)]
pub(crate) fn clear_substrate_audit_events_for_test() {
    if let Ok(mut events) = trace_events().lock() {
        events.clear();
    }
}

#[cfg(test)]
pub(crate) fn audit_events_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("Fix: audit event test lock must not be poisoned")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "self-substrate-adapters")]
    fn snapshot_yields_nonempty_substrate_list() {
        let snap = DriverObservability::snapshot();
        assert!(
            snap.substrate_calls
                .iter()
                .any(|(module, count)| *count > 0 && !module.is_empty()),
            "snapshot must record at least one substrate module with nonzero calls"
        );
    }

    #[test]
    #[cfg(feature = "self-substrate-adapters")]
    fn prometheus_output_contains_module_labels() {
        let snap = DriverObservability::snapshot();
        let prom = snap.to_prometheus();
        assert!(prom.contains("module=\"matroid_megakernel_scheduler\""));
        assert!(prom.contains("module=\"vsa_fingerprint\""));
        assert!(prom.contains("# HELP vyre_driver_substrate_calls_total"));
    }

    #[test]
    #[cfg(not(feature = "self-substrate-adapters"))]
    fn try_snapshot_without_adapter_feature_returns_structured_error() {
        let error = DriverObservability::try_snapshot()
            .expect_err("try_snapshot must report missing substrate telemetry as an error");
        let message = error.to_string();
        assert!(
            message.contains("self-substrate-adapters"),
            "structured error must name the missing feature"
        );
        assert!(
            message.contains("DriverObservability::snapshot"),
            "structured error must name the dispatch-only compatibility path"
        );
    }

    #[test]
    #[cfg(not(feature = "self-substrate-adapters"))]
    fn snapshot_without_adapter_feature_is_dispatch_only_not_panic() {
        let snapshot = DriverObservability::snapshot();
        assert!(snapshot.substrate_calls.is_empty());
        assert_eq!(snapshot.substrate_total_calls, 0);
        assert!(snapshot.decision_buckets.is_empty());
    }

    #[test]
    #[cfg(feature = "self-substrate-adapters")]
    fn total_calls_appears_in_prometheus() {
        let snap = DriverObservability::snapshot();
        let prom = snap.to_prometheus();
        assert!(prom.contains("vyre_driver_substrate_total_calls"));
    }

    #[test]
    #[cfg(feature = "self-substrate-adapters")]
    fn audit_log_and_prometheus_include_recorded_events() {
        let _guard = audit_events_test_lock();
        clear_substrate_audit_events_for_test();
        record_substrate_audit_event_for_test(SubstrateAuditEvent {
            substrate: "trace_jit",
            action: "speculate",
            saved_ns: 123,
            detail: "predicted_shape",
        });
        let snap = DriverObservability::snapshot();
        assert_eq!(snap.audit_events.len(), 1);
        assert!(snap
            .to_audit_log()
            .contains("trace_jit speculate saved=123ns"));
        assert!(snap
            .to_prometheus()
            .contains("vyre_driver_substrate_audit_saved_ns"));
        clear_substrate_audit_events_for_test();
    }

    #[test]
    fn dispatch_telemetry_records_bytes_slots_and_prometheus_metrics() {
        let before = snapshot_dispatch_telemetry();
        record_dispatch_io(&[&[1, 2, 3], &[4]], &[vec![9, 8]]);
        record_output_slot_stats(crate::backend::OutputSlotStats {
            total_slots: 3,
            reused_slots: 1,
            moved_slots: 1,
            appended_slots: 1,
        });
        record_output_slot_byte_stats(crate::backend::OutputSlotByteStats {
            incoming_bytes: 9,
            copied_bytes: 2,
            moved_bytes: 4,
            appended_bytes: 3,
            retained_capacity_bytes: 16,
        });

        let dispatch = snapshot_dispatch_telemetry();
        assert!(dispatch.launches >= before.launches + 1);
        assert!(dispatch.input_bytes >= before.input_bytes + 4);
        assert!(dispatch.output_bytes >= before.output_bytes + 2);
        assert!(dispatch.output_slots >= before.output_slots + 3);
        assert!(dispatch.output_slots_reused >= before.output_slots_reused + 1);
        assert!(dispatch.output_slots_moved >= before.output_slots_moved + 1);
        assert!(dispatch.output_slots_appended >= before.output_slots_appended + 1);
        assert!(dispatch.output_slot_incoming_bytes >= before.output_slot_incoming_bytes + 9);
        assert!(dispatch.output_slot_copied_bytes >= before.output_slot_copied_bytes + 2);
        assert!(dispatch.output_slot_moved_bytes >= before.output_slot_moved_bytes + 4);
        assert!(dispatch.output_slot_appended_bytes >= before.output_slot_appended_bytes + 3);
        assert!(
            dispatch.output_slot_retained_capacity_bytes
                >= before.output_slot_retained_capacity_bytes + 16
        );

        #[cfg(feature = "self-substrate-adapters")]
        {
            let snap = DriverObservability::snapshot();
            let prom = snap.to_prometheus();
            assert!(prom.contains("vyre_driver_dispatch_launches_total"));
            assert!(prom.contains("direction=\"input\""));
            assert!(prom.contains("kind=\"appended\""));
            assert!(prom.contains("kind=\"retained_capacity\""));
        }
    }

    #[test]
    fn grid_sync_telemetry_records_segments_and_sync_points() {
        let before = snapshot_dispatch_telemetry();
        record_grid_sync_split(4);
        let after = snapshot_dispatch_telemetry();

        assert!(after.grid_sync_splits >= before.grid_sync_splits + 1);
        assert!(after.grid_sync_segments >= before.grid_sync_segments + 4);
        assert!(after.grid_sync_points >= before.grid_sync_points + 3);

        #[cfg(feature = "self-substrate-adapters")]
        assert!(DriverObservability::snapshot()
            .to_prometheus()
            .contains("kind=\"sync_points\""));
    }
}
