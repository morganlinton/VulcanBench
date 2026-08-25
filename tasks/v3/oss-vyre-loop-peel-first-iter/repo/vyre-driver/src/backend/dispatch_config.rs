//! Immutable dispatch policy supplied by callers before backend execution.

use std::time::Duration;

/// Immutable execution policy supplied by the caller before dispatch.
///
/// `DispatchConfig` is an additive, non-exhaustive struct so that new backend
/// options (conformance profiles, adapter hints, etc.) can be added without
/// breaking the frozen `VyreBackend::dispatch` signature. Backends must treat
/// every field as read-only policy and must not assume the presence of any
/// particular option.
///
/// # Examples
///
/// ```
/// use vyre::DispatchConfig;
///
/// // DispatchConfig is `#[non_exhaustive]`; construct it through
/// // `default()` and overwrite the fields you want to change.
/// let mut config = DispatchConfig::default();
/// config.profile = Some("stress".to_string());
/// config.ulp_budget = None;
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct DispatchConfig {
    /// Optional stable profile identifier such as `default`, `stress`, or a
    /// backend-defined conformance mode.
    pub profile: Option<String>,
    /// Optional maximum ULP error budget for approximate transcendental lowering.
    ///
    /// `None` and `Some(0)` require the strict target-text intrinsic path. A positive
    /// budget allows backends to select fast approximate intrinsic wrappers only
    /// when the wrapper contract is bounded by the supplied ULP ceiling.
    pub ulp_budget: Option<u8>,
    /// Optional timeout for the dispatch.
    pub timeout: Option<Duration>,
    /// Optional label for the dispatch (for debugging/profiling).
    pub label: Option<String>,
    /// Optional maximum output byte limit.
    pub max_output_bytes: Option<usize>,
    /// Optional workgroup size override.
    ///
    /// When `Some`, the backend uses the supplied `[x, y, z]` workgroup size
    /// instead of the one declared on the [`vyre_foundation::ir::Program`].
    /// This lets callers tune workgroup sizing at dispatch time without
    /// cloning the program metadata. When `None` (the default), the backend
    /// falls back to `Program::workgroup_size`.
    pub workgroup_override: Option<[u32; 3]>,
    /// Optional grid size override (number of workgroups).
    ///
    /// When set, the backend launches the supplied workgroup count instead of
    /// the one inferred from the program's output buffer size. This is
    /// required for megakernels where the work queue length is managed through
    /// storage buffers rather than the primary output slot.
    pub grid_override: Option<[u32; 3]>,
    /// Maximum back-to-back dispatch iterations the backend should run on
    /// the same persistent input/output handles before reading back the
    /// final outputs.
    ///
    /// `None` means one iteration. `Some(0)` is invalid: backends must reject
    /// it instead of silently rewriting caller policy.
    pub fixpoint_iterations: Option<u32>,
    /// Optional speculation policy.
    pub speculation: Option<crate::speculate::SpeculationMode>,
    /// Optional persistent-thread dispatch policy.
    pub persistent_thread: Option<crate::persistent::PersistentThreadMode>,
    /// Whether the backend should launch through its cooperative-grid API.
    ///
    /// A backend MUST reject `cooperative = true` with `UnsupportedFeature`
    /// when its `VyreBackend::supports_grid_sync()` returns `false`.
    pub cooperative: bool,
}

impl DispatchConfig {
    /// Construct a `DispatchConfig` from explicit fields in one call.
    /// Complement to `DispatchConfig::default()` for external crates
    /// that want all optional fields set up front.
    #[must_use]
    pub fn new(
        profile: Option<String>,
        ulp_budget: Option<u8>,
        timeout: Option<Duration>,
        label: Option<String>,
    ) -> Self {
        Self {
            profile,
            ulp_budget,
            timeout,
            label,
            max_output_bytes: None,
            workgroup_override: None,
            grid_override: None,
            fixpoint_iterations: None,
            speculation: None,
            persistent_thread: None,
            cooperative: false,
        }
    }
}
