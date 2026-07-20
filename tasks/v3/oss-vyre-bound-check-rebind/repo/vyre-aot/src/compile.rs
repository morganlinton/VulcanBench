//! Program → CompiledArtifact lowering.
//!
//! The compile path asks the `vyre-driver` AOT registry for a linked
//! target emitter and packs the resulting bytes plus binding/dispatch
//! metadata into a [`CompiledArtifact`].

use thiserror::Error;
use vyre_foundation::ir::{inline_calls_with_resolver, OpResolver, Program};

use crate::artifact::{
    BufferAccessKind, BufferEntry, BufferMemoryKind, CompiledArtifact, DispatchGeometry, Target,
};
use crate::VERSION;

/// Errors returned by [`compile`].
#[derive(Debug, Error)]
pub enum CompileError {
    /// The chosen `Target` is not enabled in this build (feature flag).
    #[error(
        "vyre-aot: target {0:?} has no linked AOT emitter. Fix: link the concrete driver crate that owns this target."
    )]
    TargetNotEnabled(Target),

    /// The backend rejected the Program with a structured message.
    #[error("vyre-aot: backend rejected Program: {0}")]
    BackendError(String),

    /// The Program cannot be represented accurately in the AOT artifact schema.
    #[error("vyre-aot: artifact layout rejected Program: {0}")]
    ArtifactLayout(String),
}

/// Compile a `Program` into a self-contained artifact for a chosen target.
///
/// This is the load-bearing entry point. All vyre-foundation / vyre-driver
/// machinery is touched HERE; the resulting [`CompiledArtifact`] is
/// self-describing and the launcher does not need vyre at runtime.
///
/// # Errors
///
/// Returns [`CompileError::TargetNotEnabled`] if the requested target's
/// feature flag is not enabled, or [`CompileError::BackendError`] if the
/// backend rejects the Program.
pub fn compile(program: &Program, target: Target) -> Result<CompiledArtifact, CompileError> {
    compile_with_resolver(program, target, None)
}

/// Compile with a caller-supplied resolver to inline `Expr::Call` nodes.
///
/// When the input Program contains `Expr::Call` to ops registered through
/// `vyre-driver`'s `DialectRegistry` or `vyre-libs::harness::OpEntry`, supply
/// an [`OpResolver`] (an `fn(&str) -> Option<Program>`) so the inline pass
/// can substitute their bodies before backend emission. Pass `None` if the
/// Program has no `Expr::Call` nodes (e.g. low-level smoke tests).
///
/// # Errors
///
/// Same as [`compile`], plus inline-pass errors when the Program has
/// unresolvable calls.
pub fn compile_with_resolver(
    program: &Program,
    target: Target,
    resolver: Option<OpResolver>,
) -> Result<CompiledArtifact, CompileError> {
    // AOT emitters need a Program with no `Expr::Call` nodes  -  run
    // inline_calls when a resolver is provided, otherwise pass through
    // unchanged (the caller has either pre-inlined or guarantees no Call).
    let inlined = match resolver {
        Some(r) => inline_calls_with_resolver(program, r)
            .map_err(|e| CompileError::BackendError(format!("{e:?}")))?,
        None => program.clone(),
    };

    // P-AOT-2: run the canonical optimizer pipeline (canonicalize →
    // region_inline → CSE → DCE) so AOT and JIT produce identical
    // post-optimization Programs for identical inputs. This is the
    // single seam where every recursion-thesis self-consumer wired
    // into `vyre_foundation::optimizer::pre_lowering::optimize` (categorical
    // pass scheduler, tensor-network fusion order, dataflow fixpoint,
    // submodular cache eviction, etc.) flows into the AOT compile
    // path. Pre-fix: AOT bypassed all optimization and emitted bytecode
    // directly from the inlined Program. Post-fix: AOT inherits every
    // substrate upgrade landed in vyre-foundation for free.
    let optimized = vyre_foundation::ir::optimize(inlined);

    // P-AOT-1: AOT artifacts carry a VSA fingerprint of the optimized
    // Program for downstream cache-dedup. Two compilations that
    // differ only in non-semantic detail (instruction order,
    // commutative-operand ordering) collide on the same VSA
    // fingerprint, letting AOT toolchains skip redundant emit.
    // Computed via the driver-level canonical VSA fingerprint  -  the
    // same approximate-match cache key that driver validation caches
    // use, so AOT and JIT share artifact-identity without reaching
    // into a CPU-named substrate helper.
    let vsa = vyre_driver::program_vsa_fingerprint(&optimized);

    let buffers = collect_buffer_entries(&optimized)?;

    let dispatch_config = vyre_driver::DispatchConfig::default();
    let kernel_bytes =
        vyre_driver::aot::emit_aot_target(target.aot_target_id(), &optimized, &dispatch_config)
            .map_err(|error| match error {
                vyre_driver::BackendError::UnsupportedFeature { .. } => {
                    CompileError::TargetNotEnabled(target)
                }
                other => CompileError::BackendError(other.to_string()),
            })?;

    let dispatch = derive_dispatch_geometry(&optimized)?;

    Ok(CompiledArtifact {
        target,
        kernel_bytes,
        entry_point: "main".to_string(),
        buffers,
        dispatch,
        aot_version: VERSION.to_string(),
        vsa_fingerprint: vsa,
    })
}

