//! Wgpu artifact emission.
//!
//! Core IR is lowered through `vyre-lower`; this module only applies wgpu
//! runtime policy, emits a Naga module via `vyre-emit-naga`, and writes WGSL
//! accepted by `wgpu`.

pub(crate) mod descriptor_gate;

use crate::descriptor_mapping::{
    descriptor_bind_group, descriptor_buffer_access, descriptor_memory_kind,
};
use crate::WgpuBackend;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use std::sync::Arc;
use vyre_foundation::lower::LoweringError;

/// Binding assignment made by the wgpu lowering pipeline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WgpuBindingAssignment {
    /// Program buffer name.
    pub name: Arc<str>,
    /// Bind group index. Vyre wgpu programs currently use group 0.
    pub group: u32,
    /// Binding slot inside the group.
    pub binding: u32,
    /// Memory tier used to choose the wgpu address space.
    pub kind: vyre_foundation::ir::MemoryKind,
    /// Access mode declared by core IR.
    pub access: vyre_foundation::ir::BufferAccess,
    /// Element type carried by the binding.
    pub element: vyre_foundation::ir::DataType,
}

/// Dispatch geometry captured during backend IR lowering.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WgpuDispatchGeometry {
    /// Shader workgroup size.
    pub workgroup_size: [u32; 3],
    /// Static x workgroup count when it is derivable from output shape.
    pub workgroups: [u32; 3],
}

/// Backend-owned wgpu IR.
#[derive(Clone, Debug)]
pub struct WgpuProgram {
    /// Structurally emitted Naga module.
    pub module: naga::Module,
    /// Resource binding decisions.
    pub bindings: Vec<WgpuBindingAssignment>,
    /// Workgroup sizing chosen for the shader entry point.
    pub workgroup_size: [u32; 3],
    /// Dispatch geometry derived from program output declarations.
    pub dispatch_geometry: WgpuDispatchGeometry,
}

/// Lower a certified program to WGSL.
///
/// The shader text is produced only after structural Naga IR construction and
/// validation. Callers that need the module itself should use
/// [`vyre_lower::lower_for_emit`] and [`vyre_emit_naga::emit`].
///
/// # Errors
///
/// Returns [`LoweringError`] when the program cannot be represented in Naga,
/// validation fails, or the final writer fails.
#[inline]
pub fn lower(program: &vyre_foundation::ir::Program) -> Result<String, LoweringError> {
    lower_with_config(program, &vyre_driver::DispatchConfig::default())
}

/// Lower a program to WGSL with explicit dispatch policy.
///
/// # Errors
///
/// Returns [`LoweringError`] for invalid IR, failed Naga validation, or failed
/// WGSL writing.
pub fn lower_with_config(
    program: &vyre_foundation::ir::Program,
    config: &vyre_driver::DispatchConfig,
) -> Result<String, LoweringError> {
    let default_features = crate::runtime::device::EnabledFeatures::default();
    lower_with_features(program, config, &default_features)
}

/// Lower a program to WGSL with explicit dispatch policy and adapter features.
///
/// # Errors
///
/// Returns [`LoweringError`] for invalid IR, failed Naga validation, or failed
/// WGSL writing.
pub(crate) fn lower_with_features(
    program: &vyre_foundation::ir::Program,
    config: &vyre_driver::DispatchConfig,
    enabled_features: &crate::runtime::device::EnabledFeatures,
) -> Result<String, LoweringError> {
    let bir = WgpuProgram::from_program(program, config, enabled_features)?;
    write_wgsl(&bir.module)
}

