//! Shared validation caches and launch-geometry checks for concrete drivers.

use std::collections::HashSet;
use std::hash::BuildHasherDefault;

use rustc_hash::FxHasher;
use vyre_foundation::ir::{OpId, Program};
use vyre_foundation::validate::{BackendValidationCapabilities, ValidationOptions};

use crate::{BackendError, DispatchConfig, VyreBackend};

/// Default successful-validation hash entries retained per backend instance.
pub const DEFAULT_VALIDATION_HASH_ENTRIES: usize = 8192;
/// Default VSA fingerprints retained per backend instance.
pub const DEFAULT_VALIDATION_VSA_ENTRIES: usize = 2048;
/// Default VSA shard count.
pub const DEFAULT_VALIDATION_VSA_SHARDS: usize = 64;

type ValidationSet = dashmap::DashSet<blake3::Hash, BuildHasherDefault<FxHasher>>;

/// Successful-program validation cache shared by concrete drivers.
pub struct ValidationCache {
    hashes: ValidationSet,
    vsa_hashes: ValidationSet,
    max_hash_entries: usize,
    max_vsa_entries: usize,
    vsa_shards: usize,
}

impl std::fmt::Debug for ValidationCache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ValidationCache")
            .field("hashes", &self.hashes.len())
            .field("vsa_hashes", &self.vsa_hashes.len())
            .field("vsa_shards", &self.vsa_shards)
            .field("max_hash_entries", &self.max_hash_entries)
            .field("max_vsa_entries", &self.max_vsa_entries)
            .finish()
    }
}

impl Default for ValidationCache {
    fn default() -> Self {
        Self::new(
            DEFAULT_VALIDATION_HASH_ENTRIES,
            DEFAULT_VALIDATION_VSA_ENTRIES,
            DEFAULT_VALIDATION_VSA_SHARDS,
        )
    }
}

impl ValidationCache {
    /// Create a validation cache with bounded hash and VSA storage.
    #[must_use]
    pub fn new(max_hash_entries: usize, max_vsa_entries: usize, vsa_shards: usize) -> Self {
        let shard_count = vsa_shards.max(1);
        Self {
            hashes: dashmap::DashSet::with_hasher(BuildHasherDefault::<FxHasher>::default()),
            vsa_hashes: dashmap::DashSet::with_capacity_and_hasher(
                max_vsa_entries.max(1),
                BuildHasherDefault::<FxHasher>::default(),
            ),
            max_hash_entries: max_hash_entries.max(1),
            max_vsa_entries: max_vsa_entries.max(1),
            vsa_shards: shard_count,
        }
    }

    /// Compute the validation hash for a program.
    #[must_use]
    pub fn program_hash(program: &Program) -> blake3::Hash {
        blake3::Hash::from(program.fingerprint())
    }

    /// Return whether a validation hash is cached.
    #[must_use]
    pub fn contains_hash(&self, hash: &blake3::Hash) -> bool {
        self.hashes.contains(hash)
    }

    /// Remember a successful validation hash.
    pub fn remember_hash(&self, hash: blake3::Hash) {
        if self.hashes.len() >= self.max_hash_entries {
            self.hashes.clear();
        }
        self.hashes.insert(hash);
    }

    /// Remember a successful validation hash and its VSA fingerprint.
    ///
    /// # Errors
    ///
    /// Returns if a VSA shard lock is poisoned.
    pub fn remember_success(&self, hash: blake3::Hash, vsa: &[u32]) -> Result<(), BackendError> {
        self.remember_hash(hash);
        if self.vsa_hashes.len() >= self.max_vsa_entries {
            self.vsa_hashes.clear();
        }
        self.vsa_hashes.insert(vsa_words_hash(vsa));
        Ok(())
    }

    /// Clear cached validation state.
    ///
    /// # Errors
    ///
    /// Returns if a VSA shard lock is poisoned.
    pub fn clear(&self) -> Result<(), BackendError> {
        self.hashes.clear();
        self.vsa_hashes.clear();
        Ok(())
    }

