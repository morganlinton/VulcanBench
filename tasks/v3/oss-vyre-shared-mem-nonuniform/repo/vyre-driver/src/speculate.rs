//! Speculative rule evaluation with commit/rollback (G6).
//!
//! # What this is
//!
//! Rules split into a cheap **pre-filter** (literal-strings via
//! gpumatch) and an expensive **confirmer** (flows_to, dominates,
//! the full taint solver). The classical path is:
//!
//! ```text
//!   dispatch(prefilter) → readback(hits) → dispatch(confirmer on hits)
//! ```
//!
//! The gather between the two dispatches is a host-visible sync
//! point that drains the GPU: the confirmer starts with 0%
//! occupancy while it fills from a compacted input stream.
//!
//! The speculative path runs the confirmer on *every* tile,
//! assuming the pre-filter would pass, and commits only the tiles
//! whose pre-filter actually passed. Rollback is free  -  a tile
//! that shouldn't have produced output writes nothing, because the
//! commit is gated on the pre-filter bit.
//!
//! ```text
//!   dispatch(prefilter & confirmer fused) → readback(committed_tiles)
//! ```
//!
//! One dispatch, no host round-trip. On Ada-class hardware this is
//! a 2-4x wall-clock win on the fused kernel vs the non-speculative
//! two-stage GPU pair,
//! *if* the pre-filter hit rate is high enough to amortise the
//! speculative confirmer work. `AdaptiveSpeculator` watches the
//! commit-rate EMA and disables speculation when it stops paying.
//!
//! # Wire format
//!
//! Every speculative dispatch's output buffer carries a two-u32
//! trailer: `[committed, rolled_back]`. The host reads it via
//! [`crate::speculate::parse_counter_tail`] to build a [`crate::speculate::SpeculationReport`].

#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

use std::sync::atomic::{AtomicU32, Ordering};

use vyre_foundation::ir::Program;

use crate::autotune_store::{AutotuneKey, AutotuneRecord, AutotuneStore};
use crate::backend::{BackendError, DispatchConfig, OutputBuffers, VyreBackend};
use crate::specialization::SpecCacheKey;

/// Counts from one speculative dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SpeculationReport {
    /// Lanes whose confirmer output survived the commit gate.
    pub committed_tiles: u32,
    /// Lanes the confirmer ran on that the pre-filter rejected
    /// (work thrown away  -  the "cost" side of the trade).
    pub rolled_back_tiles: u32,
}

impl SpeculationReport {
    /// Construct a report from the raw counter pair a kernel wrote.
    #[must_use]
    pub fn from_counts(committed: u32, rolled: u32) -> Self {
        Self {
            committed_tiles: committed,
            rolled_back_tiles: rolled,
        }
    }

    /// Empty report  -  no tiles touched yet. Equivalent to
    /// [`Self::default`].
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Total tiles the confirmer ran on.
    #[must_use]
    pub fn attempted_tiles(&self) -> u64 {
        u64::from(self.committed_tiles) + u64::from(self.rolled_back_tiles)
    }

    /// Commit rate in parts-per-million. `0` when no tiles ran, to
    /// keep the return total-order (a missing observation doesn't
    /// outrank a 0% observation).
    #[must_use]
    pub fn commit_rate_ppm(&self) -> u32 {
        let total = self.attempted_tiles();
        crate::numeric::ratio_parts_per_million_u64(
            u64::from(self.committed_tiles),
            total,
            0,
            "speculation commit-rate",
            "driver",
        )
    }

    /// Commit rate as a whole-percent, floored.
    #[must_use]
    pub fn commit_rate_pct(&self) -> u32 {
        self.commit_rate_ppm() / 10_000
    }

    /// True when speculation is paying for itself vs the two-stage GPU
    /// path at the caller's threshold. Integer-only comparison.
    #[must_use]
    pub fn worthwhile(&self, threshold_pct: u32) -> bool {
        let threshold_ppm = u64::from(threshold_pct) * 10_000;
        u64::from(self.commit_rate_ppm()) >= threshold_ppm
    }
}

/// Default crossover threshold. Below this commit rate the
/// speculative path underperforms the non-speculative GPU
/// prefilter -> confirmer pair on Ada-class hardware. Empirical.
pub const DEFAULT_THRESHOLD_PCT: u32 = 15;

