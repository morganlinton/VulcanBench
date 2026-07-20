//! Backend-neutral autotuner framework and cache metadata.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use vyre_foundation::ir::Program;

/// Canonical 1D workgroup-size probes shared by live dispatch tuning and
/// backend timer sweeps.
pub const WORKGROUP_CANDIDATES: &[u32] = &[32, 64, 128, 256, 512, 1024];
const AUTOTUNER_ENV: &str = "VYRE_AUTOTUNER";
const MAX_TUNER_CACHE_BYTES: u64 = 4 * 1024 * 1024;

/// Tuner runtime mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Mode {
    /// Sweep candidate sizes on first dispatch.
    On,
    /// Sweep candidate sizes and use Fisher-preconditioned policy updates.
    NaturalGradient,
    /// Use cached decisions when present, otherwise the default workgroup.
    OffUseDefault,
}

impl Mode {
    /// Production default when `VYRE_AUTOTUNER` is unset.
    ///
    /// Explicit `VYRE_AUTOTUNER=off` or `default` still gives the stable
    /// cached/default path for deterministic bisects, but the release path
    /// exercises the Fisher-preconditioned autotuner by default.
    #[must_use]
    pub const fn production_default() -> Self {
        Mode::NaturalGradient
    }

    /// Resolve mode from `VYRE_AUTOTUNER`.
    #[must_use]
    pub fn from_env() -> Self {
        match std::env::var(AUTOTUNER_ENV).ok() {
            Some(value) => Self::from_env_value(Some(value.as_str())),
            None => Self::production_default(),
        }
    }

    fn from_env_value(value: Option<&str>) -> Self {
        match value {
            Some("on") => Mode::On,
            Some("natural" | "ng") => Mode::NaturalGradient,
            Some("off" | "default") => Mode::OffUseDefault,
            Some(_) => Self::production_default(),
            None => Self::production_default(),
        }
    }
}

/// Backend timing hook used by the generic best-of-N framework.
pub trait BackendTimer {
    /// Error type returned by a concrete timing implementation.
    type Error;

    /// Measure one workgroup-size candidate and return elapsed nanoseconds.
    ///
    /// # Errors
    ///
    /// Returns the concrete backend timing error when the dispatch or timer
    /// instrumentation fails.
    fn measure_candidate_ns(
        &mut self,
        program: &Program,
        workgroup_size: [u32; 3],
    ) -> Result<u64, Self::Error>;
}

/// Per-adapter tuner decisions keyed by program fingerprint.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TunerCache {
    /// `program_fingerprint -> best_workgroup_size`.
    pub entries: BTreeMap<String, [u32; 3]>,
}

/// Static program shape used to disambiguate autotuner decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticProgramShape {
    /// Declared or overridden workgroup shape.
    pub workgroup_size: [u32; 3],
    /// Static workgroup-count override when known.
    pub workgroup_count: Option<[u32; 3]>,
    /// Static visible output byte count used by the dispatch.
    pub output_bytes: u64,
}

impl StaticProgramShape {
    /// Build a shape record from a program and caller-known launch facts.
    #[must_use]
    pub fn new(program: &Program, workgroup_count: Option<[u32; 3]>, output_bytes: u64) -> Self {
        Self {
            workgroup_size: program.workgroup_size(),
            workgroup_count,
            output_bytes,
        }
    }
}

/// Stable key for per-adapter workgroup autotuning decisions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TunerProgramKey(String);

impl TunerProgramKey {
    /// Build a key from the canonical program fingerprint plus static shape.
    #[must_use]
    pub fn from_program(program: &Program, shape: StaticProgramShape) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"vyre-driver-workgroup-tuner-v1\0program\0");
        hasher.update(&program.fingerprint());
        hasher.update(b"\0workgroup-size\0");
        for axis in shape.workgroup_size {
            hasher.update(&axis.to_le_bytes());
        }
        hasher.update(b"\0workgroup-count\0");
        match shape.workgroup_count {
            Some(count) => {
                hasher.update(&[1]);
                for axis in count {
                    hasher.update(&axis.to_le_bytes());
                }
            }
            None => {
                hasher.update(&[0]);
            }
        }
        hasher.update(b"\0output-bytes\0");
        hasher.update(&shape.output_bytes.to_le_bytes());
        let digest = hasher.finalize();
        let mut key = String::with_capacity(67);
        key.push_str("v1-");
        push_hex(digest.as_bytes(), &mut key);
        Self(key)
    }

    /// String form used in the TOML cache.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn push_hex(bytes: &[u8], out: &mut String) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
}