    /// Validate `program` once, memoizing the complete backend contract.
    ///
    /// This is the shared driver validation path: foundation invariants,
    /// backend supported-op coverage, program capability requirements, and
    /// VSA cache insertion all happen in one place. Concrete drivers supply
    /// only their actual capability values.
    ///
    /// # Errors
    ///
    /// Returns when validation fails or a VSA shard lock is poisoned.
    pub fn get_or_validate(
        &self,
        program: &Program,
        validation_options: ValidationOptions<'_>,
        supported_ops: &HashSet<OpId>,
        caps: ProgramValidationCaps,
    ) -> Result<(), BackendError> {
        let hash = Self::program_hash(program);
        if self.contains_hash(&hash) || program.is_validated_on(caps.backend_id) {
            self.remember_hash(hash);
            return Ok(());
        }

        validate_program_contract(program, validation_options, supported_ops, caps)?;

        let vsa = crate::launch::program_vsa_fingerprint_words(program);
        self.remember_success(hash, &vsa)?;
        program.mark_validated_on(caps.backend_id);
        Ok(())
    }

    /// Validate `program` against a concrete backend and cache successful
    /// results.
    ///
    /// This is the canonical driver-owned validation-cache entry point for
    /// backends that implement both the runtime backend contract and the
    /// foundation capability-validation contract.
    ///
    /// # Errors
    ///
    /// Returns when validation fails or cache mutation fails.
    pub fn get_or_validate_backend<B>(
        &self,
        program: &Program,
        backend: &B,
    ) -> Result<(), BackendError>
    where
        B: VyreBackend + BackendValidationCapabilities,
    {
        let validation_options = ValidationOptions::default().with_backend(backend);
        self.get_or_validate(
            program,
            validation_options,
            backend.supported_ops(),
            ProgramValidationCaps::from_backend(backend),
        )
    }
}

/// Concrete backend capability values needed for shared program validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramValidationCaps {
    /// Stable backend identifier used in diagnostics and validation stamps.
    pub backend_id: &'static str,
    /// Native subgroup operations are available and lowered.
    pub supports_subgroup_ops: bool,
    /// IEEE binary16 buffers/operations are lowered.
    pub supports_f16: bool,
    /// Bfloat16 buffers/operations are lowered.
    pub supports_bf16: bool,
    /// Indirect dispatch is lowered.
    pub supports_indirect_dispatch: bool,
    /// Distributed collective communication nodes are lowered.
    pub supports_distributed_collectives: bool,
    /// `Node::Trap` is lowered with backend-visible trap semantics.
    pub supports_trap_propagation: bool,
    /// Maximum supported workgroup dimensions.
    pub max_workgroup_size: [u32; 3],
}

impl ProgramValidationCaps {
    /// Snapshot capability values from a `VyreBackend` trait object.
    #[must_use]
    pub fn from_backend(backend: &dyn VyreBackend) -> Self {
        Self {
            backend_id: backend.id(),
            supports_subgroup_ops: backend.supports_subgroup_ops(),
            supports_f16: backend.supports_f16(),
            supports_bf16: backend.supports_bf16(),
            supports_indirect_dispatch: backend.supports_indirect_dispatch(),
            supports_distributed_collectives: backend.supports_distributed_collectives(),
            supports_trap_propagation: true,
            max_workgroup_size: backend.max_workgroup_size(),
        }
    }
}

