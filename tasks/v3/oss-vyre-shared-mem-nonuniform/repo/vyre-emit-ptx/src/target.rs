/// Target compute capability for PTX emit. Defaults to `sm_70` (Volta),
/// the broad-compatibility floor for the shipped PTX op set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ComputeCapability {
    pub major: u32,
    pub minor: u32,
}

impl ComputeCapability {
    pub const SM_70: Self = Self { major: 7, minor: 0 };
    pub const SM_75: Self = Self { major: 7, minor: 5 };
    pub const SM_80: Self = Self { major: 8, minor: 0 };
    pub const SM_86: Self = Self { major: 8, minor: 6 };
    pub const SM_89: Self = Self { major: 8, minor: 9 };
    pub const SM_90: Self = Self { major: 9, minor: 0 };

    #[must_use]
    pub const fn supports_async_copy(&self) -> bool {
        self.major >= 8
    }

    /// True when the target supports `ldmatrix` shared-memory matrix loads.
    ///
    /// Turing introduced `ldmatrix`; Ampere and later combine it with
    /// `cp.async` for global-to-shared staging plus shared-to-fragment loads.
    #[must_use]
    pub const fn supports_ldmatrix(&self) -> bool {
        self.major > 7 || (self.major == 7 && self.minor >= 5)
    }

    #[must_use]
    pub const fn supports_wmma_f16(&self) -> bool {
        self.major >= 7
    }

    #[must_use]
    pub const fn supports_wmma_bf16(&self) -> bool {
        self.major >= 8
    }
}

impl Default for ComputeCapability {
    fn default() -> Self {
        Self::SM_70
    }
}

/// CUDA PTX emission knobs that affect instruction selection but not
/// descriptor semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtxEmitOptions {
    pub target: ComputeCapability,
    pub subgroup_size: u32,
    pub ulp_budget: Option<u32>,
    /// Lower `MemoryOrdering::GridSync` barriers to a native cooperative grid
    /// barrier (a monotonic-counter spin on a module-scope counter) instead of
    /// rejecting them. The emitted barrier is correct ONLY under a cooperative
    /// launch (every CTA co-resident); a non-cooperative launch deadlocks. The
    /// consumer MUST guarantee a cooperative launch before enabling this — the
    /// CUDA backend does, by forcing `cuLaunchCooperativeKernel` for any
    /// grid-sync program and zeroing the counter per launch. Default `false`:
    /// the emitter rejects GridSync (fail-safe) so a backend that cannot
    /// guarantee cooperative residency never silently emits a deadlock-prone or
    /// CTA-scope-downgraded barrier.
    pub cooperative_grid_sync: bool,
}

impl PtxEmitOptions {
    pub fn for_target(target: ComputeCapability) -> Self {
        Self {
            target,
            subgroup_size: 32,
            ulp_budget: None,
            cooperative_grid_sync: false,
        }
    }
}

impl Default for PtxEmitOptions {
    fn default() -> Self {
        Self::for_target(ComputeCapability::default())
    }
}
