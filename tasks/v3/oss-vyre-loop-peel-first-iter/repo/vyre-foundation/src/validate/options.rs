use crate::dialect_lookup::DialectLookup;
use crate::ir::DataType;

/// Backend-specific validation hooks for capability-sensitive rules.
///
/// Foundation validation is backend-agnostic by default. Callers that know the
/// concrete lowering target can provide a capability implementation here so the
/// validator rejects IR shapes that would only fail later in a backend.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "backend capability snapshots are explicit feature bits; replacing them with enums would obscure capability checks and break the stable validation ABI"
)]
pub struct BackendCapabilities {
    /// The backend can lower `Expr::SubgroupAdd`, `Expr::SubgroupBallot`, and
    /// `Expr::SubgroupShuffle`.
    pub supports_subgroup_ops: bool,
    /// The backend can lower indirect dispatch paths.
    pub supports_indirect_dispatch: bool,
    /// The backend can compile specialization constants.
    pub supports_specialization_constants: bool,
    /// The backend can lower distributed collective communication nodes.
    pub supports_distributed_collectives: bool,
    /// Backend has native unsigned multiply-high.
    pub has_mul_high: bool,
    /// INT32 and FP32 pipelines can execute simultaneously.
    pub has_dual_issue_fp32_int32: bool,
    /// Backend supports tensor-core integer matrix multiply.
    pub has_tensor_core_int: bool,
    /// Backend supports native f16 arithmetic at useful throughput.
    pub has_native_f16: bool,
    /// Backend supports warp-level shuffle primitives.
    pub has_warp_shuffle: bool,
    /// Backend supports shared memory with explicit barriers.
    pub has_shared_memory: bool,
    /// Backend can emit bounded polynomial approximations for selected transcendentals.
    pub has_transcendental_polynomial_emit: bool,
    /// Maximum supported integer width for native operations.
    pub max_native_int_width: u32,
}

/// Capability view supplied by a concrete backend during validation.
pub trait BackendValidationCapabilities {
    /// Stable backend name used in diagnostics.
    fn backend_name(&self) -> &'static str;

    /// Return true when the backend can lower a cast whose destination is
    /// `target`.
    fn supports_cast_target(&self, target: &DataType) -> bool;

    /// Return true when the backend supports subgroup operations.
    #[inline]
    fn supports_subgroup_ops(&self) -> bool {
        false
    }

    /// Return true when the backend supports indirect dispatch.
    #[inline]
    fn supports_indirect_dispatch(&self) -> bool {
        false
    }

    /// Return true when the backend supports specialization constants.
    #[inline]
    fn supports_specialization_constants(&self) -> bool {
        false
    }

    /// Return true when the backend supports distributed collective nodes.
    #[inline]
    fn supports_distributed_collectives(&self) -> bool {
        false
    }

    /// Export backend capabilities in a version-stable value object.
    #[must_use]
    #[inline]
    fn backend_capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            supports_subgroup_ops: self.supports_subgroup_ops(),
            supports_indirect_dispatch: self.supports_indirect_dispatch(),
            supports_specialization_constants: self.supports_specialization_constants(),
            supports_distributed_collectives: self.supports_distributed_collectives(),
            ..BackendCapabilities::default()
        }
    }
}

/// Configuration for one validation pass.
///
/// `ValidationOptions::default()` is a best-effort universal pass: it enforces
/// backend-independent invariants only. Provide `backend` when the caller knows
/// the concrete lowering target and wants capability-sensitive rejection.
#[derive(Clone, Copy, Default)]
pub struct ValidationOptions<'a> {
    /// Concrete backend capability surface to validate against.
    pub backend: Option<&'a dyn BackendValidationCapabilities>,
    /// Snapshot of backend capabilities for direct feature checks.
    pub backend_capabilities: Option<BackendCapabilities>,
    /// Optional dialect lookup used to resolve `Expr::Call` signatures.
    pub dialect_lookup: Option<&'a dyn DialectLookup>,
    /// Allow nested-scope shadowing explicitly for this validation run.
    pub allow_shadowing: bool,
}

impl<'a> ValidationOptions<'a> {
    /// Build the default best-effort universal validator configuration.
    #[must_use]
    #[inline]
    pub fn universal() -> Self {
        Self::default()
    }