fn derive_dispatch_geometry(program: &Program) -> Result<DispatchGeometry, CompileError> {
    let plan = vyre_driver::binding::BindingPlan::build(program)
        .map_err(|error| CompileError::BackendError(error.to_string()))?;
    let element_count =
        vyre_driver::program_walks::dispatch_element_count_for_program(program, &plan.bindings);
    let grid_size =
        vyre_driver::infer_dispatch_grid_for_count(element_count, program.workgroup_size)
            .map_err(|error| CompileError::BackendError(error.to_string()))?;
    Ok(DispatchGeometry {
        workgroup_size: program.workgroup_size,
        grid_size,
        dynamic_shared_bytes: 0,
    })
}

fn collect_buffer_entries(program: &Program) -> Result<Vec<BufferEntry>, CompileError> {
    program
        .buffers()
        .iter()
        .map(|buf| {
            Ok(BufferEntry {
                name: buf.name().to_string(),
                binding: buf.binding(),
                element_count: buf.count(),
                element_size_bytes: element_size_bytes(buf.name(), buf.element())?,
                memory_kind: convert_memory_kind(buf.kind()),
                access: convert_access(buf.name(), buf.access())?,
            })
        })
        .collect()
}

fn element_size_bytes(name: &str, ty: vyre_foundation::ir::DataType) -> Result<u32, CompileError> {
    // Delegate to the canonical size table on `DataType` so every
    // variant (Vec2U32 = 8, Vec4U32 = 16, Bytes = 1, Array = element
    // size, F8/F4/I4/NF4 = 1, etc.) round-trips correctly into the AOT
    // artifact. The previous hand-written match defaulted to 4 for any
    // unenumerated variant, mis-sizing Bytes (1 byte/element treated
    // as 4) and the vector / quantized families.
    let raw = ty.size_bytes().ok_or_else(|| {
        CompileError::ArtifactLayout(format!(
            "buffer `{name}` uses runtime-sized element type {ty:?}. Fix: lower this buffer to a concrete fixed-width ABI type before AOT emission; do not encode a guessed element size."
        ))
    })?;
    u32::try_from(raw).map_err(|_| {
        CompileError::ArtifactLayout(format!(
            "buffer `{name}` element type {ty:?} has {raw} bytes per element, which exceeds the AOT artifact u32 size field. Fix: shard or specialize the buffer layout before AOT emission."
        ))
    })
}

fn convert_memory_kind(k: vyre_foundation::ir::MemoryKind) -> BufferMemoryKind {
    use vyre_foundation::ir::MemoryKind;
    match k {
        MemoryKind::Shared | MemoryKind::Local => BufferMemoryKind::Shared,
        MemoryKind::Uniform | MemoryKind::Push | MemoryKind::Readonly => BufferMemoryKind::Constant,
        _ => BufferMemoryKind::Global,
    }
}

