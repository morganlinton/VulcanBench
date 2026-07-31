//! Compatibility entry points for callers that still hand this crate a
//! high-level `Program`.
//!
//! These functions immediately route through `vyre-lower::lower_for_emit` and
//! the descriptor emitter. They do not maintain a second Program-to-Naga
//! lowering path.

use std::sync::Arc;

use naga::Module;
use rustc_hash::FxHashSet;

use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Ident, MemoryKind, Program};
use vyre_foundation::visit::visit_node_preorder;

use super::atomic_scanner::scan_atomic_targets_into;
use super::trap_collector::TrapTagCollector;
use super::types::{TrapTag, TRAP_SIDECAR_NAME, TRAP_SIDECAR_WORDS};
use super::{bind_group_for, LoweringError, ProgramEmitFeatures};

/// Emit a validated Naga module for a vyre program.
///
/// # Errors
///
/// Returns [`LoweringError`] when the IR references unsupported types,
/// buffers, statements, or expressions, or when Naga validation rejects the
/// emitted module.
pub fn emit_module(
    program: &Program,
    config: &vyre_driver::DispatchConfig,
    workgroup_size: [u32; 3],
) -> Result<Module, LoweringError> {
    emit_module_with_features(
        program,
        config,
        workgroup_size,
        ProgramEmitFeatures::default(),
    )
}

/// Emit a Naga module using the exact feature contract supplied by the runtime.
///
/// feature-sensitive IR such as `MemoryOrdering::SeqCst` barriers must be
/// lowered against the real device contract instead of a permissive default.
pub fn emit_module_with_features(
    program: &Program,
    _config: &vyre_driver::DispatchConfig,
    workgroup_size: [u32; 3],
    _features: ProgramEmitFeatures,
) -> Result<Module, LoweringError> {
    // Fail closed on the ONE IR-validity hazard that otherwise BOTH silently
    // miscompiles AND emits successfully: an `Fma` node with non-f32 operands
    // lowers to integer `a*b+c`, not fused-multiply-add (a Law-10 silent
    // miscompile). `lower_for_emit` runs dead-code elimination, so an unused
    // such node is stripped before any descriptor-level check could see it —
    // the original Program node tree is the only stage that observes it. We
    // deliberately do NOT run full `vyre_foundation::validate` here: every
    // other validation rule corresponds to a program that either emits
    // correctly or fails with a dedicated, more-specific downstream diagnostic
    // (e.g. "Vec4U32 not representable", "rejected at lowering boundary"), so
    // running the full validator would preempt those precise messages. The Fma
    // f32 check reuses the foundation validator's scope/type inference, so the
    // emit boundary and `validate` agree by construction.
    let fma_violations = vyre_foundation::validate::fma_f32_violations(program);
    if !fma_violations.is_empty() {
        return Err(LoweringError::invalid(format!(
            "vyre IR program rejected before Naga emission: {}. Fix: correct the reported Fma operand types before emitting; emit_module does not silently lower an integer Fma to `a*b+c`.",
            format_validation_errors(&fma_violations)
        )));
    }
    // Reject unresolved async/resume nodes at this Program-compatibility entry.
    // It does not run the async-resolution pass, and lowering them silently (or
    // treating `Resume` as a no-op) would drop their semantics — see
    // `async_resume_guard` for why the descriptor emitter still lowers them.
    if let Err(kind) = super::async_resume_guard::reject_async_resume(program) {
        return Err(LoweringError::invalid(format!(
            "vyre IR `{kind}` node reached the Program-compatibility Naga emit entry, which does not run the async/trap resolution pass that gives it meaning. Fix: resolve {kind} nodes before emit_module (run the async lowering / resolution pass), or build a KernelDescriptor and call the descriptor emitter directly."
        )));
    }
    // Workgroup (shared) buffers lower to a fixed-size `var<workgroup>` array,
    // so they need a positive static element count. A zero-count one is pruned
    // when unused (silently vanishing) or would emit a zero-length array — both
    // wrong — so reject it at the boundary before lowering can drop it.
    for buffer in program.buffers() {
        if buffer.access == BufferAccess::Workgroup && buffer.count == 0 {
            return Err(LoweringError::invalid(format!(
                "workgroup buffer `{}` has zero static element count; shared memory needs a positive fixed size. Fix: declare a positive element count on the workgroup buffer.",
                buffer.name
            )));
        }
    }
    let mut lowered = vyre_lower::lower_for_emit(program).map_err(|error| {
        LoweringError::invalid(format!(
            "canonical pre-emit lowering failed before Naga Program compatibility emission: {error}. Fix: route callers through vyre-lower::lower_for_emit and descriptor emit instead of direct Program emission."
        ))
    })?;
    lowered.descriptor.dispatch.workgroup_size = workgroup_size;
    if let Err(errors) = vyre_lower::verify::verify(&lowered.descriptor) {
        return Err(LoweringError::invalid(format!(
            "KernelDescriptor verification failed after Naga Program compatibility workgroup override: {}. Fix: keep the requested workgroup size valid before emission.",
            format_verify_errors(&errors)
        )));
    }
    crate::emit(&lowered.descriptor).map_err(|error| {
        LoweringError::invalid(format!(
            "descriptor Naga emission failed from Program compatibility entry point: {error}. Fix: extend vyre-emit-naga descriptor emission; direct Program lowering is not a fallback path."
        ))
    })
}