/// Heuristic for selecting the optimal workgroup size for a program.
///
/// Innovation I.6: Adaptive workgroup sizing.
///
/// Takes the requested size from the program and the adapter capability
/// reports, and returns a size that maximizes occupancy and throughput.
/// Multi-axis workgroups are flattened to 1D [N, 1, 1] for current
/// scan-based vyre opcodes.
pub(crate) fn optimal_workgroup_size(
    program: &vyre_foundation::ir::Program,
    enabled_features: &crate::runtime::device::EnabledFeatures,
) -> [u32; 3] {
    let requested = program.workgroup_size;

    // If the program specified a non-default concrete size, honor it.
    // [1, 1, 1] is the legacy scalar default used by many builders.
    if requested != [1, 1, 1] && requested != [0, 0, 0] {
        return requested;
    }

    // Heuristic: use a multiple of the subgroup size.
    // If unknown (0), default to 64.
    let subgroup = enabled_features.min_subgroup_size.max(32);
    let size = if program.is_explicit_noop() {
        1
    } else {
        // For scan-heavy workloads, 4x subgroup size often yields
        // good occupancy without hitting register pressure.
        (subgroup * 4).min(256)
    };

    let max_x = enabled_features.max_workgroup_size[0].max(1);
    [size.min(max_x), 1, 1]
}

impl WgpuProgram {
    /// Build backend IR from a core program.
    ///
    /// # Errors
    ///
    /// Returns [`LoweringError`] when the program cannot be represented as
    /// wgpu/Naga IR.
    pub fn from_program(
        program: &vyre_foundation::ir::Program,
        config: &vyre_driver::DispatchConfig,
        enabled_features: &crate::runtime::device::EnabledFeatures,
    ) -> Result<Self, LoweringError> {
        let mut descriptor = descriptor_gate::validate_and_analyze(program)?;
        let workgroup_size = config
            .workgroup_override
            .unwrap_or_else(|| optimal_workgroup_size(program, enabled_features));
        descriptor.dispatch.workgroup_size = workgroup_size;

        if std::env::var("VYRE_DUMP_KDESC").is_ok() {
            dump_kdesc_if_requested(&descriptor, None);
        }

        let module = match emit_naga_module_for_descriptor(&descriptor) {
            Ok(module) => module,
            Err(error) => {
                if std::env::var("VYRE_CAPTURE_FAILED_DESCRIPTOR").is_ok() {
                    dump_kdesc_if_requested(&descriptor, None);
                }
                return Err(error);
            }
        };

        if std::env::var("VYRE_CAPTURE_FAILED_DESCRIPTOR").is_ok() {
            // Also capture success if specifically requested, or just have it ready if WGSL writing fails downstream.
            // Let's just capture it now, because from_program succeeds and WGSL writing might fail.
            // Wait, we only want to capture on failure.
            // Actually, we can just save it to a temporary location or just write it if the env var is set.
            // The spec says "On dispatch failure... serialize in-flight".
            // Since we don't know if WGSL will fail here, we can just proactively dump it if the feature is on.
            dump_kdesc_if_requested(&descriptor, Some(&module));
        }

        let bindings = binding_assignments(&descriptor);
        let dispatch_geometry = WgpuDispatchGeometry {
            workgroup_size,
            workgroups: static_workgroups(&descriptor, workgroup_size),
        };
        Ok(Self {
            module,
            bindings,
            workgroup_size,
            dispatch_geometry,
        })
    }
}

pub(crate) fn emit_naga_module_for_descriptor(
    descriptor: &vyre_lower::KernelDescriptor,
) -> Result<naga::Module, LoweringError> {
    if let Err(errors) = vyre_lower::verify::verify(descriptor) {
        return Err(LoweringError::invalid(format!(
            "KernelDescriptor verification failed after wgpu workgroup selection: {}. Fix: keep DispatchConfig.workgroup_override within descriptor limits.",
            format_descriptor_verify_errors(&errors)
        )));
    }
    vyre_emit_naga::emit(descriptor).map_err(|error| {
        LoweringError::invalid(format!(
            "KernelDescriptor Naga emission failed before wgpu WGSL writing: {error}. Fix: extend vyre-emit-naga descriptor emission; do not route around it with driver-local lowering."
        ))
    })
}

