#![allow(
    clippy::doc_lazy_continuation,
    clippy::double_must_use,
    clippy::manual_div_ceil,
    clippy::needless_range_loop,
    clippy::collapsible_if,
    clippy::match_like_matches_macro,
    clippy::redundant_closure,
    clippy::too_many_arguments,
    clippy::nonminimal_bool,
    clippy::derivable_impls
)]
//! SPIR-V binary emitter for vyre `KernelDescriptor`.
//!
//! Substrate parity strategy: route the descriptor through
//! `vyre-emit-naga` to get a `naga::Module`, then use
//! `naga::back::spv::Writer` to produce a SPIR-V binary. This shares
//! the lossless lowering work with the wgpu/naga path  -  both backends
//! see the exact same naga::Module  -  and avoids forking a second
//! KernelOp → SPIR-V translation table.
//!
//! ## Op coverage
//!
//! Inherits from `vyre-emit-naga`. Anything that emit-naga refuses
//! also fails here. Anything emit-naga accepts gets converted to a
//! valid SPIR-V binary if naga's spv-out can lower it (essentially
//! everything except SPIR-V-specific extensions naga doesn't model).
//!
//! ## Validation gate
//!
//! `naga::valid::Validator` runs before `naga::back::spv::Writer`, so
//! any invalid module is rejected at the boundary. The emitted SPIR-V
//! binary is guaranteed to satisfy SPIR-V's structural requirements
//! per naga's spec compliance. Optional external `spirv-val` validation
//! sits in the integration-test surface (added when CI has spirv-tools).

use thiserror::Error;
use vyre_lower::KernelDescriptor;

/// SPIR-V anchor-DFA offload evidence.
pub mod anchor_dfa_offload;
pub mod patterns;

pub use anchor_dfa_offload::{
    SpirvAnchorDfaOffloadEvidence, SPIRV_ANCHOR_DFA_OFFLOAD_SCHEMA_VERSION,
};