pub fn emit_prepared_module_with_features(
    program: &Program,
    config: &vyre_driver::DispatchConfig,
    workgroup_size: [u32; 3],
    features: ProgramEmitFeatures,
) -> Result<Module, LoweringError> {
    emit_module_with_features(program, config, workgroup_size, features)
}

/// Inline, optimize, and infer buffer access modes before Naga lowering.
///
/// # Errors
///
/// Returns a lowering error when call inlining fails or the rewritten program
/// cannot preserve the backend's buffer-access invariants.
pub fn prepared_program(program: &Program) -> Result<Program, LoweringError> {
    let lowered = vyre_lower::lower_for_emit(program).map_err(|error| {
        LoweringError::invalid(format!(
            "canonical pre-emit lowering failed before Naga Program compatibility preparation: {error}. Fix: route Program compatibility helpers through vyre-lower::lower_for_emit instead of local inlining or optimizer passes."
        ))
    })?;
    let program = lowered.program;
    // BufferAccess auto-inference. Walk the entry nodes and collect
    // the set of buffers that receive a write (Node::Store /
    // AsyncStore / AsyncLoad / IndirectDispatch / Expr::Atomic*). Any
    // ReadWrite buffer NOT in that set is auto-downgraded to
    // ReadOnly. The result flows to BOTH the naga emitter (which
    // emits the WGSL `var<storage, read>` access mode) AND the
    // pipeline-layout descriptor (which sets `read_only=true`)  -  they
    // agree by construction. Pre-fix: the consumer's merge step defaulted
    // every intermediate buffer to ReadWrite for safety; pipeline
    // layout was built from BufferDecl.access (ReadWrite →
    // read_only=false) but the shader emitter saw only loads.
    // Naga validation rejected the mismatch.
    let mut atomic_targets = FxHashSet::<Ident>::default();
    let mut write_targets = FxHashSet::<Ident>::default();
    for node in program.entry() {
        scan_atomic_targets_into(node, &mut atomic_targets, &mut write_targets)?;
    }
    let new_buffers: Vec<BufferDecl> = program
        .buffers()
        .iter()
        .map(|buffer| {
            let buffer_name = Ident::from(buffer.name());
            if matches!(buffer.access, vyre_foundation::ir::BufferAccess::ReadWrite)
                && !write_targets.contains(&buffer_name)
                && !atomic_targets.contains(&buffer_name)
            {
                let mut downgraded = buffer.clone();
                downgraded.access = vyre_foundation::ir::BufferAccess::ReadOnly;
                downgraded
            } else {
                buffer.clone()
            }
        })
        .collect();
    let workgroup_size = program.workgroup_size;
    let entry = program.into_entry_vec();
    Ok(Program::wrapped(new_buffers, workgroup_size, entry))
}

pub fn trap_tags(program: &Program) -> Result<Arc<[TrapTag]>, LoweringError> {
    let program = prepared_program(program)?;
    Ok(trap_tags_for_prepared_program(&program).into())
}

pub fn trap_sidecar_decl(program: &Program) -> Result<BufferDecl, LoweringError> {
    Ok(BufferDecl::storage(
        TRAP_SIDECAR_NAME,
        trap_sidecar_binding(program)?,
        BufferAccess::ReadWrite,
        DataType::U32,
    )
    .with_count(TRAP_SIDECAR_WORDS))
}

fn trap_tags_for_prepared_program(program: &Program) -> Vec<TrapTag> {
    let mut collector = TrapTagCollector::default();
    for node in program.entry() {
        debug_assert!(
            visit_node_preorder(&mut collector, node).is_continue(),
            "trap tag collection must not short-circuit"
        );
    }
    collector.into_tags()
}

fn trap_sidecar_binding(program: &Program) -> Result<u32, LoweringError> {
    let trap_group = bind_group_for(MemoryKind::Global);
    let mut next = 0u32;
    for buffer in program.buffers() {
        if bind_group_for(buffer.kind()) == trap_group {
            next = next.max(buffer.binding().checked_add(1).ok_or_else(|| {
                LoweringError::invalid(
                    "program uses u32::MAX as a Naga binding in the trap sidecar bind group. Fix: leave one free binding for backend-owned trap propagation.",
                )
            })?);
        }
    }
    Ok(next)
}

fn format_verify_errors(errors: &[vyre_lower::verify::VerifyError]) -> String {
    let mut out = String::new();
    for (index, error) in errors.iter().take(4).enumerate() {
        if index != 0 {
            out.push_str("; ");
        }
        out.push_str(&format!("{error:?}"));
    }
    if errors.len() > 4 {
        out.push_str("; ...");
    }
    out
}

/// Render program-validation errors using their `Display` form (the
/// human-actionable `Fix:` message), not `Debug`, so the surfaced
/// `LoweringError` preserves each rule's remediation text verbatim.
fn format_validation_errors(errors: &[vyre_foundation::validate::ValidationError]) -> String {
    let mut out = String::new();
    for (index, error) in errors.iter().take(4).enumerate() {
        if index != 0 {
            out.push_str("; ");
        }
        out.push_str(error.message());
    }
    if errors.len() > 4 {
        out.push_str("; ...");
    }
    out
}