fn dump_kdesc_if_requested(
    descriptor: &vyre_lower::KernelDescriptor,
    module: Option<&naga::Module>,
) {
    if let Ok(dir) = std::env::var("VYRE_DUMP_KDESC")
        .or_else(|_| std::env::var("VYRE_CAPTURE_FAILED_DESCRIPTOR"))
    {
        let path = std::path::Path::new(&dir);
        if let Err(error) = std::fs::create_dir_all(path) {
            tracing::warn!(
                "Fix: failed to create WGPU descriptor dump directory `{}`: {error}",
                path.display()
            );
            return;
        }
        let id = &descriptor.id;

        let kdesc_path = path.join(format!("{id}.kdesc.bin"));
        match std::fs::File::create(&kdesc_path) {
            Ok(mut file) => {
                if let Err(error) = bincode::serde::encode_into_std_write(
                    descriptor,
                    &mut file,
                    bincode::config::standard(),
                ) {
                    tracing::warn!(
                        "Fix: failed to serialize WGPU KernelDescriptor dump `{}`: {error}",
                        kdesc_path.display()
                    );
                }
            }
            Err(error) => tracing::warn!(
                "Fix: failed to create WGPU KernelDescriptor dump `{}`: {error}",
                kdesc_path.display()
            ),
        }

        if let Some(m) = module {
            let module_path = path.join(format!("{id}.module.ron"));
            match std::fs::File::create(&module_path) {
                Ok(mut file) => {
                    use std::io::Write;
                    if let Err(error) = write!(file, "{m:#?}") {
                        tracing::warn!(
                            "Fix: failed to write WGPU Naga module dump `{}`: {error}",
                            module_path.display()
                        );
                    }
                }
                Err(error) => tracing::warn!(
                    "Fix: failed to create WGPU Naga module dump `{}`: {error}",
                    module_path.display()
                ),
            }
        }
    }
}

impl WgpuBackend {
    /// Lower core IR into the backend-owned wgpu IR.
    pub fn lower_to_backend_ir(
        &self,
        program: &vyre_foundation::ir::Program,
    ) -> Result<WgpuProgram, LoweringError> {
        WgpuProgram::from_program(
            program,
            &vyre_driver::DispatchConfig::default(),
            &self.enabled_features,
        )
    }

    /// Borrow the Naga module produced by lowering ([`WgpuProgram::from_program`]).
    ///
    /// This avoids cloning the entire [`naga::Module`]; callers that need an owned
    /// copy can call `.clone()` explicitly.
    #[must_use]
    pub fn lower_to_target<'a>(&self, bir: &'a WgpuProgram) -> &'a naga::Module {
        &bir.module
    }
}

fn write_wgsl(module: &naga::Module) -> Result<String, LoweringError> {
    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
    let info = match validator.validate(module) {
        Ok(info) => info,
        Err(e) => {
            // VYRE_NAGA_LOWER MEDIUM: replace `println!` with
            // structured tracing so shader constants and buffer
            // layouts don't leak to application stdout. `trace!`
            // level keeps the diagnostic available under
            // `RUST_LOG=vyre_driver_wgpu=trace` without shipping
            // it to normal logs.
            if let Some(func) = module.functions.iter().next() {
                tracing::trace!(
                    target: "vyre_driver_wgpu::naga",
                    function_expressions = ?func.1.expressions,
                    "naga validation failed  -  function expressions",
                );
            }
            if let Some(ep) = module.entry_points.first() {
                tracing::trace!(
                    target: "vyre_driver_wgpu::naga",
                    entrypoint_expressions = ?ep.function.expressions,
                    "naga validation failed  -  entrypoint expressions",
                );
                tracing::trace!(
                    target: "vyre_driver_wgpu::naga",
                    entrypoint_locals = ?ep.function.local_variables,
                    "naga validation failed  -  entrypoint local variables",
                );
                tracing::trace!(
                    target: "vyre_driver_wgpu::naga",
                    entrypoint_body = ?ep.function.body,
                    "naga validation failed  -  entrypoint body",
                );
            }
            return Err(LoweringError::validation(e));
        }
    };
    let wgsl =
        naga::back::wgsl::write_string(module, &info, naga::back::wgsl::WriterFlags::empty())
            .map_err(LoweringError::writer)?;
    // Emission size cap (Task #65): adapter shader-binary-size limits
    // are finite. At 1000+ fused arms WGSL source can exceed the
    // ceiling. Fail-fast at write_wgsl with a clear diagnostic
    // naming the byte count, instead of opaque pipeline-creation
    // failure downstream. The 32 MiB cap below is the safe floor  -
    // most adapters allow 256 MiB but Metal-on-iOS is the strictest.
    // Production adapters report their limit via wgpu::Limits; if the
    // FusionPlan partitioning harness is wired (Task #65 callers),
    // it consults the adapter limit and partitions before reaching
    // here. This guard is the last-line failsafe.
    const MAX_WGSL_BYTES: usize = 32 * 1024 * 1024;
    if wgsl.len() > MAX_WGSL_BYTES {
        return Err(LoweringError::invalid(format!(
            "emitted WGSL is {} bytes, exceeding the {MAX_WGSL_BYTES}-byte safety cap. Fix: partition the FusionPlan into multiple megakernels (group_a / group_b / ...) with shared standard pack, or split the source Program into smaller compilation units. Adapter shader-binary-size limits are finite at scale.",
            wgsl.len()
        )));
    }
    Ok(wgsl)
}