#[derive(Debug, Error)]
pub enum EmitError {
    #[error("naga emission failed: {0}")]
    NagaEmit(#[from] vyre_emit_naga::EmitError),

    #[error("naga validation failed: {0}")]
    NagaValidation(String),

    #[error("SPIR-V writer construction failed: {0}")]
    WriterConstruction(String),

    #[error("SPIR-V writer.write failed: {0}")]
    WriterWrite(String),
}

/// Emit a SPIR-V binary from a `KernelDescriptor`.
///
/// Returns the raw SPIR-V words as a `Vec<u32>` (the canonical SPIR-V
/// representation per the spec). Callers that need bytes can convert
/// via `bytemuck::cast_slice` or by writing each word as little-endian
/// (SPIR-V is host-endian per spec but most consumers expect LE).
///
/// Use [`emit_optimized`] to run the `vyre_lower::rewrites::run_all`
/// pipeline before emission.
pub fn emit(desc: &KernelDescriptor) -> Result<Vec<u32>, EmitError> {
    let module = vyre_emit_naga::emit(desc).map_err(EmitError::NagaEmit)?;
    emit_from_naga_module(&module)
}

/// Emit a SPIR-V binary from an optimized form of `desc`  -  runs the
/// full vyre rewrite stack before lowering. Recommended over [`emit`]
/// for production use.
pub fn emit_optimized(desc: &KernelDescriptor) -> Result<Vec<u32>, EmitError> {
    emit_optimized_with_stats(desc).map(|(w, _)| w)
}

/// Like [`emit_optimized`] but also returns
/// [`vyre_lower::rewrites::OptimizationStats`].
pub fn emit_optimized_with_stats(
    desc: &KernelDescriptor,
) -> Result<(Vec<u32>, vyre_lower::rewrites::OptimizationStats), EmitError> {
    // Verify the INPUT descriptor before the rewrite pipeline. Each rewrite
    // pass assumes a valid descriptor and, in debug builds, `assert!`s validity
    // after every pass; an already-invalid descriptor would panic inside a pass
    // rather than surface as a structured error. Fail closed here so invalid
    // input is rejected identically in debug and release, before any pass runs.
    vyre_lower::verify::verify(desc).map_err(|errors| {
        EmitError::NagaEmit(vyre_emit_naga::EmitError::InvalidDescriptor(format!(
            "invalid descriptor: input failed verification before optimization — {} error(s). \
             Fix: see vyre_lower::verify for the invariants the descriptor violated. \
             First error: {:?}",
            errors.len(),
            errors.first()
        )))
    })?;
    let (optimized, stats) = vyre_lower::rewrites::run_all_with_stats(desc);
    // Unconditionally verify the rewrite pipeline output. A `debug_assert!`
    // here silently skips this gate in release builds, where a buggy rewrite
    // could produce an invalid descriptor that proceeds to SPIR-V emission and
    // yields a binary with semantics different from the original — a
    // silently-wrong result with no diagnostics. Fail closed instead.
    vyre_lower::verify::verify(&optimized).map_err(|errors| {
        EmitError::NagaEmit(vyre_emit_naga::EmitError::InvalidDescriptor(format!(
            "rewrite pipeline produced an invalid descriptor — {} error(s). \
             Fix: see vyre_lower::verify for the invariants the rewrite violated. \
             First error: {:?}",
            errors.len(),
            errors.first()
        )))
    })?;
    let words = emit(&optimized)?;
    Ok((words, stats))
}

/// Lower-level entry: emit SPIR-V from a pre-built naga::Module.
/// Useful when callers want to apply naga-level analyses or rewrites
/// between `vyre-emit-naga::emit` and SPIR-V conversion.
pub fn emit_from_naga_module(module: &naga::Module) -> Result<Vec<u32>, EmitError> {
    use naga::back::spv::{Options, PipelineOptions, Writer, WriterFlags};
    use naga::valid::{Capabilities, ValidationFlags, Validator};

    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
    let info = validator
        .validate(module)
        .map_err(|e| EmitError::NagaValidation(format!("{e:?}")))?;

    let options = Options {
        lang_version: (1, 3),
        capabilities: None,
        flags: WriterFlags::empty(),
        binding_map: Default::default(),
        zero_initialize_workgroup_memory:
            naga::back::spv::ZeroInitializeWorkgroupMemoryMode::Polyfill,
        force_loop_bounding: true,
        bounds_check_policies: naga::proc::BoundsCheckPolicies::default(),
        debug_info: None,
    };
    let pipeline = PipelineOptions {
        shader_stage: naga::ShaderStage::Compute,
        entry_point: "main".to_string(),
    };

    let mut writer = Writer::new(&options).map_err(|e| {
        EmitError::WriterConstruction(format!(
            "{e:?}. Fix: upgrade naga or relax spv-out feature flags."
        ))
    })?;
    let mut out = Vec::new();
    writer
        .write(module, &info, Some(&pipeline), &None, &mut out)
        .map_err(|e| EmitError::WriterWrite(format!("{e:?}")))?;
    Ok(out)
}

/// Convenience: emit SPIR-V as raw little-endian bytes (the form most
/// runtime loaders accept directly).
pub fn emit_bytes(desc: &KernelDescriptor) -> Result<Vec<u8>, EmitError> {
    words_to_le_bytes(emit(desc)?)
}

/// Like [`emit_bytes`] but runs the optimization pipeline first.
/// Recommended for production loaders that want minimal SPIR-V binary
/// size + already-optimized contents.
pub fn emit_optimized_bytes(desc: &KernelDescriptor) -> Result<Vec<u8>, EmitError> {
    words_to_le_bytes(emit_optimized(desc)?)
}

/// Combined optimization + bytes + stats.
pub fn emit_optimized_bytes_with_stats(
    desc: &KernelDescriptor,
) -> Result<(Vec<u8>, vyre_lower::rewrites::OptimizationStats), EmitError> {
    let (words, stats) = emit_optimized_with_stats(desc)?;
    Ok((words_to_le_bytes(words)?, stats))
}

fn words_to_le_bytes(words: Vec<u32>) -> Result<Vec<u8>, EmitError> {
    let mut bytes = Vec::with_capacity(words.len() * 4);
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    Ok(bytes)
}

/// SPIR-V magic number  -  `0x07230203` per the spec. Useful for
/// integration tests and consumer-side sanity checks.
pub const SPIRV_MAGIC: u32 = 0x07230203;

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_emit_naga::vyre_lower;
    use vyre_foundation::ir::DataType;
    use vyre_lower::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        KernelOp, KernelOpKind, LiteralValue, MemoryClass,
    };

    fn one_store_kernel() -> KernelDescriptor {
        KernelDescriptor {
            id: "store_one".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadWrite,
                    name: "out".into(),
                }],
            },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(7)],
            },
        }
    }

    #[test]
    fn empty_kernel_emits_valid_spirv_with_magic_header() {
        let desc = KernelDescriptor {
            id: "empty".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let words = emit(&desc).unwrap();
        assert!(!words.is_empty());
        assert_eq!(
            words[0], SPIRV_MAGIC,
            "first word must be the SPIR-V magic number"
        );
    }

    #[test]
    fn one_store_kernel_emits_non_trivial_spirv() {
        let words = emit(&one_store_kernel()).unwrap();
        assert!(
            words.len() > 16,
            "real kernel should produce more than the header"
        );
        assert_eq!(words[0], SPIRV_MAGIC);
    }

    #[test]
    fn emit_bytes_matches_words_in_le() {
        let desc = KernelDescriptor {
            id: "empty".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let words = emit(&desc).unwrap();
        let bytes = emit_bytes(&desc).unwrap();
        assert_eq!(bytes.len(), words.len() * 4);
        let first_word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert_eq!(first_word, SPIRV_MAGIC);
    }

    #[test]
    fn emit_optimized_bytes_produces_valid_spirv() {
        let desc = KernelDescriptor {
            id: "ob".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let bytes = emit_optimized_bytes(&desc).unwrap();
        assert!(bytes.len() >= 4);
        let first_word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert_eq!(first_word, SPIRV_MAGIC);
    }

    #[test]
    fn emit_optimized_bytes_with_stats_returns_both() {
        let desc = KernelDescriptor {
            id: "obs".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let (bytes, stats) = emit_optimized_bytes_with_stats(&desc).unwrap();
        assert!(bytes.len() >= 4);
        let first_word = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        assert_eq!(first_word, SPIRV_MAGIC);
        assert!(stats.iterations >= 1);
    }

    #[test]
    fn emit_with_unsupported_op_propagates_naga_error() {
        let desc = KernelDescriptor {
            id: "bad".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![KernelOp {
                    kind: KernelOpKind::SubgroupReduce { op: vyre_lower::SubgroupReduceOp::Add },
                    operands: vec![0],
                    result: Some(0),
                }],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let r = emit(&desc);
        assert!(matches!(r, Err(EmitError::NagaEmit(_))));
    }

    #[test]
    fn binop_add_emits_valid_spirv() {
        let kernel = KernelDescriptor {
            id: "add".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(vyre_foundation::ir::BinOp::Add),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(3), LiteralValue::U32(4)],
            },
        };
        let words = emit(&kernel).unwrap();
        assert_eq!(words[0], SPIRV_MAGIC);
        assert!(words.len() > 16);
    }

    #[test]
    fn spirv_magic_constant_matches_spec() {
        assert_eq!(SPIRV_MAGIC, 0x0723_0203);
    }

    #[test]
    fn emit_optimized_succeeds_and_produces_valid_spirv() {
        let words = emit_optimized(&one_store_kernel()).unwrap();
        assert_eq!(words[0], SPIRV_MAGIC);
        assert!(words.len() > 16);
    }

    #[test]
    fn emit_optimized_drops_dead_arithmetic() {
        // Same shape  -  identity + absorbing zero → dead after run_all.
        // Optimized SPIR-V should be no longer than raw.
        use vyre_foundation::ir::BinOp as Bo;
        let desc = KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: None,
                    memory_class: MemoryClass::Global,
                    visibility: BindingVisibility::ReadWrite,
                    name: "buf".into(),
                }],
            },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![1],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(Bo::Add),
                        operands: vec![1, 0],
                        result: Some(2),
                    },
                    KernelOp {
                        kind: KernelOpKind::BinOpKind(Bo::Mul),
                        operands: vec![1, 0],
                        result: Some(3),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![0, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0), LiteralValue::U32(99)],
            },
        };
        let raw = emit(&desc).unwrap();
        let optimized = emit_optimized(&desc).unwrap();
        assert!(
            optimized.len() <= raw.len(),
            "optimized SPIR-V ({} words) should not exceed raw ({} words)",
            optimized.len(),
            raw.len()
        );
    }

    #[test]
    fn emit_from_naga_module_independently_consumable() {
        // Build a valid naga::Module via emit-naga, then convert.
        let module = vyre_emit_naga::emit(&KernelDescriptor {
            id: "k".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(1, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        })
        .unwrap();
        let words = emit_from_naga_module(&module).unwrap();
        assert_eq!(words[0], SPIRV_MAGIC);
    }

    /// `emit_optimized` must return a structured error for a descriptor that
    /// fails the post-rewrite verification gate. Previously `debug_assert!`
    /// compiled to nothing in release builds, letting an invalid optimized
    /// descriptor silently proceed to SPIR-V emission and produce a binary
    /// with different semantics from the original. Replacing it with an
    /// unconditional `?` propagation makes this an observable failure in
    /// all build profiles.
    #[test]
    fn emit_optimized_errors_on_invalid_rewrite_output() {
        // A zero workgroup dimension is preserved unchanged by the rewrite
        // pipeline and always triggers VerifyErrorKind::DispatchZeroDim.
        let bad = KernelDescriptor {
            id: "zero_dim".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(0, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        };
        let result = emit_optimized(&bad);
        assert!(
            result.is_err(),
            "emit_optimized must return Err for a descriptor that fails post-rewrite verify; \
             the old debug_assert! would silently proceed in release builds"
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("invalid descriptor"),
            "error message must mention 'invalid descriptor', got: {err_msg}"
        );
    }
}