impl AsRef<str> for TunerProgramKey {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl TunerCache {
    /// Return the best workgroup size for the given key, if cached.
    #[must_use]
    pub fn get(&self, program_fp: &str) -> Option<[u32; 3]> {
        self.entries.get(program_fp).copied()
    }

    /// Return the cached decision for a typed tuner key.
    #[must_use]
    pub fn get_key(&self, key: &TunerProgramKey) -> Option<[u32; 3]> {
        self.get(key.as_str())
    }

    /// Record a decision.
    pub fn set(&mut self, program_fp: impl Into<String>, size: [u32; 3]) {
        self.entries.insert(program_fp.into(), size);
    }

    /// Record a decision under a typed key.
    ///
    /// HOT PATH (autotuner cache write): takes ownership of `key` so the fingerprint `String`
    /// moves into the map  -  `set(key.as_str(), …)` would allocate a second copy of the same bytes.
    pub fn set_key(&mut self, key: TunerProgramKey, size: [u32; 3]) {
        self.entries.insert(key.0, size);
    }

    /// Load from a TOML file. Missing file returns an empty cache.
    ///
    /// # Errors
    ///
    /// Returns when the file exists but contains invalid TOML.
    pub fn load(path: &Path) -> Result<Self, String> {
        let Ok(contents) = read_tuner_cache_bounded(path) else {
            return Ok(Self::default());
        };
        let parsed: toml::Value = toml::from_str(&contents).map_err(|error| {
            format!(
                "Fix: tuner cache `{}` is not valid TOML: {error}",
                path.display()
            )
        })?;
        let mut entries = BTreeMap::new();
        if let Some(table) = parsed.as_table() {
            for (key, value) in table {
                if let Some(array) = value.as_array() {
                    if array.len() == 3 {
                        let mut triple = [0u32; 3];
                        for (index, value) in array.iter().enumerate() {
                            if let Some(number) = value.as_integer() {
                                if let Ok(converted) = u32::try_from(number) {
                                    triple[index] = converted;
                                }
                            }
                        }
                        entries.insert(key.clone(), triple);
                    }
                }
            }
        }
        Ok(Self { entries })
    }

    /// Persist to disk. Creates parent directories as needed.
    ///
    /// # Errors
    ///
    /// Returns when the parent directory cannot be created or the file cannot
    /// be written.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Fix: could not create tuner cache directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        let mut out = String::with_capacity(tuner_cache_string_capacity(self.entries.len()));
        for (key, size) in &self.entries {
            let _ = writeln!(out, "\"{}\" = [{}, {}, {}]", key, size[0], size[1], size[2]);
        }
        fs::write(path, &out).map_err(|error| {
            format!(
                "Fix: could not write tuner cache {}: {error}",
                path.display()
            )
        })
    }
}

fn read_tuner_cache_bounded(path: &Path) -> std::io::Result<String> {
    use std::io::Read as _;

    let mut file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    if metadata.len() > MAX_TUNER_CACHE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("tuner cache exceeds {MAX_TUNER_CACHE_BYTES} byte limit"),
        ));
    }
    let mut text = String::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take(MAX_TUNER_CACHE_BYTES + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX_TUNER_CACHE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "tuner cache exceeded bounded read limit",
        ));
    }
    Ok(text)
}

/// Best-of-N measurement result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TuningMeasurement {
    /// Winning workgroup size.
    pub workgroup_size: [u32; 3],
    /// Measured elapsed nanoseconds for the winner.
    pub elapsed_ns: u64,
}

/// 16.16 fixed-point value representing 1.0.
pub const Q16_ONE: u32 = 1 << 16;