fn binding_assignments(descriptor: &vyre_lower::KernelDescriptor) -> Vec<WgpuBindingAssignment> {
    let mut assignments = Vec::with_capacity(descriptor.bindings.slots.len());
    for slot in &descriptor.bindings.slots {
        let Some(group) = descriptor_bind_group(slot.memory_class) else {
            continue;
        };
        assignments.push(WgpuBindingAssignment {
            name: Arc::from(slot.name.as_str()),
            group,
            binding: slot.slot,
            kind: descriptor_memory_kind(slot.memory_class),
            access: descriptor_buffer_access(slot.visibility),
            element: slot.element_type.clone(),
        });
    }
    assignments
}

fn static_workgroups(
    descriptor: &vyre_lower::KernelDescriptor,
    workgroup_size: [u32; 3],
) -> [u32; 3] {
    let output_words = descriptor
        .bindings
        .slots
        .iter()
        .filter(|slot| {
            matches!(slot.memory_class, vyre_lower::MemoryClass::Global)
                && matches!(
                    slot.visibility,
                    vyre_lower::BindingVisibility::WriteOnly
                        | vyre_lower::BindingVisibility::ReadWrite
                )
        })
        .filter_map(|slot| slot.element_count)
        .map(|count| count.max(1))
        .max()
        .unwrap_or(1);
    // Use the product of all workgroup dimensions as total thread count.
    // Previously only workgroup_size[0] was used, causing multi-dimensional
    // workgroups (e.g. [8,8,1] = 64 threads) to over-dispatch by the
    // product of the ignored dimensions.
    let total_threads =
        workgroup_size[0].max(1) * workgroup_size[1].max(1) * workgroup_size[2].max(1);
    [output_words.div_ceil(total_threads).max(1), 1, 1]
}

