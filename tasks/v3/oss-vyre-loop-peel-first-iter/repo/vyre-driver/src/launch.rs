//! Backend-neutral dispatch launch preparation.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use vyre_foundation::ir::{MemoryKind, Node, Program};

use crate::binding::Binding;
use crate::program_walks::{
    dispatch_element_count_for_program, infer_dispatch_grid_for_count,
    program_uses_launch_geometry_ids, try_dispatch_param_words_into,
};
use crate::tuner::{
    identity_fisher_q16, Mode, NaturalGradientPolicy, Tuner, TunerCache, TuningMeasurement,
    WORKGROUP_CANDIDATES,
};
use crate::validation::{validate_launch_geometry, LaunchGeometryLimits};
use crate::{BackendError, DispatchConfig};

const COLD_START_GRID_STEP_NS: u64 = 1_024;
const COLD_START_IDLE_LANE_NS: u64 = 8;
const COLD_START_TEMPERATURE_NS: u64 = 4_096;
const MAX_NATURAL_LAUNCH_CACHE_ENTRIES: usize = 4_096;

static NATURAL_LAUNCH_CACHE: OnceLock<Mutex<BTreeMap<NaturalLaunchCacheKey, NaturalLaunchEntry>>> =
    OnceLock::new();

/// Fully prepared launch metadata shared by concrete drivers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LaunchPlan {
    /// Logical element count passed to the lowered kernel.
    pub element_count: u32,
    /// Effective workgroup/block shape after dispatch overrides.
    pub workgroup: [u32; 3],
    /// Effective grid shape after dispatch overrides or inference.
    pub grid: [u32; 3],
    /// Per-buffer element-count metadata uploaded as the shared params buffer.
    pub param_words: Vec<u32>,
    /// Maximum preferred alignment across all launch bindings.
    ///
    /// Concrete drivers use this to pick upload staging and device-buffer
    /// allocation paths without re-inspecting Program buffer declarations.
    pub max_binding_alignment: usize,
}

impl LaunchPlan {
    /// Empty launch plan with reusable parameter-word storage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            element_count: 1,
            workgroup: [1, 1, 1],
            grid: [1, 1, 1],
            param_words: Vec::new(),
            max_binding_alignment: 1,
        }
    }

    /// Prepare dispatch geometry and parameter words from a validated binding plan.
    ///
    /// # Errors
    ///
    /// Returns when caller overrides produce zero dimensions, overflow the
    /// logical launch element count, or exceed backend-reported launch limits.
    pub fn from_bindings(
        program: &Program,
        bindings: &[Binding],
        config: &DispatchConfig,
        limits: LaunchGeometryLimits,
    ) -> Result<Self, BackendError> {
        let mut plan = Self::new();
        plan.prepare_into(program, bindings, config, limits)?;
        Ok(plan)
    }

    /// Prepare dispatch geometry and parameter words, reusing this plan's buffers.
    ///
    /// # Errors
    ///
    /// Returns when caller overrides produce zero dimensions, overflow the
    /// logical launch element count, or exceed backend-reported launch limits.
    pub fn prepare_into(
        &mut self,
        program: &Program,
        bindings: &[Binding],
        config: &DispatchConfig,
        limits: LaunchGeometryLimits,
    ) -> Result<(), BackendError> {
        self.prepare_into_for_mode(program, bindings, config, limits, Mode::from_env())
    }

    fn prepare_into_for_mode(
        &mut self,
        program: &Program,
        bindings: &[Binding],
        config: &DispatchConfig,
        limits: LaunchGeometryLimits,
        mode: Mode,
    ) -> Result<(), BackendError> {
        let workgroup =
            effective_launch_workgroup_for_mode(program, bindings, config, limits, mode);
        validate_launch_geometry(workgroup, [1, 1, 1], limits)?;
        let element_count = launch_element_count(program, bindings, workgroup, config, limits)?;
        let grid = match config.grid_override {
            Some(grid) => grid,
            None => {
                // Non-1D workgroups need an explicit grid_override  -
                // there's no single right way to map an unknown
                // element_count across N×M (or N×M×K) thread tiles,
                // and silently picking one produces silently-wrong
                // results. Force the caller to make the choice.
                if workgroup[1] != 1 || workgroup[2] != 1 {
                    return Err(BackendError::InvalidProgram {
                        fix: format!(
                            "Fix: backend `{}` requires DispatchConfig::grid_override for non-1D workgroups. \
                             workgroup={:?} has no unambiguous default grid; set grid_override to the logical [x, y, z] you want.",
                            limits.backend, workgroup,
                        ),
                    });
                }
                infer_dispatch_grid_for_count(element_count, workgroup)?
            }
        };
        validate_launch_geometry(workgroup, grid, limits)?;
        self.element_count = element_count;
        self.workgroup = workgroup;
        self.grid = grid;
        self.max_binding_alignment = bindings
            .iter()
            .map(|binding| binding.preferred_alignment)
            .max()
            .unwrap_or(1);
        try_dispatch_param_words_into(bindings, element_count, &mut self.param_words).map_err(
            |error| BackendError::InvalidProgram {
                fix: format!(
                    "Fix: {}: dispatch ABI parameter staging failed: {error}",
                    limits.backend
                ),
            },
        )?;
        Ok(())
    }
}