/// Natural-gradient policy for choosing the next autotune probe from
/// measured latency samples.
///
/// The policy treats the candidate set as a discrete distribution over
/// launch configurations. Latency samples become a softmax over
/// `-elapsed_ns / temperature_ns`; the supplied inverse-Fisher square-root
/// matrix preconditions that probability/gradient vector before the driver
/// picks the next candidate. CUDA/self-substrate can produce the same
/// fixed-point matrix through the primitive-backed natural-gradient path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NaturalGradientPolicy {
    /// Softmax temperature in nanoseconds. Larger values explore more.
    pub temperature_ns: u64,
}

impl Default for NaturalGradientPolicy {
    fn default() -> Self {
        Self {
            temperature_ns: 10_000,
        }
    }
}

/// Result of a natural-gradient autotune policy update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NaturalGradientTuningStep {
    /// Candidate selected after Fisher preconditioning.
    pub selected_workgroup_size: [u32; 3],
    /// Fastest candidate observed in the raw measurement window.
    pub best_measured_workgroup_size: [u32; 3],
    /// Fastest elapsed time observed in the raw measurement window.
    pub best_measured_elapsed_ns: u64,
    /// Softmax policy weights in 16.16 fixed-point form.
    pub policy_weights_q16: Vec<u32>,
    /// Fisher-preconditioned gradient magnitudes in 16.16 fixed-point form.
    pub natural_gradient_q16: Vec<u32>,
}

/// Errors returned by natural-gradient autotune policy construction.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NaturalGradientTuningError {
    /// No latency samples were provided.
    EmptyMeasurements,
    /// The inverse-Fisher square-root matrix was not `n * n`.
    FisherMatrixShape {
        /// Number of latency samples.
        measurements: usize,
        /// Number of fixed-point cells in the supplied matrix.
        cells: usize,
    },
    /// The softmax temperature was zero.
    ZeroTemperature,
}

impl std::fmt::Display for NaturalGradientTuningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyMeasurements => {
                write!(
                    f,
                    "natural-gradient tuner received no measurements. Fix: measure at least one candidate before policy update."
                )
            }
            Self::FisherMatrixShape {
                measurements,
                cells,
            } => write!(
                f,
                "natural-gradient tuner expected an inverse-Fisher matrix with {} cells for {measurements} measurement(s), got {cells}. Fix: pass an n*n 16.16 matrix.",
                measurements.saturating_mul(*measurements)
            ),
            Self::ZeroTemperature => {
                write!(
                    f,
                    "natural-gradient tuner temperature is zero. Fix: use a positive temperature_ns."
                )
            }
        }
    }
}

impl std::error::Error for NaturalGradientTuningError {}