fn format_descriptor_verify_errors(errors: &[vyre_lower::VerifyError]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
    use vyre_lower::emit_adversarial_corpus::{self, EmitAdversarialBackend};

    #[test]
    fn wgpu_program_lowers_through_kernel_descriptor() {
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("out", 0, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(64),
            ],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(7))],
        );
        let mut config = vyre_driver::DispatchConfig::default();
        config.workgroup_override = Some([32, 1, 1]);
        let lowered = WgpuProgram::from_program(
            &program,
            &config,
            &crate::runtime::device::EnabledFeatures::default(),
        )
        .expect("Fix: wgpu lowering must use descriptor Naga emission");

        assert_eq!(lowered.workgroup_size, [32, 1, 1]);
        assert_eq!(lowered.dispatch_geometry.workgroups, [2, 1, 1]);
        assert_eq!(lowered.bindings.len(), 1);
        assert_eq!(lowered.bindings[0].name.as_ref(), "out");
        assert_eq!(lowered.bindings[0].group, 0);
        assert_eq!(lowered.bindings[0].binding, 0);
    }

    #[test]
    fn descriptor_binding_assignments_skip_non_resource_slots() {
        let descriptor = vyre_lower::KernelDescriptor {
            id: "bindings".into(),
            bindings: vyre_lower::BindingLayout {
                slots: vec![
                    vyre_lower::BindingSlot {
                        slot: 0,
                        element_type: DataType::U32,
                        element_count: Some(8),
                        memory_class: vyre_lower::MemoryClass::Shared,
                        visibility: vyre_lower::BindingVisibility::ReadWrite,
                        name: "scratch".to_owned(),
                    },
                    vyre_lower::BindingSlot {
                        slot: 1,
                        element_type: DataType::U32,
                        element_count: Some(8),
                        memory_class: vyre_lower::MemoryClass::Global,
                        visibility: vyre_lower::BindingVisibility::WriteOnly,
                        name: "out".to_owned(),
                    },
                ],
            },
            dispatch: vyre_lower::Dispatch::new(8, 1, 1),
            body: vyre_lower::KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let assignments = binding_assignments(&descriptor);
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].name.as_ref(), "out");
        assert_eq!(static_workgroups(&descriptor, [4, 1, 1]), [2, 1, 1]);
    }

    #[test]
    fn adversarial_success_corpus_passes_wgpu_descriptor_emit_path() {
        assert!(
            emit_adversarial_corpus::required_backends().contains(&EmitAdversarialBackend::Wgpu),
            "Fix: shared emit adversarial corpus must register WGPU as a required consumer."
        );

        for case in emit_adversarial_corpus::success_cases() {
            let module =
                emit_naga_module_for_descriptor(&case.descriptor).unwrap_or_else(|error| {
                    panic!(
                        "Fix: `{}` ({:?}) must pass WGPU descriptor emission: {}",
                        case.id,
                        case.family,
                        error.message()
                    )
                });
            assert_eq!(
                module.entry_points[0].name, "main",
                "{}: WGPU descriptor path must preserve compute entry point",
                case.id
            );
            assert_eq!(
                module.entry_points[0].workgroup_size, case.descriptor.dispatch.workgroup_size,
                "{}: WGPU descriptor path must preserve workgroup size before adapter override",
                case.id
            );
            assert!(
                binding_assignments(&case.descriptor).len() <= case.descriptor.bindings.slots.len(),
                "{}: WGPU binding assignment projection must not invent resource slots",
                case.id
            );
            assert!(
                static_workgroups(&case.descriptor, case.descriptor.dispatch.workgroup_size)[0]
                    >= 1,
                "{}: WGPU static dispatch geometry must produce at least one workgroup",
                case.id
            );
        }
    }

    #[test]
    fn adversarial_rejection_corpus_returns_structured_wgpu_errors() {
        for case in emit_adversarial_corpus::rejection_cases() {
            let error = emit_naga_module_for_descriptor(&case.descriptor)
                .expect_err("Fix: rejection corpus case must fail WGPU descriptor emission");
            assert!(
                error.message().contains("KernelDescriptor") && error.message().contains("Fix:"),
                "Fix: `{}` WGPU descriptor rejection must include structured KernelDescriptor repair text: {}",
                case.id,
                error.message()
            );
        }
    }

    /// Regression test: multi-dimensional workgroup sizes must use the
    /// product of all three dimensions as total thread count.
    /// Before the fix, only `workgroup_size[0]` was used, so a
    /// `[8, 8, 1]` workgroup (64 total threads) was treated as 8 threads,
    /// dispatching 8× too many workgroups.
    #[test]
    fn static_workgroups_multi_dimensional_uses_total_threads() {
        let descriptor = vyre_lower::KernelDescriptor {
            id: "multidim".into(),
            bindings: vyre_lower::BindingLayout {
                slots: vec![vyre_lower::BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: Some(256),
                    memory_class: vyre_lower::MemoryClass::Global,
                    visibility: vyre_lower::BindingVisibility::ReadWrite,
                    name: "out".to_owned(),
                }],
            },
            dispatch: vyre_lower::Dispatch::new(8, 8, 1),
            body: vyre_lower::KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        // [8, 8, 1] = 64 total threads → 256 / 64 = 4 workgroups
        assert_eq!(static_workgroups(&descriptor, [8, 8, 1]), [4, 1, 1]);
        // [4, 4, 4] = 64 total threads → 256 / 64 = 4 workgroups
        assert_eq!(static_workgroups(&descriptor, [4, 4, 4]), [4, 1, 1]);
        // [16, 1, 1] = 16 total threads → 256 / 16 = 16 workgroups
        assert_eq!(static_workgroups(&descriptor, [16, 1, 1]), [16, 1, 1]);
    }
}