/// Caller-controlled speculation policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpeculationMode {
    /// Framework decides per-dispatch based on the backend capability
    /// and the adaptive speculator.
    #[default]
    Auto,
    /// Force the speculative path; fail loudly if the backend does not
    /// support it.
    Force,
    /// Never speculate, even if the backend supports it.
    Disable,
}

/// α = 1/4 for the commit-rate EMA. Reacts inside ~4 batches while
/// staying quiet on a single anomalous dispatch.
const EMA_SHIFT: u32 = 2;

/// Online speculator  -  decides dispatch by dispatch whether to run
/// the fused speculative kernel or the non-speculative two-stage GPU path.
///
/// The EMA is stored in ppm so we never leave integer math.
#[derive(Debug)]
pub struct AdaptiveSpeculator {
    threshold_ppm: u64,
    ema_commit_rate_ppm: AtomicU32,
    speculation_enabled: AtomicU32,
    samples: AtomicU32,
}

impl AdaptiveSpeculator {
    /// Construct with the given threshold in whole percent.
    /// Speculation starts **enabled** with a seed EMA equal to the
    /// threshold, so the first few dispatches take the speculative
    /// path and produce real evidence for the EMA.
    #[must_use]
    pub fn new(threshold_pct: u32) -> Self {
        let threshold_ppm = u64::from(threshold_pct) * 10_000;
        let seeded_ema_ppm = threshold_ppm.min(u64::from(u32::MAX)) as u32;
        Self {
            threshold_ppm,
            ema_commit_rate_ppm: AtomicU32::new(seeded_ema_ppm),
            speculation_enabled: AtomicU32::new(1),
            samples: AtomicU32::new(0),
        }
    }

    /// Default-threshold speculator.
    #[must_use]
    pub fn default_threshold() -> Self {
        Self::new(DEFAULT_THRESHOLD_PCT)
    }

    /// Current EMA-smoothed commit rate in ppm.
    #[must_use]
    pub fn commit_rate_ppm(&self) -> u32 {
        self.ema_commit_rate_ppm.load(Ordering::Acquire)
    }

    /// Number of dispatches folded into the EMA so far.
    #[must_use]
    pub fn samples(&self) -> u32 {
        self.samples.load(Ordering::Acquire)
    }

    /// Whether the next dispatch should use the speculative kernel.
    #[must_use]
    pub fn should_speculate(&self) -> bool {
        self.speculation_enabled.load(Ordering::Acquire) != 0
    }

    /// Record the outcome of one speculative dispatch and update
    /// the routing decision for the next one.
    ///
    /// EMA: `new = old + (obs - old) / 4`, implemented on u32 with
    /// signed intermediate to avoid wrap. A report with zero
    /// attempted tiles is ignored  -  it carries no signal.
    pub fn record(&self, report: SpeculationReport) {
        if report.attempted_tiles() == 0 {
            return;
        }
        let observation = report.commit_rate_ppm();
        // EMA update  -  single fetch_update so concurrent callers
        // cannot lose samples.
        self.ema_commit_rate_ppm
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                let delta = i64::from(observation) - i64::from(old);
                let step = delta >> EMA_SHIFT;
                let new = i64::from(old) + step;
                Some(new.clamp(0, i64::from(u32::MAX)) as u32)
            })
            .unwrap_or_else(|_| unreachable!("speculation EMA update closure always returns Some"));
        self.samples.fetch_add(1, Ordering::AcqRel);
        let new_ppm = u64::from(self.ema_commit_rate_ppm.load(Ordering::Acquire));
        // Hysteresis: enable when we clearly beat threshold,
        // disable when we clearly miss it. Deadband is ±25% of
        // threshold to avoid flapping right at the crossover.
        let margin = self.threshold_ppm / 4;
        let enable_at = self.threshold_ppm + margin;
        let disable_at = self.threshold_ppm - margin;
        let prev = self.speculation_enabled.load(Ordering::Acquire);
        if prev == 0 && new_ppm >= enable_at {
            self.speculation_enabled.store(1, Ordering::Release);
        } else if prev != 0 && new_ppm < disable_at {
            self.speculation_enabled.store(0, Ordering::Release);
        }
    }

    /// Threshold in ppm.
    #[must_use]
    pub fn threshold_ppm(&self) -> u64 {
        self.threshold_ppm
    }
}

/// Two little-endian u32s written at the tail of a speculative
/// output buffer by the fused kernel: `[committed, rolled_back]`.
pub const COUNTER_TAIL_BYTES: usize = 8;