impl NaturalGradientPolicy {
    /// Suggest the next workgroup-size candidate from latency samples and an
    /// inverse-Fisher square-root matrix.
    ///
    /// `fisher_inv_sqrt_q16` is row-major `n x n`, 16.16 fixed-point. Passing
    /// an identity matrix makes the policy reduce to the softmax-gradient
    /// candidate. Non-identity blocks let the runtime bias exploration by the
    /// local latency manifold instead of blindly reusing the single fastest
    /// point.
    ///
    /// # Errors
    ///
    /// Returns [`NaturalGradientTuningError`] when the measurement set is
    /// empty, temperature is zero, or the Fisher matrix shape does not match.
    pub fn suggest(
        &self,
        measurements: &[TuningMeasurement],
        fisher_inv_sqrt_q16: &[u32],
    ) -> Result<NaturalGradientTuningStep, NaturalGradientTuningError> {
        if measurements.is_empty() {
            return Err(NaturalGradientTuningError::EmptyMeasurements);
        }
        if self.temperature_ns == 0 {
            return Err(NaturalGradientTuningError::ZeroTemperature);
        }
        let expected_cells = measurements.len().checked_mul(measurements.len()).ok_or(
            NaturalGradientTuningError::FisherMatrixShape {
                measurements: measurements.len(),
                cells: fisher_inv_sqrt_q16.len(),
            },
        )?;
        if fisher_inv_sqrt_q16.len() != expected_cells {
            return Err(NaturalGradientTuningError::FisherMatrixShape {
                measurements: measurements.len(),
                cells: fisher_inv_sqrt_q16.len(),
            });
        }

        let mut best_index = 0usize;
        let mut best_elapsed = measurements[0].elapsed_ns;
        for (index, measurement) in measurements.iter().enumerate().skip(1) {
            if measurement.elapsed_ns < best_elapsed {
                best_index = index;
                best_elapsed = measurement.elapsed_ns;
            }
        }

        let policy_weights_q16 =
            latency_softmax_weights_q16(measurements, best_elapsed, self.temperature_ns);
        let natural_gradient_q16 =
            precondition_q16(fisher_inv_sqrt_q16, &policy_weights_q16, measurements.len());
        let selected_index = natural_gradient_q16
            .iter()
            .enumerate()
            .max_by_key(|(_, value)| *value)
            .map(|(index, _)| index)
            .unwrap_or(best_index);

        Ok(NaturalGradientTuningStep {
            selected_workgroup_size: measurements[selected_index].workgroup_size,
            best_measured_workgroup_size: measurements[best_index].workgroup_size,
            best_measured_elapsed_ns: best_elapsed,
            policy_weights_q16,
            natural_gradient_q16,
        })
    }
}

/// Build an identity inverse-Fisher square-root matrix in 16.16 fixed point.
#[must_use]
pub fn identity_fisher_q16(candidate_count: usize) -> Vec<u32> {
    let mut out = Vec::new();
    identity_fisher_q16_into(candidate_count, &mut out);
    out
}

/// Write an identity inverse-Fisher square-root matrix into caller-owned
/// storage.

pub fn identity_fisher_q16_into(candidate_count: usize, out: &mut Vec<u32>) {
    let Some(cells) = candidate_count.checked_mul(candidate_count) else {
        out.clear();
        return;
    };
    out.clear();
    out.resize(cells, 0);
    for index in 0..candidate_count {
        out[index * candidate_count + index] = Q16_ONE;
    }
}

fn latency_softmax_weights_q16(
    measurements: &[TuningMeasurement],
    best_elapsed: u64,
    temperature_ns: u64,
) -> Vec<u32> {
    let temperature = temperature_ns as f64;
    let mut weights = Vec::with_capacity(measurements.len());
    let mut sum = 0.0f64;
    for measurement in measurements {
        let penalty = measurement.elapsed_ns.saturating_sub(best_elapsed) as f64;
        let weight = (-penalty / temperature).exp();
        weights.push(weight);
        sum += weight;
    }
    let mut out = Vec::with_capacity(measurements.len());
    let mut assigned = 0u32;
    for (index, weight) in weights.iter().enumerate() {
        if index + 1 == weights.len() {
            out.push(Q16_ONE.saturating_sub(assigned));
            break;
        }
        let q16 = ((*weight / sum) * f64::from(Q16_ONE)).round() as u32;
        let remaining = Q16_ONE.saturating_sub(assigned);
        let q16 = q16.min(remaining);
        assigned = assigned.saturating_add(q16);
        out.push(q16);
    }
    out
}

fn precondition_q16(matrix_q16: &[u32], gradient_q16: &[u32], n: usize) -> Vec<u32> {
    let mut out = vec![0u32; n];
    for row in 0..n {
        let mut acc = 0u64;
        for col in 0..n {
            let matrix = u64::from(matrix_q16[row * n + col]);
            let gradient = u64::from(gradient_q16[col]);
            acc = acc.saturating_add((matrix.saturating_mul(gradient)) >> 16);
        }
        out[row] = acc.min(u64::from(u32::MAX)) as u32;
    }
    out
}

/// Workgroup-size autotuner.
pub struct Tuner {
    mode: Mode,
    cache: TunerCache,
    cache_path: PathBuf,
}