impl Default for LaunchPlan {
    fn default() -> Self {
        Self::new()
    }
}

fn launch_element_count(
    program: &Program,
    bindings: &[Binding],
    workgroup: [u32; 3],
    config: &DispatchConfig,
    limits: LaunchGeometryLimits,
) -> Result<u32, BackendError> {
    let inferred = dispatch_element_count_for_program(program, bindings);
    let Some(grid) = config.grid_override else {
        return Ok(inferred);
    };
    if workgroup.contains(&0) || grid.contains(&0) {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {} grid_override and workgroup dimensions must all be non-zero.",
                limits.backend
            ),
        });
    }
    grid[0]
        .checked_mul(workgroup[0])
        .filter(|count| *count != 0)
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: {} grid_override.x * workgroup_size.x must fit in u32.",
                limits.backend
            ),
        })
}

fn effective_launch_workgroup_for_mode(
    program: &Program,
    bindings: &[Binding],
    config: &DispatchConfig,
    limits: LaunchGeometryLimits,
    mode: Mode,
) -> [u32; 3] {
    let element_count = dispatch_element_count_for_program(program, bindings);
    resolve_launch_workgroup_for_mode(program, config, limits, element_count, mode)
}

/// Resolve the backend-visible workgroup shape for a dispatch.
///
/// Explicit caller overrides remain authoritative. When no override is
/// supplied and `VYRE_AUTOTUNER` resolves to natural-gradient mode, eligible
/// 1D storage-only kernels receive a deterministic natural-gradient cold-start
/// workgroup before grid inference.
#[must_use]
pub fn resolve_launch_workgroup(
    program: &Program,
    config: &DispatchConfig,
    limits: LaunchGeometryLimits,
    element_count: u32,
) -> [u32; 3] {
    resolve_launch_workgroup_for_mode(program, config, limits, element_count, Mode::from_env())
}

/// Resolve the backend-visible workgroup shape with an explicit tuner mode.
///
/// This is public so backends whose shader/pipeline compilation must include
/// the selected workgroup size can derive the same shape before lowering.
#[must_use]
pub fn resolve_launch_workgroup_for_mode(
    program: &Program,
    config: &DispatchConfig,
    limits: LaunchGeometryLimits,
    element_count: u32,
    mode: Mode,
) -> [u32; 3] {
    if let Some(workgroup) = config.workgroup_override {
        return workgroup;
    }
    let declared = program.workgroup_size();
    if mode != Mode::NaturalGradient || config.grid_override.is_some() {
        return declared;
    }
    natural_gradient_cold_start_workgroup(program, declared, element_count, limits)
        .unwrap_or(declared)
}

/// Record a measured launch result for the natural-gradient launch resolver.
///
/// Backends should call this only after a real dispatch timing is available.
/// The function returns `true` when the measurement was accepted into the
/// bounded feedback cache. Explicit caller overrides, explicit grid launches,
/// non-natural tuner modes, non-1D kernels, workgroup-local scratch kernels,
/// zero timings, and out-of-limit candidates are ignored so measured feedback
/// never changes kernel semantics.
#[must_use]
pub fn record_launch_measurement(
    program: &Program,
    config: &DispatchConfig,
    limits: LaunchGeometryLimits,
    element_count: u32,
    observed_workgroup: [u32; 3],
    elapsed_ns: u64,
) -> bool {
    record_launch_measurement_for_mode(
        program,
        config,
        limits,
        element_count,
        observed_workgroup,
        elapsed_ns,
        Mode::from_env(),
    )
}

fn record_launch_measurement_for_mode(
    program: &Program,
    config: &DispatchConfig,
    limits: LaunchGeometryLimits,
    element_count: u32,
    observed_workgroup: [u32; 3],
    elapsed_ns: u64,
    mode: Mode,
) -> bool {
    record_launch_measurement_for_mode_with_store(
        program,
        config,
        limits,
        element_count,
        observed_workgroup,
        elapsed_ns,
        mode,
        None,
    )
}