/// Validate a program against backend-neutral and backend-reported contracts.
///
/// # Errors
///
/// Returns when foundation validation, supported-op validation, or required
/// capability checks fail.
pub fn validate_program_contract(
    program: &Program,
    validation_options: ValidationOptions<'_>,
    supported_ops: &HashSet<OpId>,
    caps: ProgramValidationCaps,
) -> Result<(), BackendError> {
    let lowered_program = if caps.supports_distributed_collectives {
        None
    } else {
        vyre_foundation::transform::collectives::lower_single_rank_collectives(program).map_err(
            |error| BackendError::InvalidProgram {
                fix: error.to_string(),
            },
        )?
    };
    let program = lowered_program.as_ref().unwrap_or(program);
    let report = vyre_foundation::validate::validate_with_options(program, validation_options);
    if let Some(first) = report.errors.into_iter().next() {
        return Err(BackendError::InvalidProgram {
            fix: first.message.into_owned(),
        });
    }

    validate_supported_ops(program, caps.backend_id, supported_ops).map_err(|error| {
        BackendError::InvalidProgram {
            fix: error.to_string(),
        }
    })?;

    let required = vyre_foundation::program_caps::scan(program);
    vyre_foundation::program_caps::check_backend_capabilities(
        caps.backend_id,
        caps.supports_subgroup_ops,
        caps.supports_f16,
        caps.supports_bf16,
        caps.supports_indirect_dispatch,
        caps.supports_trap_propagation,
        caps.supports_distributed_collectives,
        caps.max_workgroup_size,
        &required,
    )
    .map_err(|error| BackendError::InvalidProgram {
        fix: error.to_string(),
    })
}

fn validate_supported_ops(
    program: &Program,
    backend_id: &'static str,
    supported_ops: &HashSet<OpId>,
) -> Result<(), vyre_foundation::ir::ValidationError> {
    struct SupportedOpsBackend<'a> {
        id: &'static str,
        ops: &'a HashSet<OpId>,
    }

    impl crate::backend::Backend for SupportedOpsBackend<'_> {
        fn id(&self) -> &'static str {
            self.id
        }

        fn version(&self) -> &'static str {
            env!("CARGO_PKG_VERSION")
        }

        fn supported_ops(&self) -> &HashSet<OpId> {
            self.ops
        }
    }

    crate::backend::validation::validate_program(
        program,
        &SupportedOpsBackend {
            id: backend_id,
            ops: supported_ops,
        },
    )
}

/// Launch-geometry limits reported by a concrete driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchGeometryLimits {
    /// Backend name used in diagnostics.
    pub backend: &'static str,
    /// Maximum invocations in one workgroup or block.
    pub max_threads_per_block: u32,
    /// Maximum workgroup or block dimensions (x, y, z).
    pub max_block_dim: [u32; 3],
    /// Maximum workgroup count per grid dimension.
    pub max_grid_dim: [u32; 3],
}

/// Validate workgroup and grid dimensions against backend launch limits.
///
/// # Errors
///
/// Returns when dimensions are zero, overflow the invocation product, exceed
/// workgroup limits, or exceed per-axis grid limits.
pub fn validate_launch_geometry(
    workgroup: [u32; 3],
    grid: [u32; 3],
    limits: LaunchGeometryLimits,
) -> Result<(), BackendError> {
    if workgroup.contains(&0) || grid.contains(&0) {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {} workgroup and grid dimensions must all be non-zero.",
                limits.backend
            ),
        });
    }
    let threads = workgroup[0]
        .checked_mul(workgroup[1])
        .and_then(|xy| xy.checked_mul(workgroup[2]))
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: {} workgroup dimensions overflowed u32; reduce workgroup_override.",
                limits.backend
            ),
        })?;
    if threads > limits.max_threads_per_block {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: {} workgroup has {threads} threads but device max is {}.",
                limits.backend, limits.max_threads_per_block
            ),
        });
    }
    for (axis, &dim) in workgroup.iter().enumerate() {
        if dim > limits.max_block_dim[axis] {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: {} workgroup axis {axis} requested {} threads but device max is {}.",
                    limits.backend, dim, limits.max_block_dim[axis]
                ),
            });
        }
    }
    for (axis, &dim) in grid.iter().enumerate() {
        if dim > limits.max_grid_dim[axis] {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: {} grid axis {axis} requested {} workgroups but device max is {}.",
                    limits.backend, dim, limits.max_grid_dim[axis]
                ),
            });
        }
    }
    Ok(())
}