/// Read the two-u32 trailer a speculative kernel wrote at the end
/// of its output buffer. Returns `None` if the buffer is too short
/// or its length is not a multiple of 4.
#[must_use]
pub fn parse_counter_tail(output_bytes: &[u8]) -> Option<SpeculationReport> {
    if output_bytes.len() < COUNTER_TAIL_BYTES {
        return None;
    }
    if output_bytes.len() % 4 != 0 {
        return None;
    }
    let tail_start = output_bytes.len() - COUNTER_TAIL_BYTES;
    let mut committed_bytes = [0_u8; 4];
    committed_bytes.copy_from_slice(&output_bytes[tail_start..tail_start + 4]);
    let mut rolled_bytes = [0_u8; 4];
    rolled_bytes.copy_from_slice(&output_bytes[tail_start + 4..tail_start + 8]);
    Some(SpeculationReport::from_counts(
        u32::from_le_bytes(committed_bytes),
        u32::from_le_bytes(rolled_bytes),
    ))
}

/// Encode a counter tail  -  used by CPU-reference kernels and
/// tests. Keeps the host + device endianness consistent.
#[must_use]
pub fn encode_counter_tail(report: SpeculationReport) -> [u8; COUNTER_TAIL_BYTES] {
    let mut out = [0_u8; COUNTER_TAIL_BYTES];
    out[0..4].copy_from_slice(&report.committed_tiles.to_le_bytes());
    out[4..8].copy_from_slice(&report.rolled_back_tiles.to_le_bytes());
    out
}

/// Variant chosen by a speculative side-cache race.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeculativeVariantKind {
    /// The conservative already-compiled plan won.
    Conservative,
    /// The speculative transformed plan won.
    Speculative,
}

/// Cache-key pair for a conservative-vs-speculative variant race.
#[derive(Debug, Clone, Copy)]
pub struct SpeculativeVariantKeys<'a> {
    /// Cache identity for the conservative plan.
    pub conservative: &'a SpecCacheKey,
    /// Cache identity for the speculative plan.
    pub speculative: &'a SpecCacheKey,
    /// Stable adapter id used by the autotune store.
    pub adapter_id: &'a str,
}

/// Timing + autotune record pair observed from one variant race.
#[derive(Debug, Clone)]
pub struct SpeculativeVariantRace {
    /// Conservative dispatch elapsed time, excluding compile.
    pub conservative_dispatch_ns: u64,
    /// Speculative dispatch elapsed time, excluding compile.
    pub speculative_dispatch_ns: u64,
    /// Conservative compile/cache-miss cost paid for this sample.
    pub conservative_compile_ns: u64,
    /// Speculative compile/cache-miss cost paid for this sample.
    pub speculative_compile_ns: u64,
    /// Autotune record that produced the conservative timing.
    pub conservative_record: AutotuneRecord,
    /// Autotune record that produced the speculative timing.
    pub speculative_record: AutotuneRecord,
}

/// Recorded verdict for one conservative-vs-speculative race.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeculativeVariantDecision {
    /// Which variant won after compile + dispatch cost.
    pub winner: SpeculativeVariantKind,
    /// Nanoseconds saved versus the loser. Zero for exact ties.
    pub saved_ns: u128,
    /// Autotune key used for the winning record.
    pub autotune_key: AutotuneKey,
}

impl SpeculativeVariantRace {
    /// Conservative total time: compile/cache-miss plus dispatch.
    #[must_use]
    pub fn conservative_total_ns(&self) -> u128 {
        u128::from(self.conservative_compile_ns) + u128::from(self.conservative_dispatch_ns)
    }

    /// Speculative total time: compile/cache-miss plus dispatch.
    #[must_use]
    pub fn speculative_total_ns(&self) -> u128 {
        u128::from(self.speculative_compile_ns) + u128::from(self.speculative_dispatch_ns)
    }

    /// Pick the faster variant. Ties intentionally choose the conservative
    /// plan so an equal speculative transform does not churn cache entries.
    #[must_use]
    pub fn winner(&self) -> SpeculativeVariantKind {
        if self.speculative_total_ns() < self.conservative_total_ns() {
            SpeculativeVariantKind::Speculative
        } else {
            SpeculativeVariantKind::Conservative
        }
    }
}