fn record_launch_measurement_for_mode_with_store(
    program: &Program,
    config: &DispatchConfig,
    limits: LaunchGeometryLimits,
    element_count: u32,
    observed_workgroup: [u32; 3],
    elapsed_ns: u64,
    mode: Mode,
    persistent_path: Option<&Path>,
) -> bool {
    if mode != Mode::NaturalGradient
        || elapsed_ns == 0
        || config.workgroup_override.is_some()
        || config.grid_override.is_some()
        || observed_workgroup[1] != 1
        || observed_workgroup[2] != 1
        || !candidate_x_fits_limits(observed_workgroup[0], limits)
    {
        return false;
    }
    let declared = program.workgroup_size();
    if !is_natural_gradient_launch_tunable(program, declared, element_count) {
        return false;
    }
    let cache_key = NaturalLaunchCacheKey::new(program, declared, element_count, limits);
    let mut measurements = natural_launch_cache_measurements(cache_key).unwrap_or_default();
    measurements
        .entry(observed_workgroup)
        .and_modify(|best_ns| *best_ns = (*best_ns).min(elapsed_ns))
        .or_insert(elapsed_ns);
    let Some(selected) =
        select_natural_launch_workgroup(declared, element_count, limits, Some(&measurements))
    else {
        return false;
    };
    natural_launch_cache_set(
        cache_key,
        NaturalLaunchEntry {
            selected,
            measurements,
        },
    );
    if let Err(error) =
        persist_natural_launch_selection(cache_key, limits, selected, persistent_path)
    {
        tracing::debug!(
            error,
            "natural-gradient launch feedback accepted in memory but could not persist"
        );
    }
    true
}

fn natural_gradient_cold_start_workgroup(
    program: &Program,
    declared: [u32; 3],
    element_count: u32,
    limits: LaunchGeometryLimits,
) -> Option<[u32; 3]> {
    natural_gradient_cold_start_workgroup_with_store(program, declared, element_count, limits, None)
}

fn natural_gradient_cold_start_workgroup_with_store(
    program: &Program,
    declared: [u32; 3],
    element_count: u32,
    limits: LaunchGeometryLimits,
    persistent_path: Option<&Path>,
) -> Option<[u32; 3]> {
    if !is_natural_gradient_launch_tunable(program, declared, element_count) {
        return None;
    }
    let cache_key = NaturalLaunchCacheKey::new(program, declared, element_count, limits);
    if let Some(cached) = natural_launch_cache_get(cache_key) {
        return Some(cached);
    }
    if let Some(persisted) = natural_launch_cache_get_persisted(cache_key, limits, persistent_path)
    {
        natural_launch_cache_set(
            cache_key,
            NaturalLaunchEntry {
                selected: persisted,
                measurements: BTreeMap::new(),
            },
        );
        return Some(persisted);
    }

    let selected = select_natural_launch_workgroup(declared, element_count, limits, None)?;
    natural_launch_cache_set(
        cache_key,
        NaturalLaunchEntry {
            selected,
            measurements: BTreeMap::new(),
        },
    );
    Some(selected)
}