    /// Validate against the provided backend capability contract.
    #[must_use]
    #[inline]
    pub fn with_backend(mut self, backend: &'a dyn BackendValidationCapabilities) -> Self {
        self.backend = Some(backend);
        self.backend_capabilities = Some(backend.backend_capabilities());
        self
    }

    /// Validate against the provided backend capability snapshot.
    #[must_use]
    #[inline]
    pub fn with_backend_capabilities(mut self, backend_capabilities: BackendCapabilities) -> Self {
        self.backend_capabilities = Some(backend_capabilities);
        self
    }

    /// Validate operation calls against an explicit dialect lookup.
    #[must_use]
    #[inline]
    pub fn with_dialect_lookup(mut self, lookup: &'a dyn DialectLookup) -> Self {
        self.dialect_lookup = Some(lookup);
        self
    }

    /// Explicitly allow nested-scope shadowing for this validation pass.
    #[must_use]
    #[inline]
    pub fn with_shadowing(mut self, allow_shadowing: bool) -> Self {
        self.allow_shadowing = allow_shadowing;
        self
    }

    /// Return the backend name carried by this configuration.
    #[must_use]
    #[inline]
    pub fn backend_name(&self) -> &'static str {
        self.backend.map_or(
            "best-effort universal",
            BackendValidationCapabilities::backend_name,
        )
    }

    /// Return true when this validation run accepts casts to `target`.
    #[must_use]
    #[inline]
    pub fn supports_cast_target(&self, target: &DataType) -> bool {
        self.backend
            .is_none_or(|backend| backend.supports_cast_target(target))
    }

    /// Return true when this validation run requires subgroup support.
    #[must_use]
    #[inline]
    pub fn requires_subgroup_ops(&self) -> bool {
        self.backend_capabilities
            .is_some_and(|caps| caps.supports_subgroup_ops)
    }

    /// Return true when this validation run accepts distributed collectives.
    #[must_use]
    #[inline]
    pub fn supports_distributed_collectives(&self) -> bool {
        self.backend_capabilities
            .is_some_and(|caps| caps.supports_distributed_collectives)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CapabilityFixtureBackend;
    impl BackendValidationCapabilities for CapabilityFixtureBackend {
        fn backend_name(&self) -> &'static str {
            "capability-fixture-gpu"
        }
        fn supports_cast_target(&self, target: &DataType) -> bool {
            matches!(target, DataType::U32 | DataType::F32)
        }
        fn supports_subgroup_ops(&self) -> bool {
            true
        }
    }

    #[test]
    fn universal_defaults() {
        let opts = ValidationOptions::universal();
        assert!(opts.backend.is_none());
        assert!(!opts.allow_shadowing);
        assert_eq!(opts.backend_name(), "best-effort universal");
    }

    #[test]
    fn with_backend_sets_name_and_caps() {
        let backend = CapabilityFixtureBackend;
        let opts = ValidationOptions::universal().with_backend(&backend);
        assert_eq!(opts.backend_name(), "capability-fixture-gpu");
        assert!(opts.requires_subgroup_ops());
    }

    #[test]
    fn supports_cast_target_delegates_to_backend() {
        let backend = CapabilityFixtureBackend;
        let opts = ValidationOptions::universal().with_backend(&backend);
        assert!(opts.supports_cast_target(&DataType::U32));
        assert!(!opts.supports_cast_target(&DataType::Bool));
    }

    #[test]
    fn supports_cast_target_defaults_true_without_backend() {
        let opts = ValidationOptions::universal();
        assert!(opts.supports_cast_target(&DataType::Bool));
    }

    #[test]
    fn with_shadowing_toggle() {
        let opts = ValidationOptions::universal().with_shadowing(true);
        assert!(opts.allow_shadowing);
    }

    #[test]
    fn backend_capabilities_default() {
        let caps = BackendCapabilities::default();
        assert!(!caps.supports_subgroup_ops);
        assert!(!caps.supports_indirect_dispatch);
        assert!(!caps.supports_specialization_constants);
        assert!(!caps.supports_distributed_collectives);
    }

    #[test]
    fn with_backend_capabilities_snapshot() {
        let caps = BackendCapabilities {
            supports_subgroup_ops: true,
            supports_indirect_dispatch: false,
            supports_specialization_constants: false,
            ..BackendCapabilities::default()
        };
        let opts = ValidationOptions::universal().with_backend_capabilities(caps);
        assert!(opts.requires_subgroup_ops());
    }
}