impl Tuner {
    /// Build a new tuner for the adapter fingerprinted as `adapter_fp`.
    #[must_use]
    pub fn new(adapter_fp: &str, mode: Mode) -> Self {
        let cache_path = Self::cache_path_for_adapter(adapter_fp);
        let cache = TunerCache::load(&cache_path).unwrap_or_default();
        Self {
            mode,
            cache,
            cache_path,
        }
    }

    /// Cache file path for a given adapter fingerprint.
    #[must_use]
    pub fn cache_path_for_adapter(adapter_fp: &str) -> PathBuf {
        let mut home = dirs_cache_root();
        home.push("vyre");
        home.push("tuner");
        home.push(format!("{adapter_fp}.toml"));
        home
    }

    /// Candidate workgroup sizes bounded by `max_invocations`.
    #[must_use]
    pub fn candidates_for(&self, max_invocations: u32) -> Vec<u32> {
        let mut candidates = Vec::new();
        let _ = candidates.try_reserve_exact(WORKGROUP_CANDIDATES.len());
        candidates.extend(
            WORKGROUP_CANDIDATES
                .iter()
                .copied()
                .filter(|candidate| *candidate <= max_invocations),
        );
        candidates
    }

    /// Default workgroup size used without cache data.
    #[must_use]
    pub const fn default_workgroup_size() -> [u32; 3] {
        crate::pipeline::DEFAULT_1D_WORKGROUP_SIZE
    }

    /// Mode this tuner is running in.
    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// Resolve the workgroup size for a program key.
    #[must_use]
    pub fn resolve(&self, program_fp: &str) -> [u32; 3] {
        self.cache
            .get(program_fp)
            .unwrap_or_else(Self::default_workgroup_size)
    }

    /// Resolve the workgroup size for a typed program/static-shape key.
    #[must_use]
    pub fn resolve_key(&self, key: &TunerProgramKey) -> [u32; 3] {
        self.resolve(key.as_str())
    }

    /// Record a sweep outcome in memory.
    pub fn record_decision(&mut self, program_fp: impl Into<String>, size: [u32; 3]) {
        self.cache.set(program_fp, size);
    }

    /// Record a sweep outcome for a typed key.
    pub fn record_key_decision(&mut self, key: TunerProgramKey, size: [u32; 3]) {
        self.cache.set_key(key, size);
    }

    /// Measure candidate sizes and choose the fastest one.
    ///
    /// # Errors
    ///
    /// Returns a backend timing error from [`BackendTimer`].
    pub fn best_of<T: BackendTimer>(
        &self,
        program: &Program,
        candidates: impl IntoIterator<Item = [u32; 3]>,
        timer: &mut T,
    ) -> Result<Option<TuningMeasurement>, T::Error> {
        let mut best = None;
        for workgroup_size in candidates {
            let elapsed_ns = timer.measure_candidate_ns(program, workgroup_size)?;
            let measurement = TuningMeasurement {
                workgroup_size,
                elapsed_ns,
            };
            if best
                .map(|current: TuningMeasurement| elapsed_ns < current.elapsed_ns)
                .unwrap_or(true)
            {
                best = Some(measurement);
            }
        }
        Ok(best)
    }

    /// Measure candidates, then choose the next probe with a
    /// Fisher-preconditioned natural-gradient policy.
    ///
    /// This is the concrete runtime handoff for `VYRE_AUTOTUNER=natural`.
    /// It reuses the same backend timer as [`Self::best_of`], records every
    /// measured candidate, and feeds those measurements into
    /// [`NaturalGradientPolicy`]. The returned step includes both the raw
    /// fastest measurement and the Fisher-directed next candidate.
    ///
    /// # Errors
    ///
    /// Returns backend timing errors from [`BackendTimer`] or policy errors
    /// from [`NaturalGradientPolicy`].
    pub fn best_of_natural_gradient<T: BackendTimer>(
        &self,
        program: &Program,
        candidates: impl IntoIterator<Item = [u32; 3]>,
        timer: &mut T,
        fisher_inv_sqrt_q16: &[u32],
        policy: NaturalGradientPolicy,
    ) -> Result<Result<NaturalGradientTuningStep, NaturalGradientTuningError>, T::Error> {
        let mut measurements = Vec::new();
        for workgroup_size in candidates {
            let elapsed_ns = timer.measure_candidate_ns(program, workgroup_size)?;
            measurements.push(TuningMeasurement {
                workgroup_size,
                elapsed_ns,
            });
        }
        Ok(policy.suggest(&measurements, fisher_inv_sqrt_q16))
    }