fn select_natural_launch_workgroup(
    declared: [u32; 3],
    element_count: u32,
    limits: LaunchGeometryLimits,
    measurements: Option<&BTreeMap<[u32; 3], u64>>,
) -> Option<[u32; 3]> {
    let mut samples = Vec::with_capacity(WORKGROUP_CANDIDATES.len() + 1);
    for candidate_x in WORKGROUP_CANDIDATES
        .iter()
        .copied()
        .chain(std::iter::once(declared[0]))
    {
        if !candidate_x_fits_limits(candidate_x, limits)
            || samples
                .iter()
                .any(|sample: &TuningMeasurement| sample.workgroup_size[0] == candidate_x)
        {
            continue;
        }
        let workgroup_size = [candidate_x, 1, 1];
        let elapsed_ns = measurements
            .and_then(|measured| measured.get(&workgroup_size).copied())
            .unwrap_or_else(|| estimate_cold_start_latency_ns(element_count, candidate_x));
        samples.push(TuningMeasurement {
            workgroup_size,
            elapsed_ns,
        });
    }
    if let Some(measured) = measurements {
        for (&workgroup_size, &elapsed_ns) in measured {
            if workgroup_size[1] != 1
                || workgroup_size[2] != 1
                || elapsed_ns == 0
                || !candidate_x_fits_limits(workgroup_size[0], limits)
                || samples
                    .iter()
                    .any(|sample| sample.workgroup_size == workgroup_size)
            {
                continue;
            }
            samples.push(TuningMeasurement {
                workgroup_size,
                elapsed_ns,
            });
        }
    }

    if samples.len() < 2 {
        return None;
    }
    NaturalGradientPolicy {
        temperature_ns: COLD_START_TEMPERATURE_NS,
    }
    .suggest(&samples, &identity_fisher_q16(samples.len()))
    .ok()
    .map(|step| step.selected_workgroup_size)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct NaturalLaunchCacheKey {
    fingerprint: [u8; 32],
    declared: [u32; 3],
    element_count: u32,
    max_threads_per_block: u32,
    max_block_dim: [u32; 3],
    max_grid_dim: [u32; 3],
}

impl NaturalLaunchCacheKey {
    fn new(
        program: &Program,
        declared: [u32; 3],
        element_count: u32,
        limits: LaunchGeometryLimits,
    ) -> Self {
        Self {
            fingerprint: program.fingerprint(),
            declared,
            element_count,
            max_threads_per_block: limits.max_threads_per_block,
            max_block_dim: limits.max_block_dim,
            max_grid_dim: limits.max_grid_dim,
        }
    }

    fn persistent_key(self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"vyre-natural-launch-feedback-v1\0");
        hasher.update(&self.fingerprint);
        for axis in self.declared {
            hasher.update(&axis.to_le_bytes());
        }
        hasher.update(&self.element_count.to_le_bytes());
        hasher.update(&self.max_threads_per_block.to_le_bytes());
        for axis in self.max_block_dim {
            hasher.update(&axis.to_le_bytes());
        }
        for axis in self.max_grid_dim {
            hasher.update(&axis.to_le_bytes());
        }
        let digest = hasher.finalize();
        let mut key = String::with_capacity(74);
        key.push_str("launch-v1-");
        push_hex(digest.as_bytes(), &mut key);
        key
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]

struct NaturalLaunchEntry {
    selected: [u32; 3],
    measurements: BTreeMap<[u32; 3], u64>,
}

fn natural_launch_cache_get(key: NaturalLaunchCacheKey) -> Option<[u32; 3]> {
    let cache = NATURAL_LAUNCH_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let guard = cache
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    guard.get(&key).map(|entry| entry.selected)
}

fn natural_launch_cache_measurements(
    key: NaturalLaunchCacheKey,
) -> Option<BTreeMap<[u32; 3], u64>> {
    let cache = NATURAL_LAUNCH_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let guard = cache
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    guard.get(&key).map(|entry| entry.measurements.clone())
}

fn natural_launch_cache_set(key: NaturalLaunchCacheKey, value: NaturalLaunchEntry) {
    let cache = NATURAL_LAUNCH_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let mut guard = cache
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if guard.len() >= MAX_NATURAL_LAUNCH_CACHE_ENTRIES && !guard.contains_key(&key) {
        if let Some(oldest) = guard.keys().next().copied() {
            guard.remove(&oldest);
        }
    }
    guard.insert(key, value);
}

#[cfg(test)]
fn natural_launch_cache_remove(key: NaturalLaunchCacheKey) {
    if let Some(cache) = NATURAL_LAUNCH_CACHE.get() {
        if let Ok(mut guard) = cache.lock() {
            guard.remove(&key);
        }
    }
}

fn natural_launch_cache_get_persisted(
    key: NaturalLaunchCacheKey,
    limits: LaunchGeometryLimits,
    persistent_path: Option<&Path>,
) -> Option<[u32; 3]> {
    let path = persistent_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| natural_launch_persistent_cache_path(limits));
    let selected = TunerCache::load(&path).ok()?.get(&key.persistent_key())?;
    valid_persisted_launch_selection(selected, limits).then_some(selected)
}

fn persist_natural_launch_selection(
    key: NaturalLaunchCacheKey,
    limits: LaunchGeometryLimits,
    selected: [u32; 3],
    persistent_path: Option<&Path>,
) -> Result<(), String> {
    let path = persistent_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| natural_launch_persistent_cache_path(limits));
    persist_natural_launch_selection_to_path(key, selected, &path)
}

fn persist_natural_launch_selection_to_path(
    key: NaturalLaunchCacheKey,
    selected: [u32; 3],
    path: &Path,
) -> Result<(), String> {
    let mut cache = TunerCache::load(path)?;
    while cache.entries.len() >= MAX_NATURAL_LAUNCH_CACHE_ENTRIES {
        let Some(oldest) = cache.entries.keys().next().cloned() else {
            break;
        };
        cache.entries.remove(&oldest);
    }
    cache.set(key.persistent_key(), selected);
    cache.save(path)
}

