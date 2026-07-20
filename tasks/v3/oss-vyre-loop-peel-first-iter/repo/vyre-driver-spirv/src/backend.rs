//! SPIR-V emission via naga::back::spv.

use naga::back::spv;
use vyre_foundation::ir::Program;

/// Emit SPIR-V words from a vyre-built naga::Module.
///
/// The caller builds the `naga::Module` through the same builder family
/// that the portable emission path uses (so the kernel body is byte-identical across
/// substrates up to the back-end writer); this function validates and
/// writes the SPIR-V blob.
pub struct SpirvBackend;

impl SpirvBackend {
    /// Stable backend identifier.
    pub const BACKEND_ID: &'static str = super::SPIRV_BACKEND_ID;

    /// Construct a new backend instance. Always succeeds.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Emit SPIR-V words from a validated naga::Module.
    ///
    /// # Errors
    /// Returns a human diagnostic when the module fails naga validation or
    /// when the SPIR-V writer rejects a construct.
    pub fn emit_spv(module: &naga::Module) -> Result<Vec<u32>, String> {
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(module)
        .map_err(|e| format!("naga validate failed: {e:?}"))?;
        let options = spv::Options::default();
        spv::write_vec(module, &info, &options, None)
            .map_err(|e| format!("spv write failed: {e:?}"))
    }

    /// Lower a vyre [`Program`] to SPIR-V words.
    ///
    /// The path is `Program → KernelDescriptor → naga::Module → SPIR-V`.
    /// This is the entry point used by the runtime dispatch path.
    ///
    /// # Errors
    /// Returns a human diagnostic when lowering, validation, or SPIR-V
    /// writing fails.
    pub fn program_to_spv(program: &Program) -> Result<Vec<u32>, String> {
        let desc =
            vyre_lower::lower::lower(program).map_err(|e| format!("vyre lower failed: {e:?}"))?;
        let module = vyre_emit_naga::emit(&desc).map_err(|e| format!("naga emit failed: {e:?}"))?;
        Self::emit_spv(&module)
    }

    /// Compute the substrate VSA fingerprint of a vyre Program. Same
    /// fingerprint vyre-aot persists on `CompiledArtifact` and
    /// runtime validation caches use for their identity key; sharing the
    /// fingerprint across backends lets a single SPIR-V or PTX cache
    /// dedup against AOT artifacts.
    ///
    /// P-SPIRV-1: substrate consumption  -  vsa_fingerprint is the
    /// identity-by-meaning key that crosses backend boundaries.
    #[must_use]
    pub fn program_fingerprint(program: &Program) -> Vec<u32> {
        vyre_driver::program_vsa_fingerprint(program)
    }

    /// Snapshot the driver-tier observability surface
    /// ([`vyre_driver::observability::DriverObservability`]).
    #[must_use]
    pub fn observability_snapshot() -> vyre_driver::observability::DriverObservability {
        vyre_driver::observability::DriverObservability::snapshot()
    }

    /// SPIR-V module disk-cache directory. Same on-disk-key family as
    /// native-module and validation caches, keyed by VSA fingerprint
    /// via [`Self::program_fingerprint`].
    ///
    /// P-SPIRV-2: SPIR-V module blobs persist across runs.
    #[must_use]
    pub fn spv_disk_cache_dir() -> std::path::PathBuf {
        std::env::var_os("VYRE_SPV_CACHE_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::temp_dir().join("vyre-spv-cache"))
    }
}

impl Default for SpirvBackend {
    fn default() -> Self {
        Self::new()
    }
}