    /// Convert measured candidates into a Fisher-preconditioned next probe.
    ///
    /// This keeps the best-of-N timing hook compatible while giving CUDA and
    /// other GPU backends a richer update rule than "pick the current fastest
    /// sample forever." Backends can feed `fisher_inv_sqrt_q16` from the
    /// primitive-backed natural-gradient self-substrate path.
    ///
    /// # Errors
    ///
    /// Returns [`NaturalGradientTuningError`] when the policy input is
    /// malformed.
    pub fn natural_gradient_step(
        &self,
        measurements: &[TuningMeasurement],
        fisher_inv_sqrt_q16: &[u32],
        policy: NaturalGradientPolicy,
    ) -> Result<NaturalGradientTuningStep, NaturalGradientTuningError> {
        policy.suggest(measurements, fisher_inv_sqrt_q16)
    }

    /// Write the cache to disk.
    ///
    /// # Errors
    ///
    /// Returns the structured error from [`TunerCache::save`].
    pub fn persist(&self) -> Result<(), String> {
        self.cache.save(&self.cache_path)
    }
}

/// Snapshot of live behavior the tuner consumes for adaptive resizing.
#[derive(Debug, Clone)]
pub struct TunerFeedback {
    /// `(opcode_id, execution_count)` pairs from backend metrics.
    pub per_opcode_counts: Vec<(u32, u32)>,
    /// Total wall-time in microseconds.
    pub wall_time_us: u64,
    /// Idle microseconds inside the window.
    pub idle_us: u64,
    /// Workgroup size x this feedback was gathered on.
    pub observed_workgroup_size_x: u32,
    /// Observed throughput per microsecond.
    pub observed_throughput_per_us: f64,
}

/// Hysteresis-based default resize policy.
#[derive(Debug, Clone)]
pub struct DefaultPolicy {
    /// Upper bound from the adapter capability probe.
    pub adapter_max_workgroup_size_x: u32,
    /// Floor below which we never shrink.
    pub minimum_workgroup_size_x: u32,
    /// Throughput below which we grow.
    pub saturation_threshold_per_us: f64,
    /// Idle time above which we shrink.
    pub idle_shrink_us: u64,
}

impl Default for DefaultPolicy {
    fn default() -> Self {
        Self {
            adapter_max_workgroup_size_x: 1024,
            minimum_workgroup_size_x: 32,
            saturation_threshold_per_us: 1.0,
            idle_shrink_us: 100_000,
        }
    }
}

impl DefaultPolicy {
    /// Suggest a new workgroup size for the next feedback window.
    #[must_use]
    pub fn suggest_resize(&self, feedback: &TunerFeedback) -> Option<u32> {
        let current = feedback.observed_workgroup_size_x.max(1);
        if feedback.idle_us > self.idle_shrink_us {
            let shrunk = current / 2;
            if shrunk >= self.minimum_workgroup_size_x && shrunk != current {
                return Some(shrunk);
            }
            return None;
        }
        if feedback.observed_throughput_per_us < self.saturation_threshold_per_us {
            let grown = current.checked_mul(2)?;
            if grown <= self.adapter_max_workgroup_size_x && grown != current {
                return Some(grown);
            }
        }
        None
    }
}

fn tuner_cache_string_capacity(entries: usize) -> usize {
    entries.saturating_mul(96)
}