/// Validate a program's effective workgroup shape against a backend's reported limits.
///
/// This is the shared pre-dispatch gate for callers that have a `VyreBackend`
/// trait object but have not entered a concrete driver yet.
///
/// # Errors
///
/// Returns when any workgroup axis is zero, exceeds the backend's per-axis
/// limit, or when total invocations exceed the backend's workgroup limit.
pub fn validate_program_for_backend(
    backend: &dyn VyreBackend,
    program: &Program,
    config: &DispatchConfig,
) -> Result<(), BackendError> {
    let workgroup = config
        .workgroup_override
        .unwrap_or(program.workgroup_size());
    let max_axes = backend.max_workgroup_size();
    if workgroup.contains(&0) {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: backend `{}` cannot dispatch zero-sized workgroup dimensions; set positive workgroup sizes.",
                backend.id()
            ),
        });
    }
    for (axis, &dim) in workgroup.iter().enumerate() {
        if dim > max_axes[axis] {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: backend `{}` workgroup axis {axis} requested {} but max is {}.",
                    backend.id(),
                    dim,
                    max_axes[axis]
                ),
            });
        }
    }
    let invocations = workgroup[0]
        .checked_mul(workgroup[1])
        .and_then(|xy| xy.checked_mul(workgroup[2]))
        .ok_or_else(|| BackendError::InvalidProgram {
            fix: format!(
                "Fix: backend `{}` workgroup dimensions overflowed u32; reduce workgroup size.",
                backend.id()
            ),
        })?;
    let max_invocations = backend.max_compute_invocations_per_workgroup();
    if invocations > max_invocations {
        return Err(BackendError::InvalidProgram {
            fix: format!(
                "Fix: backend `{}` workgroup has {invocations} invocations but max is {max_invocations}.",
                backend.id()
            ),
        });
    }
    if let Some(grid) = config.grid_override {
        let max_workgroups = backend.max_compute_workgroups_per_dimension();
        if grid.contains(&0) {
            return Err(BackendError::InvalidProgram {
                fix: format!(
                    "Fix: backend `{}` cannot dispatch zero-sized grid dimensions; set positive grid_override values.",
                    backend.id()
                ),
            });
        }
        for (axis, &dim) in grid.iter().enumerate() {
            if dim > max_workgroups {
                return Err(BackendError::InvalidProgram {
                    fix: format!(
                        "Fix: backend `{}` grid_override axis {axis} requested {} workgroups but max is {}.",
                        backend.id(),
                        dim,
                        max_workgroups
                    ),
                });
            }
        }
    }
    Ok(())
}

fn vsa_words_hash(words: &[u32]) -> blake3::Hash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(words.len() as u64).to_le_bytes());
    for word in words {
        hasher.update(&word.to_le_bytes());
    }
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_cache_records_vsa_without_lock_shards() {
        let cache = ValidationCache::new(8, 8, 4);
        let hash = blake3::hash(b"program");
        cache
            .remember_success(hash, &[1, 2, 3, 4])
            .expect("Fix: lock-free VSA cache insertion must not fail");

        assert!(cache.contains_hash(&hash));
        assert_eq!(cache.vsa_hashes.len(), 1);
        assert!(format!("{cache:?}").contains("vsa_hashes"));
    }

    #[test]
    fn validation_cache_bounds_vsa_hashes_by_clear() {
        let cache = ValidationCache::new(8, 2, 4);
        for i in 0..3u32 {
            cache
                .remember_success(blake3::hash(&i.to_le_bytes()), &[i])
                .expect("Fix: VSA cache insertion must stay infallible");
        }
        assert!(
            cache.vsa_hashes.len() <= 2,
            "Fix: bounded VSA cache must not grow past max entries"
        );
    }
}