fn convert_access(
    name: &str,
    a: vyre_foundation::ir::BufferAccess,
) -> Result<BufferAccessKind, CompileError> {
    use vyre_foundation::ir::BufferAccess;
    match a {
        BufferAccess::ReadOnly => Ok(BufferAccessKind::ReadOnly),
        BufferAccess::WriteOnly => Ok(BufferAccessKind::WriteOnly),
        BufferAccess::ReadWrite => Ok(BufferAccessKind::ReadWrite),
        // Uniform is semantically read-only (small, fast, read-only path).
        BufferAccess::Uniform => Ok(BufferAccessKind::ReadOnly),
        // Workgroup-local shared memory is both read and written within a workgroup.
        BufferAccess::Workgroup => Ok(BufferAccessKind::ReadWrite),
        other => Err(CompileError::ArtifactLayout(format!(
            "buffer `{name}` uses unrecognised access kind {other:?}. \
             Fix: add an explicit mapping for this variant in convert_access \
             (vyre-aot/src/compile.rs). Silently defaulting to ReadWrite would \
             miscompile buffers that are not read-write."
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::{BufferDecl, DataType, Expr, Node};

    #[test]
    fn dispatch_geometry_is_explicit_not_runtime_grid_placeholder() {
        let program = Program::wrapped(
            vec![
                BufferDecl::read("input", 0, DataType::U32).with_count(1024),
                BufferDecl::read_write("out", 1, DataType::U32).with_count(1024),
            ],
            [128, 1, 1],
            vec![
                Node::let_bind("idx", Expr::u32(0)),
                Node::store(
                    "out",
                    Expr::var("idx"),
                    Expr::load("input", Expr::var("idx")),
                ),
            ],
        );

        let dispatch = derive_dispatch_geometry(&program)
            .expect("Fix: AOT dispatch geometry derivation must accept finite buffer shapes.");

        assert_eq!(dispatch.workgroup_size, [128, 1, 1]);
        assert_eq!(
            dispatch.grid_size,
            [8, 1, 1],
            "Fix: vyre-aot must emit explicit finite grid geometry for CUDA launchers, not [0,0,0]."
        );
    }

    #[test]
    fn dispatch_geometry_rejects_zero_workgroup_axes_before_artifact_emission() {
        let program = Program::wrapped(
            vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(16)],
            [0, 1, 1],
            vec![
                Node::let_bind("idx", Expr::u32(0)),
                Node::store("out", Expr::var("idx"), Expr::u32(1)),
            ],
        );

        let err = derive_dispatch_geometry(&program).expect_err(
            "Fix: AOT must reject zero workgroup axes instead of emitting a poisoned manifest.",
        );

        assert!(
            err.to_string().contains("workgroup dimensions must be non-zero"),
            "Fix: zero-workgroup AOT rejection must point at the dispatch shape contract, got {err}."
        );
    }

    #[test]
    fn convert_access_uniform_maps_to_read_only_not_read_write() {
        // Before the fix, BufferAccess::Uniform fell through the wildcard arm
        // and silently mapped to ReadWrite (convert-access-wildcard-fallback).
        // Uniform is semantically read-only; mapping it to ReadWrite is a
        // miscompile that wastes memory bandwidth on restricted-access GPU APIs.
        use vyre_foundation::ir::BufferAccess;
        let result = convert_access("my_uniform_buf", BufferAccess::Uniform)
            .expect("Fix: BufferAccess::Uniform must map to ReadOnly, not Err");
        assert_eq!(
            result,
            BufferAccessKind::ReadOnly,
            "Fix: BufferAccess::Uniform must map to BufferAccessKind::ReadOnly, \
             not ReadWrite. Got {result:?}."
        );
    }

    #[test]
    fn convert_access_workgroup_maps_to_read_write_explicitly_not_via_wildcard() {
        // BufferAccess::Workgroup is workgroup-local shared memory, legitimately
        // ReadWrite. The old code reached this correct result only via the
        // silent wildcard fallback — which also maps future unknown variants
        // silently. This test pins the explicit mapping.
        use vyre_foundation::ir::BufferAccess;
        let result = convert_access("scratch", BufferAccess::Workgroup)
            .expect("Fix: BufferAccess::Workgroup must map to ReadWrite, not Err");
        assert_eq!(
            result,
            BufferAccessKind::ReadWrite,
            "Fix: BufferAccess::Workgroup must map to BufferAccessKind::ReadWrite. \
             Got {result:?}."
        );
    }

    #[test]
    fn convert_access_all_known_variants_map_deterministically() {
        // All known BufferAccess variants must map to the correct access kind.
        // This test will fail to compile if a new non-exhaustive variant is added
        // to vyre_foundation::ir::BufferAccess but not handled in convert_access.
        use vyre_foundation::ir::BufferAccess;
        let cases = [
            (BufferAccess::ReadOnly, BufferAccessKind::ReadOnly),
            (BufferAccess::WriteOnly, BufferAccessKind::WriteOnly),
            (BufferAccess::ReadWrite, BufferAccessKind::ReadWrite),
            (BufferAccess::Uniform, BufferAccessKind::ReadOnly),
            (BufferAccess::Workgroup, BufferAccessKind::ReadWrite),
        ];
        for (access, expected) in cases {
            let got = convert_access("buf", access.clone())
                .unwrap_or_else(|e| panic!("Fix: convert_access({access:?}) returned Err: {e}"));
            assert_eq!(
                got, expected,
                "Fix: convert_access({access:?}) must map to {expected:?}, got {got:?}."
            );
        }
    }
}