fn dirs_cache_root() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(xdg)
    } else if let Some(home) = std::env::var_os("HOME") {
        let mut path = PathBuf::from(home);
        path.push(".cache");
        path
    } else {
        PathBuf::from(".")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measurements() -> Vec<TuningMeasurement> {
        vec![
            TuningMeasurement {
                workgroup_size: [64, 1, 1],
                elapsed_ns: 12_000,
            },
            TuningMeasurement {
                workgroup_size: [128, 1, 1],
                elapsed_ns: 8_000,
            },
            TuningMeasurement {
                workgroup_size: [256, 1, 1],
                elapsed_ns: 10_000,
            },
        ]
    }

    struct StaticTimer {
        fail_on: Option<u32>,
        measured: Vec<[u32; 3]>,
    }

    impl StaticTimer {
        fn new() -> Self {
            Self {
                fail_on: None,
                measured: Vec::new(),
            }
        }

        fn failing(fail_on: u32) -> Self {
            Self {
                fail_on: Some(fail_on),
                measured: Vec::new(),
            }
        }
    }

    impl BackendTimer for StaticTimer {
        type Error = &'static str;

        fn measure_candidate_ns(
            &mut self,
            _program: &Program,
            workgroup_size: [u32; 3],
        ) -> Result<u64, Self::Error> {
            self.measured.push(workgroup_size);
            if self.fail_on == Some(workgroup_size[0]) {
                return Err("timer failed");
            }
            Ok(match workgroup_size[0] {
                64 => 12_000,
                128 => 8_000,
                256 => 10_000,
                _ => 50_000,
            })
        }
    }

    fn empty_program() -> Program {
        Program::wrapped(Vec::new(), [64, 1, 1], Vec::new())
    }

    #[test]
    fn unset_autotuner_mode_defaults_to_natural_gradient_release_path() {
        assert_eq!(Mode::production_default(), Mode::NaturalGradient);
        assert_eq!(Mode::from_env_value(None), Mode::NaturalGradient);
    }

    #[test]
    fn explicit_env_modes_preserve_escape_hatches() {
        assert_eq!(Mode::from_env_value(Some("natural")), Mode::NaturalGradient);
        assert_eq!(Mode::from_env_value(Some("ng")), Mode::NaturalGradient);
        assert_eq!(Mode::from_env_value(Some("on")), Mode::On);
        assert_eq!(Mode::from_env_value(Some("off")), Mode::OffUseDefault);
        assert_eq!(Mode::from_env_value(Some("default")), Mode::OffUseDefault);
    }

    #[test]
    fn identity_fisher_preserves_fastest_candidate_policy_gradient() {
        let policy = NaturalGradientPolicy {
            temperature_ns: 4_000,
        };
        let samples = measurements();
        let step = policy
            .suggest(&samples, &identity_fisher_q16(samples.len()))
            .expect("Fix: identity Fisher natural-gradient update should be valid");

        assert_eq!(step.best_measured_workgroup_size, [128, 1, 1]);
        assert_eq!(step.selected_workgroup_size, [128, 1, 1]);
        assert_eq!(step.best_measured_elapsed_ns, 8_000);
    }

    #[test]
    fn anisotropic_fisher_can_redirect_next_probe_without_changing_measurement_winner() {
        let policy = NaturalGradientPolicy {
            temperature_ns: 4_000,
        };
        let samples = measurements();
        let mut fisher = identity_fisher_q16(samples.len());
        fisher[0] = Q16_ONE * 8;

        let step = policy
            .suggest(&samples, &fisher)
            .expect("Fix: diagonal Fisher natural-gradient update should be valid");

        assert_eq!(step.best_measured_workgroup_size, [128, 1, 1]);
        assert_eq!(
            step.selected_workgroup_size,
            [64, 1, 1],
            "Fix: Fisher geometry must be able to steer exploration away from the raw fastest sample."
        );
        assert!(
            step.natural_gradient_q16[0] > step.natural_gradient_q16[1],
            "Fix: preconditioned gradient should reflect the anisotropic Fisher block."
        );
    }

    #[test]
    fn softmax_weights_conserve_q16_probability_mass_across_hostile_latencies() {
        let policy = NaturalGradientPolicy { temperature_ns: 1 };
        for base in [0_u64, 1, 10, 1_000, u64::MAX - 2] {
            let samples = vec![
                TuningMeasurement {
                    workgroup_size: [32, 1, 1],
                    elapsed_ns: base,
                },
                TuningMeasurement {
                    workgroup_size: [64, 1, 1],
                    elapsed_ns: base.saturating_add(1),
                },
                TuningMeasurement {
                    workgroup_size: [128, 1, 1],
                    elapsed_ns: base.saturating_add(2),
                },
            ];
            let step = policy
                .suggest(&samples, &identity_fisher_q16(samples.len()))
                .expect("Fix: hostile latency range should still produce a normalized policy");
            let total: u32 = step.policy_weights_q16.iter().sum();
            assert_eq!(
                total, Q16_ONE,
                "Fix: fixed-point policy weights must conserve probability mass for base={base}."
            );
        }
    }

    #[test]
    fn rejects_empty_measurements_zero_temperature_and_bad_fisher_shape() {
        let policy = NaturalGradientPolicy::default();
        assert_eq!(
            policy.suggest(&[], &[]),
            Err(NaturalGradientTuningError::EmptyMeasurements)
        );

        let samples = measurements();
        let zero_temp = NaturalGradientPolicy { temperature_ns: 0 };
        assert_eq!(
            zero_temp.suggest(&samples, &identity_fisher_q16(samples.len())),
            Err(NaturalGradientTuningError::ZeroTemperature)
        );
        assert_eq!(
            policy.suggest(&samples, &[Q16_ONE]),
            Err(NaturalGradientTuningError::FisherMatrixShape {
                measurements: samples.len(),
                cells: 1,
            })
        );
    }

    #[test]
    fn tuner_exposes_natural_gradient_step_surface() {
        let tuner = Tuner::new("natural-gradient-test-adapter", Mode::OffUseDefault);
        let samples = measurements();
        let step = tuner
            .natural_gradient_step(
                &samples,
                &identity_fisher_q16(samples.len()),
                NaturalGradientPolicy::default(),
            )
            .expect("Fix: tuner natural-gradient policy surface should accept identity Fisher");

        assert_eq!(step.selected_workgroup_size, [128, 1, 1]);
    }

    #[test]
    fn measured_natural_gradient_sweep_uses_backend_timer_and_fisher_policy() {
        let tuner = Tuner::new(
            "measured-natural-gradient-test-adapter",
            Mode::NaturalGradient,
        );
        let mut timer = StaticTimer::new();
        let mut fisher = identity_fisher_q16(3);
        fisher[0] = Q16_ONE * 8;

        let step = tuner
            .best_of_natural_gradient(
                &empty_program(),
                [[64, 1, 1], [128, 1, 1], [256, 1, 1]],
                &mut timer,
                &fisher,
                NaturalGradientPolicy {
                    temperature_ns: 4_000,
                },
            )
            .expect("Fix: backend timer should succeed")
            .expect("Fix: natural-gradient policy should accept measured candidates");

        assert_eq!(
            timer.measured,
            vec![[64, 1, 1], [128, 1, 1], [256, 1, 1]],
            "Fix: natural-gradient sweep must measure every supplied candidate."
        );
        assert_eq!(step.best_measured_workgroup_size, [128, 1, 1]);
        assert_eq!(
            step.selected_workgroup_size,
            [64, 1, 1],
            "Fix: measured natural-gradient sweep must use Fisher policy, not raw fastest-only selection."
        );
    }

    #[test]
    fn measured_natural_gradient_sweep_propagates_timer_failures() {
        let tuner = Tuner::new(
            "measured-natural-gradient-error-test-adapter",
            Mode::NaturalGradient,
        );
        let mut timer = StaticTimer::failing(128);
        let err = tuner
            .best_of_natural_gradient(
                &empty_program(),
                [[64, 1, 1], [128, 1, 1], [256, 1, 1]],
                &mut timer,
                &identity_fisher_q16(3),
                NaturalGradientPolicy::default(),
            )
            .expect_err("Fix: backend timer failures must propagate before policy update");

        assert_eq!(err, "timer failed");
        assert_eq!(
            timer.measured,
            vec![[64, 1, 1], [128, 1, 1]],
            "Fix: failed measurements must stop the sweep instead of producing a fake policy result."
        );
    }
}