/// Record the faster side of a conservative-vs-speculative race.
///
/// Backends run the actual compile/dispatch measurements; this shared helper
/// owns the decision rule and autotune-store write so every backend learns the
/// same way from vec-pack, shared-promote, async-load-promote, and similar
/// speculative transforms.
pub fn record_speculative_variant_race(
    store: &mut AutotuneStore,
    keys: SpeculativeVariantKeys<'_>,
    race: SpeculativeVariantRace,
) -> SpeculativeVariantDecision {
    let conservative_total = race.conservative_total_ns();
    let speculative_total = race.speculative_total_ns();
    let winner = race.winner();
    let (cache_key, record, saved_ns) = match winner {
        SpeculativeVariantKind::Conservative => (
            keys.conservative,
            race.conservative_record,
            speculative_total - conservative_total,
        ),
        SpeculativeVariantKind::Speculative => (
            keys.speculative,
            race.speculative_record,
            conservative_total - speculative_total,
        ),
    };
    let autotune_key = AutotuneKey::new(cache_key, keys.adapter_id);
    store.put(autotune_key.clone(), record);
    SpeculativeVariantDecision {
        winner,
        saved_ns,
        autotune_key,
    }
}

/// Program pair used by a real prefilter/confirm scan path.
#[derive(Clone, Copy, Debug)]
pub struct SpeculativeDispatchPlan<'a> {
    /// Fused speculative prefilter + confirmer program.
    pub fused_program: &'a Program,
    /// Cheap prefilter program used by the non-speculative two-stage GPU path.
    pub prefilter_program: &'a Program,
    /// Output buffer containing the two-u32 speculative counter tail.
    pub counter_output_index: usize,
    /// Remove the counter tail from the returned fused output.
    pub strip_counter_tail: bool,
}

/// Result of a speculative routing decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeculativeDispatchOutcome {
    /// Output buffers produced by the fused program or two-stage confirmer.
    pub outputs: OutputBuffers,
    /// Counters observed from a fused speculative dispatch.
    pub report: Option<SpeculationReport>,
    /// True when the fused speculative program ran.
    pub used_speculative_path: bool,
}

/// Dispatch a real prefilter/confirm pair with adaptive speculative routing.
///
/// When `speculator.should_speculate()` is true this runs `plan.fused_program`,
/// parses the counter tail, records it back into the speculator, and returns
/// the fused outputs. When speculation is disabled it runs
/// `plan.prefilter_program` and passes the materialized prefilter outputs to
/// `confirm_two_stage`; the closure owns GPU candidate compaction and
/// confirmer dispatch semantics for the caller's domain.
///
/// # Errors
///
/// Returns a backend error when either dispatch fails, when the fused output
/// does not contain the required counter tail, or when the two-stage confirmer
/// closure rejects the prefilter output.
pub fn dispatch_prefilter_confirm<B, F>(
    backend: &B,
    speculator: &AdaptiveSpeculator,
    plan: SpeculativeDispatchPlan<'_>,
    inputs: &[&[u8]],
    config: &DispatchConfig,
    mut confirm_two_stage: F,
) -> Result<SpeculativeDispatchOutcome, BackendError>
where
    B: VyreBackend + ?Sized,
    F: FnMut(OutputBuffers) -> Result<OutputBuffers, BackendError>,
{
    let use_speculative = match config.speculation {
        Some(SpeculationMode::Force) => {
            if !backend.supports_speculation() {
                return Err(BackendError::UnsupportedFeature {
                    name: "speculative dispatch".to_string(),
                    backend: backend.id().to_string(),
                });
            }
            true
        }
        Some(SpeculationMode::Disable) => false,
        Some(SpeculationMode::Auto) | None => speculator.should_speculate(),
    };
    if use_speculative {
        let mut outputs = backend.dispatch_borrowed(plan.fused_program, inputs, config)?;
        let output_count = outputs.len();
        let counter_output = outputs.get_mut(plan.counter_output_index).ok_or_else(|| {
            BackendError::new(format!(
                "speculative dispatch expected counter output #{}, but fused program returned {output_count} outputs. Fix: set SpeculativeDispatchPlan.counter_output_index to the output carrying the two-u32 counter tail.",
                plan.counter_output_index,
            ))
        })?;
        let report = parse_counter_tail(counter_output).ok_or_else(|| {
            BackendError::new(
                "speculative dispatch output is missing the two-u32 counter tail. Fix: fused prefilter/confirm kernels must append [committed, rolled_back] to the configured output buffer.",
            )
        })?;
        if plan.strip_counter_tail {
            let new_len = counter_output.len().checked_sub(COUNTER_TAIL_BYTES).ok_or_else(|| {
                BackendError::new(
                    "speculative counter-tail strip underflowed. Fix: only strip outputs that contain the standard counter tail.",
                )
            })?;
            counter_output.truncate(new_len);
        }
        speculator.record(report);
        return Ok(SpeculativeDispatchOutcome {
            outputs,
            report: Some(report),
            used_speculative_path: true,
        });
    }

    let prefilter_outputs = backend.dispatch_borrowed(plan.prefilter_program, inputs, config)?;
    let outputs = confirm_two_stage(prefilter_outputs)?;
    Ok(SpeculativeDispatchOutcome {
        outputs,
        report: None,
        used_speculative_path: false,
    })
}

