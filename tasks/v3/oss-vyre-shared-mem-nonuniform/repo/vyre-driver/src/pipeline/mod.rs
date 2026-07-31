//! Pipeline mode  -  pre-compile a Program once, dispatch repeatedly with new inputs.

/// Shared on-disk compiled-pipeline cache.
pub mod cache;
/// Backend-neutral pipeline compilation entry points.
pub mod compiler;
/// Stable cache hashing and device fingerprint helpers.
pub mod hashing;

pub use cache::{
    DiskPipelineCache, PipelineCacheIdentity, PipelineCacheKey, PipelineCacheMissEvidence,
    PipelineCacheMissReason, PipelineFeatureFlags,
};
pub use compiler::{
    compile, compile_owned, compile_owned_with_telemetry, compile_shared,
    compile_shared_with_telemetry, compile_with_telemetry, prewarm, prewarm_owned, prewarm_shared,
};
pub use hashing::{
    dispatch_policy_cache_digest, dispatch_policy_cache_string, hex_encode, hex_short,
    normalized_program_cache_digest, try_normalized_program_cache_digest,
    update_dispatch_policy_cache_hash, PipelineDeviceFingerprint,
};

/// Version mixed into every persistent pipeline cache key.
pub const CURRENT_PIPELINE_CACHE_KEY_VERSION: u32 = 1;
/// Default maximum number of compiled pipeline artifacts retained in memory.
pub const DEFAULT_PIPELINE_CACHE_ENTRIES: usize = 256;
/// Default maximum bytes retained by a backend pipeline cache.
pub const DEFAULT_PIPELINE_CACHE_BYTES: usize = 256 * 1024 * 1024;
/// Baseline one-dimensional workgroup used when a caller supplies no override.
pub const DEFAULT_1D_WORKGROUP_SIZE: [u32; 3] = [64, 1, 1];

/// Backend-reported compiled-pipeline cache counters.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct PipelineCacheSnapshot {
    /// Cache lookups that found an already-compiled artifact.
    pub hits: u64,
    /// Cache lookups that required compile/load work.
    pub misses: u64,
}

/// Result of compiling a reusable pipeline with honest cache telemetry.
#[derive(Clone)]
pub struct CompiledPipelineBuild {
    /// Reusable pipeline returned by the backend or passthrough wrapper.
    pub pipeline: std::sync::Arc<dyn crate::backend::CompiledPipeline>,
    /// `Some(true)` when backend counters prove a cache hit,
    /// `Some(false)` when counters prove a miss, and `None` when the backend
    /// does not expose real compile-cache counters.
    pub cache_hit: Option<bool>,
    /// Reproducibility manifest for this compiled artifact.
    pub manifest: PipelineReproManifest,
}

/// Result of prewarming a backend pipeline cache before the hot dispatch path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PipelinePrewarmReport {
    /// Backend pipeline id that was materialized or fetched from cache.
    pub pipeline_id: String,
    /// `Some(true)` when backend counters prove the pipeline was already warm,
    /// `Some(false)` when this call performed compile/load work, and `None`
    /// when the backend does not expose real cache counters.
    pub cache_hit: Option<bool>,
    /// Reproducibility manifest for the warmed artifact.
    pub manifest: PipelineReproManifest,
}

/// JSON-serializable reproducibility sidecar for a compiled pipeline.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PipelineReproManifest {
    /// Manifest schema version.
    pub schema: u32,
    /// Backend id that compiled the artifact.
    pub backend_id: String,
    /// Backend pipeline id returned by [`crate::backend::CompiledPipeline::id`].
    pub pipeline_id: String,
    /// Canonical normalized Program digest as lowercase hex.
    pub program_digest: String,
    /// Dispatch policy fields that affect generated backend code.
    pub dispatch_policy: String,
    /// Backend-reported cache status for this compile/prewarm.
    pub cache_hit: Option<bool>,
}

impl PipelineReproManifest {
    /// Current manifest schema.
    pub const SCHEMA: u32 = 1;

    /// Build a manifest from shared compile facts.
    #[must_use]
    pub fn new(
        backend_id: impl Into<String>,
        pipeline_id: impl Into<String>,
        program_digest: [u8; 32],
        dispatch_policy: impl Into<String>,
        cache_hit: Option<bool>,
    ) -> Self {
        Self {
            schema: Self::SCHEMA,
            backend_id: backend_id.into(),
            pipeline_id: pipeline_id.into(),
            program_digest: hex_encode(&program_digest),
            dispatch_policy: dispatch_policy.into(),
            cache_hit,
        }
    }