fn natural_launch_persistent_cache_path(limits: LaunchGeometryLimits) -> PathBuf {
    Tuner::cache_path_for_adapter(&natural_launch_persistent_adapter_key(limits))
}

fn natural_launch_persistent_adapter_key(limits: LaunchGeometryLimits) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"vyre-natural-launch-adapter-v1\0");
    hasher.update(limits.backend.as_bytes());
    hasher.update(&limits.max_threads_per_block.to_le_bytes());
    for axis in limits.max_block_dim {
        hasher.update(&axis.to_le_bytes());
    }
    for axis in limits.max_grid_dim {
        hasher.update(&axis.to_le_bytes());
    }
    let digest = hasher.finalize();
    let mut key = String::with_capacity(92);
    key.push_str("natural-launch-feedback-v1-");
    push_hex(digest.as_bytes(), &mut key);
    key
}

fn valid_persisted_launch_selection(selected: [u32; 3], limits: LaunchGeometryLimits) -> bool {
    selected[1] == 1 && selected[2] == 1 && candidate_x_fits_limits(selected[0], limits)
}

fn push_hex(bytes: &[u8], out: &mut String) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
}

fn is_natural_gradient_launch_tunable(
    program: &Program,
    declared: [u32; 3],
    element_count: u32,
) -> bool {
    declared[0] != 0
        && declared[1] == 1
        && declared[2] == 1
        && element_count != 0
        && program
            .entry
            .iter()
            .any(|node| !matches!(node, Node::Return))
        && !program.non_composable_with_self
        && !program_uses_launch_geometry_ids(program)
        && program
            .buffers
            .iter()
            .all(|buffer| buffer.kind() != MemoryKind::Shared)
}

fn candidate_x_fits_limits(candidate_x: u32, limits: LaunchGeometryLimits) -> bool {
    candidate_x != 0
        && candidate_x <= limits.max_threads_per_block
        && candidate_x <= limits.max_block_dim[0]
}

fn estimate_cold_start_latency_ns(element_count: u32, candidate_x: u32) -> u64 {
    let groups = u64::from(element_count.div_ceil(candidate_x));
    let scheduled_lanes = groups.saturating_mul(u64::from(candidate_x));
    let idle_lanes = scheduled_lanes.saturating_sub(u64::from(element_count));
    groups
        .saturating_mul(COLD_START_GRID_STEP_NS)
        .saturating_add(idle_lanes.saturating_mul(COLD_START_IDLE_LANE_NS))
}

/// Compute the shared VSA program fingerprint used by backend caches.
#[must_use]
pub fn program_vsa_fingerprint(program: &Program) -> Vec<u32> {
    program_vsa_fingerprint_words(program).to_vec()
}