#[cfg(test)]

mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn empty_report_has_zero_commit_rate() {
        let r = SpeculationReport::empty();
        assert_eq!(r.commit_rate_ppm(), 0);
        assert_eq!(r.commit_rate_pct(), 0);
        assert_eq!(r.attempted_tiles(), 0);
        assert!(!r.worthwhile(1));
    }

    #[test]
    fn commit_rate_exact_at_quarter() {
        let r = SpeculationReport::from_counts(1, 3);
        assert_eq!(r.commit_rate_ppm(), 250_000);
        assert_eq!(r.commit_rate_pct(), 25);
    }

    #[test]
    fn worthwhile_honors_threshold() {
        let r = SpeculationReport::from_counts(20, 80);
        assert!(r.worthwhile(20));
        assert!(!r.worthwhile(25));
    }

    #[test]
    fn all_rolled_back_is_zero_commit_rate() {
        let r = SpeculationReport::from_counts(0, 1024);
        assert_eq!(r.commit_rate_ppm(), 0);
        assert!(!r.worthwhile(1));
    }

    #[test]
    fn all_committed_is_full_commit_rate() {
        let r = SpeculationReport::from_counts(1024, 0);
        assert_eq!(r.commit_rate_ppm(), 1_000_000);
        assert!(r.worthwhile(99));
    }

    #[test]
    fn commit_rate_uses_exact_attempt_count_without_clamping() {
        let r = SpeculationReport::from_counts(u32::MAX, u32::MAX);

        assert_eq!(r.attempted_tiles(), 8_589_934_590);
        assert_eq!(r.commit_rate_ppm(), 500_000);
        assert!(!r.worthwhile(u32::MAX));
    }

    #[test]
    fn parse_counter_tail_reads_pair() {
        let mut buf = vec![0_u8; 32];
        buf[24..28].copy_from_slice(&42_u32.to_le_bytes());
        buf[28..32].copy_from_slice(&100_u32.to_le_bytes());
        let r = parse_counter_tail(&buf)
            .expect("Fix: valid length; restore this invariant before continuing.");
        assert_eq!(r.committed_tiles, 42);
        assert_eq!(r.rolled_back_tiles, 100);
    }

    #[test]
    fn parse_counter_tail_rejects_short_buffer() {
        assert!(parse_counter_tail(&[0_u8; 7]).is_none());
    }

    #[test]
    fn parse_counter_tail_rejects_misaligned_length() {
        assert!(parse_counter_tail(&[0_u8; 9]).is_none());
    }

    #[test]
    fn encode_then_parse_roundtrips() {
        let r = SpeculationReport::from_counts(7, 13);
        let tail = encode_counter_tail(r);
        let mut buf = vec![0_u8; 32];
        buf[24..32].copy_from_slice(&tail);
        let parsed = parse_counter_tail(&buf).unwrap();
        assert_eq!(parsed, r);
    }

    fn spec_key(spec_hash: u64) -> SpecCacheKey {
        SpecCacheKey {
            shader_hash: 0xfeed,
            binding_sig: 0xbeef,
            workgroup_size: [64, 1, 1],
            spec_hash,
        }
    }

    fn tune(unroll: u32) -> AutotuneRecord {
        AutotuneRecord {
            workgroup_size: [64, 1, 1],
            unroll,
            tile: [0, 0, 0],
            recorded_at: String::new(),
        }
    }

    #[test]
    fn speculative_variant_race_records_speculative_winner() {
        let conservative = spec_key(1);
        let speculative = spec_key(2);
        let mut store = AutotuneStore::default();
        let decision = record_speculative_variant_race(
            &mut store,
            SpeculativeVariantKeys {
                conservative: &conservative,
                speculative: &speculative,
                adapter_id: "native-sm120",
            },
            SpeculativeVariantRace {
                conservative_dispatch_ns: 1_000,
                conservative_compile_ns: 0,
                speculative_dispatch_ns: 200,
                speculative_compile_ns: 100,
                conservative_record: tune(1),
                speculative_record: tune(4),
            },
        );

        assert_eq!(decision.winner, SpeculativeVariantKind::Speculative);
        assert_eq!(decision.saved_ns, 700);
        assert_eq!(store.len(), 1);
        assert_eq!(
            store
                .get(&decision.autotune_key)
                .expect("Fix: winning speculative record must be stored")
                .unroll,
            4
        );
        assert_eq!(
            decision.autotune_key,
            AutotuneKey::new(&speculative, "native-sm120")
        );
    }

    #[test]
    fn speculative_variant_race_tie_keeps_conservative_record() {
        let conservative = spec_key(3);
        let speculative = spec_key(4);
        let mut store = AutotuneStore::default();
        let decision = record_speculative_variant_race(
            &mut store,
            SpeculativeVariantKeys {
                conservative: &conservative,
                speculative: &speculative,
                adapter_id: "portable-vk",
            },
            SpeculativeVariantRace {
                conservative_dispatch_ns: 500,
                conservative_compile_ns: 100,
                speculative_dispatch_ns: 100,
                speculative_compile_ns: 500,
                conservative_record: tune(2),
                speculative_record: tune(8),
            },
        );

        assert_eq!(decision.winner, SpeculativeVariantKind::Conservative);
        assert_eq!(decision.saved_ns, 0);
        assert_eq!(
            store
                .get(&decision.autotune_key)
                .expect("Fix: winning conservative record must be stored")
                .unroll,
            2
        );
        assert_eq!(
            decision.autotune_key,
            AutotuneKey::new(&conservative, "portable-vk")
        );
    }

    #[test]
    fn speculative_variant_race_uses_widened_total_time() {
        let conservative = spec_key(5);
        let speculative = spec_key(6);
        let mut store = AutotuneStore::default();
        let decision = record_speculative_variant_race(
            &mut store,
            SpeculativeVariantKeys {
                conservative: &conservative,
                speculative: &speculative,
                adapter_id: "native-sm120",
            },
            SpeculativeVariantRace {
                conservative_dispatch_ns: u64::MAX,
                conservative_compile_ns: u64::MAX,
                speculative_dispatch_ns: 1,
                speculative_compile_ns: 1,
                conservative_record: tune(1),
                speculative_record: tune(8),
            },
        );

        assert_eq!(decision.winner, SpeculativeVariantKind::Speculative);
        assert_eq!(decision.saved_ns, (u128::from(u64::MAX) * 2) - 2);
    }

    #[test]
    fn speculation_source_uses_exact_arithmetic_for_report_and_race_policy() {
        let source = include_str!("speculate.rs");

        assert!(
            !source.contains(concat!("saturating", "_add"))
                && !source.contains(concat!("saturating", "_sub"))
                && !source.contains(concat!("saturating", "_mul")),
            "Fix: speculation report, hysteresis, and race timing policy must use exact widened arithmetic rather than saturating release-path counters."
        );
        assert!(
            source.contains("u64::from(self.committed_tiles) + u64::from(self.rolled_back_tiles)")
                && source.contains("u128::from(self.conservative_compile_ns)")
                && source.contains("u128::from(self.speculative_compile_ns)"),
            "Fix: speculation policy must widen before tile-count and timing arithmetic."
        );
    }

    #[test]
    fn adaptive_speculator_starts_enabled_at_threshold_seed() {
        let s = AdaptiveSpeculator::new(15);
        assert!(s.should_speculate());
        assert_eq!(s.commit_rate_ppm(), 150_000);
        assert_eq!(s.samples(), 0);
    }

    #[test]
    fn adaptive_speculator_disables_on_sustained_low_commit_rate() {
        let s = AdaptiveSpeculator::new(20);
        for _ in 0..20 {
            // 1% commit rate, well under 20% - 5% = 15% disable threshold.
            s.record(SpeculationReport::from_counts(1, 99));
        }
        assert!(
            !s.should_speculate(),
            "EMA should have collapsed below disable threshold"
        );
        assert!(s.commit_rate_ppm() < 150_000);
    }

    #[test]
    fn adaptive_speculator_reenables_after_sustained_high_commit_rate() {
        let s = AdaptiveSpeculator::new(20);
        for _ in 0..20 {
            s.record(SpeculationReport::from_counts(1, 99));
        }
        assert!(!s.should_speculate());
        for _ in 0..20 {
            s.record(SpeculationReport::from_counts(80, 20));
        }
        assert!(
            s.should_speculate(),
            "EMA should have climbed past enable threshold"
        );
    }

    #[test]
    fn adaptive_speculator_ignores_empty_report() {
        let s = AdaptiveSpeculator::new(15);
        let before = s.commit_rate_ppm();
        s.record(SpeculationReport::empty());
        assert_eq!(s.commit_rate_ppm(), before);
        assert_eq!(s.samples(), 0);
    }

    #[test]
    fn adaptive_speculator_hysteresis_avoids_flap_near_threshold() {
        let s = AdaptiveSpeculator::new(20);
        // Hover right at 20%  -  inside the ±5% deadband.
        for _ in 0..50 {
            s.record(SpeculationReport::from_counts(20, 80));
        }
        assert!(s.should_speculate(), "should stay on inside deadband");
    }

    #[test]
    fn adaptive_speculator_samples_count_matches_record_calls() {
        let s = AdaptiveSpeculator::new(15);
        for i in 0..17 {
            s.record(SpeculationReport::from_counts(i + 1, 10));
        }
        assert_eq!(s.samples(), 17);
    }

    struct TailBackend;

    impl crate::backend::private::Sealed for TailBackend {}

    impl VyreBackend for TailBackend {
        fn id(&self) -> &'static str {
            "tail-test"
        }

        fn supported_ops(&self) -> &HashSet<vyre_foundation::ir::OpId> {
            static OPS: std::sync::OnceLock<HashSet<vyre_foundation::ir::OpId>> =
                std::sync::OnceLock::new();
            OPS.get_or_init(HashSet::new)
        }

        fn dispatch(
            &self,
            _program: &Program,
            _inputs: &[Vec<u8>],
            _config: &DispatchConfig,
        ) -> Result<OutputBuffers, BackendError> {
            Ok(vec![encode_counter_tail(SpeculationReport::from_counts(
                3, 1,
            ))
            .to_vec()])
        }
    }

    #[test]
    fn dispatch_prefilter_confirm_records_fused_counter_tail() {
        let backend = TailBackend;
        let speculator = AdaptiveSpeculator::new(15);
        let plan = SpeculativeDispatchPlan {
            fused_program: &Program::empty(),
            prefilter_program: &Program::empty(),
            counter_output_index: 0,
            strip_counter_tail: true,
        };
        let outcome = dispatch_prefilter_confirm(
            &backend,
            &speculator,
            plan,
            &[],
            &DispatchConfig::default(),
            |_| panic!("non-speculative two-stage path must not run while speculation is enabled"),
        )
        .expect("Fix: fused counter tail should parse");
        assert!(outcome.used_speculative_path);
        assert_eq!(outcome.report, Some(SpeculationReport::from_counts(3, 1)));
        assert_eq!(outcome.outputs, vec![Vec::<u8>::new()]);
        assert_eq!(speculator.samples(), 1);
    }

    #[test]
    fn dispatch_prefilter_confirm_runs_two_stage_gpu_path_when_disabled() {
        let backend = TailBackend;
        let speculator = AdaptiveSpeculator::new(90);
        for _ in 0..16 {
            speculator.record(SpeculationReport::from_counts(0, 100));
        }
        assert!(!speculator.should_speculate());
        let plan = SpeculativeDispatchPlan {
            fused_program: &Program::empty(),
            prefilter_program: &Program::empty(),
            counter_output_index: 0,
            strip_counter_tail: false,
        };
        let outcome = dispatch_prefilter_confirm(
            &backend,
            &speculator,
            plan,
            &[],
            &DispatchConfig::default(),
            |prefilter| {
                assert_eq!(prefilter.len(), 1);
                Ok(vec![b"confirmed".to_vec()])
            },
        )
        .expect("Fix: two-stage GPU path should dispatch prefilter and confirmer");
        assert!(!outcome.used_speculative_path);
        assert_eq!(outcome.report, None);
        assert_eq!(outcome.outputs, vec![b"confirmed".to_vec()]);
    }
}