    /// Serialize as compact JSON for sidecar files and result envelopes.
    ///
    /// # Errors
    ///
    /// Returns when serde cannot serialize the manifest. This should not occur
    /// for the current schema, but the error is propagated for forward
    /// compatibility.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// ROADMAP C6 substrate: pipeline reuse cache hit-rate audit.
///
/// Aggregates a stream of `Option<bool>` cache_hit values from the
/// dispatcher's [`CompiledPipelineBuild`]/`PipelinePrewarmReport`
/// reports into hit-rate telemetry. The dispatcher pushes one entry
/// per resolved pipeline (or one per prewarm); the audit produces a
/// `PipelineCacheAuditReport` that names the hit rate, the count of
/// each outcome, and whether the rate falls below a configurable
/// alarm threshold so operators can wire it into observability and
/// CI gates.
///
/// `Option<bool>::None` values count as `unknown` and are excluded
/// from the rate denominator. This matches the upstream contract:
/// some backends do not expose real compile-cache counters and
/// honestly report `None` rather than lying about a hit.
#[derive(Debug, Default, Clone)]
pub struct PipelineCacheAudit {
    hits: u64,
    misses: u64,
    unknowns: u64,
}

/// Snapshot of a [`PipelineCacheAudit`].
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineCacheAuditReport {
    /// Lookups that found an already-compiled artifact.
    pub hits: u64,
    /// Lookups that performed compile/load work.
    pub misses: u64,
    /// Lookups whose backend did not report cache state.
    pub unknowns: u64,
    /// Hit rate in basis points (0..=10_000) over the
    /// `hits + misses` denominator (excluding unknowns). `None` when
    /// `hits + misses == 0` so the caller can distinguish "no data"
    /// from "0% hit rate".
    pub hit_rate_bps: Option<u32>,
    /// Whether the hit rate is below the operator-supplied alarm
    /// threshold. Always `false` when `hit_rate_bps` is `None`.
    pub below_alarm_threshold: bool,
}

impl PipelineCacheAudit {
    /// Empty audit accumulator.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push one outcome from the dispatcher.
    pub fn observe(&mut self, cache_hit: Option<bool>) {
        match cache_hit {
            Some(true) => self.hits = increment_counter(self.hits, "pipeline cache hits"),
            Some(false) => self.misses = increment_counter(self.misses, "pipeline cache misses"),
            None => self.unknowns = increment_counter(self.unknowns, "pipeline cache unknowns"),
        }
    }

    /// Snapshot the audit, scoring it against `alarm_threshold_bps`.
    /// `alarm_threshold_bps = 8000` flags any audit with under 80% hit
    /// rate; pass `0` to disable the alarm.
    #[must_use]
    pub fn snapshot(&self, alarm_threshold_bps: u32) -> PipelineCacheAuditReport {
        let denominator = self.hits.saturating_add(self.misses);
        let hit_rate_bps = if denominator == 0 {
            None
        } else {
            Some(crate::numeric::ratio_basis_points_u64(
                self.hits,
                denominator,
                0,
                "pipeline cache hit rate",
                "driver",
            ))
        };
        let below_alarm_threshold = match hit_rate_bps {
            Some(rate) if alarm_threshold_bps > 0 => rate < alarm_threshold_bps,
            _ => false,
        };
        PipelineCacheAuditReport {
            hits: self.hits,
            misses: self.misses,
            unknowns: self.unknowns,
            hit_rate_bps,
            below_alarm_threshold,
        }
    }
}

fn increment_counter(value: u64, _label: &str) -> u64 {
    value.saturating_add(1)
}

/// Resolve pipeline cache limits from Tier-A operational environment settings.
#[must_use]
pub fn pipeline_cache_limits_from_env() -> (u32, usize) {
    let entries = parse_positive_env_u32(
        "VYRE_PIPELINE_CACHE_ENTRIES",
        DEFAULT_PIPELINE_CACHE_ENTRIES as u32,
    );
    let bytes = parse_positive_env_usize("VYRE_PIPELINE_CACHE_BYTES", DEFAULT_PIPELINE_CACHE_BYTES);
    (entries, bytes)
}

fn parse_positive_env_u32(name: &str, default: u32) -> u32 {
    let Some(raw) = std::env::var(name).ok() else {
        return default;
    };
    raw.parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn parse_positive_env_usize(name: &str, default: usize) -> usize {
    let Some(raw) = std::env::var(name).ok() else {
        return default;
    };
    raw.parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

#[cfg(test)]
mod tests;