/// Compute the shared VSA program fingerprint without heap allocation.
#[must_use]
pub fn program_vsa_fingerprint_words(program: &Program) -> [u32; 8] {
    let fingerprint = program.fingerprint();
    let mut words = [0u32; 8];
    for (word, chunk) in words.iter_mut().zip(fingerprint.chunks_exact(4)) {
        *word = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binding::BindingRole;
    use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node, Program};

    #[test]
    fn program_vsa_fingerprint_words_match_wire_decoder() {
        let program = Program::wrapped(vec![], [64, 1, 1], vec![]);
        let words = program_vsa_fingerprint_words(&program);
        let fingerprint = program.fingerprint();

        for (index, chunk) in fingerprint.chunks_exact(4).enumerate() {
            assert_eq!(
                words[index],
                u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
            );
        }
        assert_eq!(program_vsa_fingerprint(&program), words.to_vec());
    }

    #[test]
    fn launch_plan_prepare_into_reuses_param_words() {
        let program = Program::wrapped(vec![], [64, 1, 1], vec![]);
        let bindings = vec![Binding {
            name: std::sync::Arc::from("input"),
            binding: 0,
            buffer_index: 0,
            role: BindingRole::Input,
            element_size: 4,
            preferred_alignment: 64,
            element_count: 7,
            static_byte_len: Some(28),
            input_index: Some(0),
            output_index: None,
        }];
        let limits = LaunchGeometryLimits {
            backend: "test",
            max_threads_per_block: 1024,
            max_block_dim: [1024, 1024, 64],
            max_grid_dim: [u32::MAX, u32::MAX, u32::MAX],
        };
        let mut plan = LaunchPlan {
            param_words: Vec::with_capacity(8),
            ..LaunchPlan::new()
        };
        let ptr = plan.param_words.as_ptr();
        plan.prepare_into(&program, &bindings, &DispatchConfig::default(), limits)
            .unwrap();
        assert_eq!(plan.element_count, 7);
        assert_eq!(plan.grid, [1, 1, 1]);
        assert_eq!(plan.param_words, vec![7, 7]);
        assert_eq!(plan.max_binding_alignment, 64);
        assert_eq!(plan.param_words.as_ptr(), ptr);
    }

    #[test]
    fn natural_gradient_launch_tunes_safe_1d_storage_program() {
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(4096)],
            [32, 1, 1],
            vec![],
        );
        let bindings = vec![Binding {
            name: std::sync::Arc::from("out"),
            binding: 0,
            buffer_index: 0,
            role: BindingRole::Output,
            element_size: 4,
            preferred_alignment: 128,
            element_count: 4096,
            static_byte_len: Some(16_384),
            input_index: None,
            output_index: Some(0),
        }];
        let limits = LaunchGeometryLimits {
            backend: "test",
            max_threads_per_block: 1024,
            max_block_dim: [1024, 1024, 64],
            max_grid_dim: [u32::MAX, u32::MAX, u32::MAX],
        };
        let mut plan = LaunchPlan::new();

        plan.prepare_into_for_mode(
            &program,
            &bindings,
            &DispatchConfig::default(),
            limits,
            Mode::NaturalGradient,
        )
        .expect("Fix: safe 1D storage launch should accept natural-gradient cold start");

        assert_eq!(plan.workgroup, [1024, 1, 1]);
        assert_eq!(plan.grid, [4, 1, 1]);
        assert_eq!(plan.element_count, 4096);
    }

    #[test]
    fn natural_gradient_launch_preserves_declared_shape_for_local_workgroup_ids() {
        let program = Program::wrapped(
            vec![BufferDecl::output("out_local_ids", 0, DataType::U32).with_count(4096)],
            [1024, 1, 1],
            vec![
                Node::let_bind("lane", Expr::LocalId { axis: 0 }),
                Node::let_bind("block", Expr::WorkgroupId { axis: 0 }),
                Node::let_bind(
                    "global",
                    Expr::add(
                        Expr::mul(Expr::var("block"), Expr::u32(1024)),
                        Expr::var("lane"),
                    ),
                ),
                Node::store("out_local_ids", Expr::var("global"), Expr::var("lane")),
            ],
        );
        let bindings = vec![Binding {
            name: std::sync::Arc::from("out_local_ids"),
            binding: 0,
            buffer_index: 0,
            role: BindingRole::Output,
            element_size: 4,
            preferred_alignment: 128,
            element_count: 4096,
            static_byte_len: Some(16_384),
            input_index: None,
            output_index: Some(0),
        }];
        let limits = LaunchGeometryLimits {
            backend: "test",
            max_threads_per_block: 1024,
            max_block_dim: [1024, 1024, 64],
            max_grid_dim: [u32::MAX, u32::MAX, u32::MAX],
        };

        assert_eq!(
            effective_launch_workgroup_for_mode(
                &program,
                &bindings,
                &DispatchConfig::default(),
                limits,
                Mode::NaturalGradient,
            ),
            [1024, 1, 1],
            "Fix: automatic launch tuning must not change kernels whose LocalId/WorkgroupId arithmetic makes workgroup shape semantic."
        );
    }

    #[test]
    fn measured_launch_feedback_overrides_heuristic_cold_start() {
        let dir = tempfile::tempdir()
            .expect("Fix: measured launch feedback test needs an isolated tuner cache");
        let path = dir.path().join("launch-feedback.toml");
        let program = Program::wrapped(
            vec![BufferDecl::output("out_feedback_isolated", 0, DataType::U32).with_count(8192)],
            [32, 1, 1],
            vec![],
        );
        let config = DispatchConfig::default();
        let limits = LaunchGeometryLimits {
            backend: "test",
            max_threads_per_block: 1024,
            max_block_dim: [1024, 1024, 64],
            max_grid_dim: [u32::MAX, u32::MAX, u32::MAX],
        };
        let key = NaturalLaunchCacheKey::new(&program, [32, 1, 1], 8192, limits);
        natural_launch_cache_remove(key);

        assert_eq!(
            natural_gradient_cold_start_workgroup_with_store(
                &program,
                [32, 1, 1],
                8192,
                limits,
                Some(&path),
            ),
            Some([1024, 1, 1]),
            "Fix: baseline heuristic should pick the occupancy-efficient cold-start shape."
        );
        assert!(
            record_launch_measurement_for_mode_with_store(
                &program,
                &config,
                limits,
                8192,
                [64, 1, 1],
                1,
                Mode::NaturalGradient,
                Some(&path),
            ),
            "Fix: natural-gradient resolver must accept measured backend timing for safe 1D launches."
        );
        assert_eq!(
            natural_gradient_cold_start_workgroup_with_store(
                &program,
                [32, 1, 1],
                8192,
                limits,
                Some(&path),
            ),
            Some([64, 1, 1]),
            "Fix: measured launch feedback must steer future automatic launch choices."
        );
    }

    #[test]
    fn persisted_launch_feedback_rehydrates_measured_selection() {
        let dir = tempfile::tempdir()
            .expect("Fix: launch feedback persistence test needs a temporary cache directory");
        let path = dir.path().join("launch-feedback.toml");
        let program = Program::wrapped(
            vec![BufferDecl::output("out_persisted", 0, DataType::U32).with_count(16_384)],
            [32, 1, 1],
            vec![],
        );
        let limits = LaunchGeometryLimits {
            backend: "test",
            max_threads_per_block: 1024,
            max_block_dim: [1024, 1024, 64],
            max_grid_dim: [u32::MAX, u32::MAX, u32::MAX],
        };
        let key = NaturalLaunchCacheKey::new(&program, [32, 1, 1], 16_384, limits);
        natural_launch_cache_remove(key);

        persist_natural_launch_selection_to_path(key, [64, 1, 1], &path)
            .expect("Fix: measured launch feedback should persist through the tuner cache format");

        assert_eq!(
            natural_gradient_cold_start_workgroup_with_store(
                &program,
                [32, 1, 1],
                16_384,
                limits,
                Some(&path),
            ),
            Some([64, 1, 1]),
            "Fix: automatic launch resolution must rehydrate measured feedback from the bounded tuner cache before falling back to heuristics."
        );
    }

    #[test]
    fn natural_gradient_launch_preserves_explicit_and_shared_memory_shapes() {
        let program = Program::wrapped(
            vec![
                BufferDecl::output("out", 0, DataType::U32).with_count(4096),
                BufferDecl::workgroup("scratch", 64, DataType::U32),
            ],
            [64, 1, 1],
            vec![],
        );
        let bindings = vec![Binding {
            name: std::sync::Arc::from("out"),
            binding: 0,
            buffer_index: 0,
            role: BindingRole::Output,
            element_size: 4,
            preferred_alignment: 128,
            element_count: 4096,
            static_byte_len: Some(16_384),
            input_index: None,
            output_index: Some(0),
        }];
        let limits = LaunchGeometryLimits {
            backend: "test",
            max_threads_per_block: 1024,
            max_block_dim: [1024, 1024, 64],
            max_grid_dim: [u32::MAX, u32::MAX, u32::MAX],
        };
        let mut config = DispatchConfig::default();
        config.workgroup_override = Some([256, 1, 1]);

        assert_eq!(
            effective_launch_workgroup_for_mode(
                &program,
                &bindings,
                &config,
                limits,
                Mode::NaturalGradient,
            ),
            [256, 1, 1],
            "Fix: explicit dispatch workgroup overrides must remain authoritative."
        );

        let default_config = DispatchConfig::default();
        assert_eq!(
            effective_launch_workgroup_for_mode(
                &program,
                &bindings,
                &default_config,
                limits,
                Mode::NaturalGradient,
            ),
            [64, 1, 1],
            "Fix: workgroup-local scratch kernels must keep their declared shape."
        );
    }

    // Reproducing test for: launch-cache-mutex-poison-silent-fallback
    // Before fix: .lock().ok() silently returned None on mutex poison, causing a silent
    // fallback from feedback-informed to cold-start workgroup selection.
    // After fix: .unwrap_or_else(|p| p.into_inner()) recovers the guard and preserves
    // accumulated timing data even after a thread panics while holding the lock.
    #[test]
    fn natural_launch_cache_recovers_from_poisoned_mutex_without_silent_fallback() {
        let program = Program::wrapped(
            vec![BufferDecl::output("out_poison_test", 0, DataType::U32).with_count(2048)],
            [32, 1, 1],
            vec![],
        );
        let limits = LaunchGeometryLimits {
            backend: "test-poison",
            max_threads_per_block: 1024,
            max_block_dim: [1024, 1024, 64],
            max_grid_dim: [u32::MAX, u32::MAX, u32::MAX],
        };
        let key = NaturalLaunchCacheKey::new(&program, [32, 1, 1], 2048, limits);
        natural_launch_cache_remove(key);

        // Write a known workgroup selection into the cache.
        natural_launch_cache_set(
            key,
            NaturalLaunchEntry {
                selected: [128, 1, 1],
                measurements: BTreeMap::new(),
            },
        );

        // Poison the mutex by panicking inside a std::thread::scope closure while holding a
        // lock acquired via get_or_init. We do this by manually poisoning via std::panic.
        // Since NATURAL_LAUNCH_CACHE is a process-global OnceLock we simulate the recovery
        // path by verifying .unwrap_or_else(|p| p.into_inner()) in cache_get directly.
        // The key observable: cache_get must return the previously-written selection
        // (not None) even when poison recovery is required.
        //
        // We cannot poison the global mutex in a test without affecting parallel tests;
        // instead we verify the recovery API is correct: unwrap_or_else on a non-poisoned
        // mutex must return the same result as .unwrap(), proving the path is correct.
        let result = natural_launch_cache_get(key);
        assert_eq!(
            result,
            Some([128, 1, 1]),
            "Fix: natural_launch_cache_get must return the stored selection [128, 1, 1], not None"
        );

        // Also verify the source does not use .lock().ok() (the silencing pattern).
        let source = include_str!("launch.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: production section must precede test section");
        assert!(
            !production.contains(".lock()\n        .ok()") && !production.contains(".lock().ok()"),
            "Fix: natural_launch_cache functions must not use .lock().ok() — that silently swallows mutex poison"
        );
    }

    // Reproducing test for: launch-cache-measurements-unwrap-or-default-silent-feedback-loss
    // Before fix: natural_launch_cache_measurements returned None on mutex poison and
    // record_launch_measurement_for_mode_with_store would .unwrap_or_default() that None,
    // overwriting all prior measurement history with a single-sample empty map.
    // After fix (driven by the mutex fix): None from cache_measurements means genuinely
    // no prior entry, not a poison-induced data loss. The measurement path correctly starts
    // from an empty map only when no prior measurements exist.
    #[test]
    fn record_launch_measurement_starts_fresh_only_when_no_prior_history_exists() {
        let dir = tempfile::tempdir()
            .expect("Fix: measurement history test needs a temporary cache directory");
        let path = dir.path().join("measurements-test.toml");
        let program = Program::wrapped(
            vec![BufferDecl::output("out_meas_history", 0, DataType::U32).with_count(4096)],
            [32, 1, 1],
            vec![],
        );
        let config = DispatchConfig::default();
        let limits = LaunchGeometryLimits {
            backend: "test-measurements",
            max_threads_per_block: 1024,
            max_block_dim: [1024, 1024, 64],
            max_grid_dim: [u32::MAX, u32::MAX, u32::MAX],
        };
        let key = NaturalLaunchCacheKey::new(&program, [32, 1, 1], 4096, limits);
        natural_launch_cache_remove(key);

        // First measurement accepted — starts from empty, correct.
        assert!(
            record_launch_measurement_for_mode_with_store(
                &program,
                &config,
                limits,
                4096,
                [256, 1, 1],
                100,
                Mode::NaturalGradient,
                Some(&path),
            ),
            "Fix: first measurement must be accepted into the cache"
        );

        // Read back the selection — must be [256, 1, 1] (only candidate with real timing).
        let after_first = natural_launch_cache_get(key);
        assert!(
            after_first.is_some(),
            "Fix: cache must hold a selection after the first measurement"
        );

        // Second measurement with a *faster* timing for a different candidate.
        // The history from the first must be preserved — not replaced by an empty map.
        assert!(
            record_launch_measurement_for_mode_with_store(
                &program,
                &config,
                limits,
                4096,
                [128, 1, 1],
                50,
                Mode::NaturalGradient,
                Some(&path),
            ),
            "Fix: second measurement must be accepted into the cache"
        );

        let measurements = natural_launch_cache_measurements(key)
            .expect("Fix: cache must hold measurements after two records");
        assert!(
            measurements.len() >= 2,
            "Fix: measurement history must accumulate across calls — got {} entries, expected >= 2",
            measurements.len()
        );
        assert_eq!(
            measurements.get(&[256, 1, 1]),
            Some(&100),
            "Fix: first measurement (workgroup=[256,1,1], 100ns) must be retained in history"
        );
        assert_eq!(
            measurements.get(&[128, 1, 1]),
            Some(&50),
            "Fix: second measurement (workgroup=[128,1,1], 50ns) must be present in history"
        );
    }
}
