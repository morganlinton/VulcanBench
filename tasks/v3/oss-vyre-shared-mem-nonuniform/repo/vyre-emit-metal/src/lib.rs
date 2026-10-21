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
//! Metal Shading Language emitter for vyre `KernelDescriptor`.
//!
//! This crate is the first native-Metal artifact seam. It deliberately reuses
//! the canonical LEGO path:
//!
//! ```text
//! Program -> vyre-lower::pre_emit -> KernelDescriptor
//! KernelDescriptor -> vyre-emit-naga -> naga::Module
//! naga::Module -> naga::back::msl -> MSL native_module artifact
//! ```
//!
//! It does not own runtime dispatch, device probing, buffer residency, or
//! semantic lowering. Those stay in the driver and lowerer layers.

use std::collections::BTreeMap;

use naga::back::msl::{BindTarget, EntryPointResources, Options, PipelineOptions};
use naga::{AddressSpace, StorageAccess};
use naga::valid::{Capabilities, ValidationFlags, Validator};
use thiserror::Error;
use vyre_lower::{BindingSlot, KernelDescriptor, MemoryClass};

/// `native_module` artifact schema version.
pub const METAL_ARTIFACT_SCHEMA: u32 = 3;

/// Default MSL target version used for source emission.
pub const DEFAULT_MSL_VERSION: (u8, u8) = (2, 4);

/// Metal emission options.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetalEmitOptions {
    /// Target MSL version.
    pub lang_version: (u8, u8),
    /// Entry point to emit.
    pub entry_point: String,
    /// Include deterministic MSL source in the JSON artifact.
    pub include_source: bool,
}

impl Default for MetalEmitOptions {
    fn default() -> Self {
        Self {
            lang_version: DEFAULT_MSL_VERSION,
            entry_point: "main".to_string(),
            include_source: true,
        }
    }
}

/// Metal emission failure.
#[derive(Debug, Error)]
pub enum EmitError {
    /// The shared Naga emitter rejected the descriptor.
    #[error("Naga emission failed before Metal MSL writing: {0}. Fix: extend vyre-emit-naga descriptor emission instead of forking Metal-local lowering.")]
    NagaEmit(String),
    /// Naga validation rejected the module.
    #[error("Naga validation failed before Metal MSL writing: {0}. Fix: repair the shared descriptor/Naga emission path before emitting native_module artifacts.")]
    NagaValidation(String),
    /// The requested entry point is missing or not compute.
    #[error("Metal entry point `{entry_point}` is unavailable for compute emission: {reason}. Fix: emit a compute KernelDescriptor with entry point `main` or pass the correct entry point name.")]
    EntryPoint { entry_point: String, reason: String },
    /// The MSL writer rejected the validated module.
    #[error("MSL writer failed: {0}. Fix: extend the shared Naga-to-MSL emission seam or lower unsupported constructs before Metal artifact emission.")]
    MslWriter(String),
    /// A binding could not be represented in Metal's flat buffer namespace.
    #[error("Metal binding map failed for resource group {group} binding {binding}: {reason}. Fix: keep Metal buffer indices within u8::MAX or add argument-buffer metadata before native_module emission.")]
    BindingMap {
        group: u32,
        binding: u32,
        reason: String,
    },
    /// Descriptor hashing failed.
    #[error("KernelDescriptor artifact hash failed: {0}. Fix: keep KernelDescriptor serde stable before using it as native_module artifact identity.")]
    DescriptorHash(String),
    /// Artifact serialization failed.
    #[error("Metal native_module JSON serialization failed: {0}. Fix: keep artifact metadata serde-compatible and deterministic.")]
    ArtifactSerialization(String),
    /// Program pre-emission lowering failed.
    #[error("Program pre-emission lowering failed before Metal artifact emission: {0}. Fix: route through vyre-lower::pre_emit and repair the neutral descriptor mapping.")]
    PreEmit(String),
}

/// ABI binding metadata stored in a Metal artifact.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MetalBindingMetadata {
    /// Vyre binding name.
    pub name: String,
    /// Descriptor slot.
    pub slot: u32,
    /// MSL buffer index.
    pub metal_buffer_index: u8,
    /// Element type as stable debug text until the shared artifact schema owns
    /// a typed cross-emitter field.
    pub element_type: String,
    /// Static element count when known.
    pub element_count: Option<u32>,
    /// Lowered memory class.
    pub memory_class: String,
    /// Read/write visibility.
    pub visibility: String,
}

/// Threadgroup-memory metadata stored in a Metal artifact.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MetalThreadgroupMemoryMetadata {
    /// Vyre binding name.
    pub name: String,
    /// Descriptor slot.
    pub slot: u32,
    /// Metal threadgroup-memory argument index.
    pub threadgroup_index: u8,
    /// Unaligned byte length required by the lowered workgroup binding.
    pub byte_length: u64,
    /// Metal-compatible 16-byte aligned allocation length.
    pub aligned_byte_length: u64,
}

/// Structured `native_module` artifact emitted by this crate.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MetalArtifact {
    /// Artifact schema version.
    pub schema: u32,
    /// Artifact target name used by `xtask compile --to native_module`.
    pub target: String,
    /// Emitter crate/version identity.
    pub emitter: String,
    /// MSL version.
    pub msl_version: String,
    /// Kernel entry point.
    pub entry_point: String,
    /// Canonical descriptor digest.
    pub descriptor_blake3: String,
    /// MSL source digest.
    pub msl_blake3: String,
    /// Dispatch workgroup shape.
    pub workgroup_size: [u32; 3],
    /// Binding ABI metadata.
    pub bindings: Vec<MetalBindingMetadata>,
    /// Metal buffer index used for Naga's `_buffer_sizes` sidecar when MSL
    /// bounds checks require runtime buffer lengths.
    pub sizes_buffer_index: Option<u8>,
    /// Threadgroup-memory arguments emitted by Naga for `var<workgroup>`
    /// globals. Runtime dispatch must allocate each index before launch.
    pub threadgroup_memories: Vec<MetalThreadgroupMemoryMetadata>,
    /// Generated MSL source.
    pub msl: String,
}

/// Emit MSL source from a descriptor using default Metal options.
///
/// # Errors
///
/// Returns [`EmitError`] when Naga emission, validation, binding-map creation,
/// or MSL writing fails.
pub fn emit(desc: &KernelDescriptor) -> Result<String, EmitError> {
    emit_with_options(desc, &MetalEmitOptions::default())
}

/// Emit MSL source from a descriptor using explicit options.
///
/// # Errors
///
/// Returns [`EmitError`] when Naga emission, validation, binding-map creation,
/// or MSL writing fails.
pub fn emit_with_options(
    desc: &KernelDescriptor,
    options: &MetalEmitOptions,
) -> Result<String, EmitError> {
    emit_artifact_with_options(desc, options).map(|artifact| artifact.msl)
}

/// Emit MSL source after the canonical descriptor rewrite stack.
///
/// # Errors
///
/// Same as [`emit`].
pub fn emit_optimized(desc: &KernelDescriptor) -> Result<String, EmitError> {
    emit_optimized_with_stats(desc).map(|(source, _)| source)
}

/// Emit optimized MSL source and descriptor rewrite statistics.
///
/// # Errors
///
/// Same as [`emit`].
pub fn emit_optimized_with_stats(
    desc: &KernelDescriptor,
) -> Result<(String, vyre_lower::rewrites::OptimizationStats), EmitError> {
    let (optimized, stats) = vyre_lower::verify_then_optimize(desc)
        .map_err(|error| EmitError::NagaValidation(format!("{error:?}")))?;
    let source = emit(&optimized)?;
    Ok((source, stats))
}

/// Emit a structured `native_module` artifact.
///
/// # Errors
///
/// Returns [`EmitError`] when MSL emission or artifact serialization metadata
/// construction fails.
pub fn emit_artifact(desc: &KernelDescriptor) -> Result<MetalArtifact, EmitError> {
    emit_artifact_with_options(desc, &MetalEmitOptions::default())
}

/// Emit a structured `native_module` artifact with explicit options.
///
/// # Errors
///
/// Returns [`EmitError`] when MSL emission or artifact serialization metadata
/// construction fails.
pub fn emit_artifact_with_options(
    desc: &KernelDescriptor,
    options: &MetalEmitOptions,
) -> Result<MetalArtifact, EmitError> {
    let index_map = metal_binding_indices(desc)?;
    let bindings = metal_bindings(desc, &index_map.by_slot)?;
    let threadgroup_memories = metal_threadgroup_memories(desc)?;
    let module =
        vyre_emit_naga::emit(desc).map_err(|error| EmitError::NagaEmit(error.to_string()))?;
    let (msl, entry_point, sizes_buffer_index) =
        emit_from_naga_module_with_resource_indices(&module, options, &index_map.by_resource)?;
    let descriptor_blake3 = descriptor_blake3(desc)?;
    let msl_blake3 = hex_encode(blake3::hash(msl.as_bytes()).as_bytes());
    Ok(MetalArtifact {
        schema: METAL_ARTIFACT_SCHEMA,
        target: "native_module".to_string(),
        emitter: format!("vyre-emit-metal/{}", env!("CARGO_PKG_VERSION")),
        msl_version: format!("{}.{}", options.lang_version.0, options.lang_version.1),
        entry_point,
        descriptor_blake3,
        msl_blake3,
        workgroup_size: desc.dispatch.workgroup_size,
        bindings,
        sizes_buffer_index,
        threadgroup_memories,
        msl: if options.include_source {
            msl
        } else {
            String::new()
        },
    })
}

/// Emit a deterministic JSON `native_module` artifact.
///
/// # Errors
///
/// Returns [`EmitError`] when emission or JSON serialization fails.
pub fn emit_artifact_json(desc: &KernelDescriptor) -> Result<String, EmitError> {
    artifact_to_json(&emit_artifact(desc)?)
}

/// Emit deterministic JSON bytes for `xtask compile --to native_module`.
///
/// # Errors
///
/// Same as [`emit_artifact_json`].
pub fn emit_artifact_bytes(desc: &KernelDescriptor) -> Result<Vec<u8>, EmitError> {
    let mut bytes = emit_artifact_json(desc)?.into_bytes();
    bytes.push(b'\n');
    Ok(bytes)
}

/// Lower a high-level Program through the canonical pre-emit seam and emit a
/// deterministic JSON `native_module` artifact.
///
/// # Errors
///
/// Returns [`EmitError`] when pre-emission lowering or artifact emission fails.
pub fn emit_program_artifact_json(
    program: &vyre_foundation::ir::Program,
) -> Result<String, EmitError> {
    let lowered = vyre_lower::pre_emit::lower_for_emit(program)
        .map_err(|error| EmitError::PreEmit(error.to_string()))?;
    emit_artifact_json(&lowered.descriptor)
}

/// Lower a high-level Program and emit deterministic JSON bytes.
///
/// # Errors
///
/// Same as [`emit_program_artifact_json`].
pub fn emit_program_artifact_bytes(
    program: &vyre_foundation::ir::Program,
) -> Result<Vec<u8>, EmitError> {
    let mut bytes = emit_program_artifact_json(program)?.into_bytes();
    bytes.push(b'\n');
    Ok(bytes)
}

/// Emit MSL from an already-constructed Naga module.
///
/// This is the narrow seam used by tests and future direct driver code that
/// wants to inspect or transform Naga before Metal source generation.
///
/// # Errors
///
/// Returns [`EmitError`] when validation, resource mapping, or MSL writing
/// fails.
pub fn emit_from_naga_module(
    module: &naga::Module,
    options: &MetalEmitOptions,
) -> Result<String, EmitError> {
    emit_from_naga_module_with_entry_name(module, options).map(|(source, _, _)| source)
}

fn emit_from_naga_module_with_entry_name(
    module: &naga::Module,
    options: &MetalEmitOptions,
) -> Result<(String, String, Option<u8>), EmitError> {
    let resource_indices = metal_resource_indices_from_module(module)?;
    emit_from_naga_module_with_resource_indices(module, options, &resource_indices)
}

fn emit_from_naga_module_with_resource_indices(
    module: &naga::Module,
    options: &MetalEmitOptions,
    resource_indices: &BTreeMap<(u32, u32), u8>,
) -> Result<(String, String, Option<u8>), EmitError> {
    let entry_index = ensure_compute_entry_point(module, &options.entry_point)?;

    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
    let info = validator
        .validate(module)
        .map_err(|error| EmitError::NagaValidation(format!("{error:?}")))?;

    let mut msl_options = Options::default();
    msl_options.lang_version = options.lang_version;
    msl_options.fake_missing_bindings = false;
    let resource_map =
        metal_entry_point_resource_map(module, &options.entry_point, resource_indices)?;
    msl_options.per_entry_point_map = resource_map.per_entry_point;

    let pipeline_options = PipelineOptions::default();
    let (source, translation) =
        naga::back::msl::write_string(module, &info, &msl_options, &pipeline_options)
            .map_err(|error| EmitError::MslWriter(format!("{error:?}")))?;
    let entry_point = translation
        .entry_point_names
        .get(entry_index)
        .ok_or_else(|| EmitError::EntryPoint {
            entry_point: options.entry_point.clone(),
            reason: "MSL translation omitted the entry point name mapping".to_string(),
        })?
        .as_ref()
        .map_err(|error| EmitError::EntryPoint {
            entry_point: options.entry_point.clone(),
            reason: format!("MSL writer rejected the entry point name: {error:?}"),
        })?
        .clone();
    Ok((source, entry_point, resource_map.sizes_buffer_index))
}

fn ensure_compute_entry_point(
    module: &naga::Module,
    entry_point: &str,
) -> Result<usize, EmitError> {
    let Some((index, ep)) = module
        .entry_points
        .iter()
        .enumerate()
        .find(|(_index, ep)| ep.name == entry_point)
    else {
        return Err(EmitError::EntryPoint {
            entry_point: entry_point.to_string(),
            reason: "entry point name was not present in the Naga module".to_string(),
        });
    };
    if ep.stage != naga::ShaderStage::Compute {
        return Err(EmitError::EntryPoint {
            entry_point: entry_point.to_string(),
            reason: format!("stage was {:?}, not Compute", ep.stage),
        });
    }
    Ok(index)
}

fn metal_entry_point_resource_map(
    module: &naga::Module,
    entry_point: &str,
    resource_indices: &BTreeMap<(u32, u32), u8>,
) -> Result<MetalResourceMap, EmitError> {
    let mut resources = EntryPointResources::default();
    let mut max_buffer = None::<u8>;
    for (_handle, global) in module.global_variables.iter() {
        let Some(binding) = global.binding else {
            continue;
        };
        let buffer = resource_indices
            .get(&(binding.group, binding.binding))
            .copied()
            .ok_or_else(|| EmitError::BindingMap {
                group: binding.group,
                binding: binding.binding,
                reason: "resource binding was not present in the dense Metal buffer index map"
                    .to_string(),
            })?;
        // Naga MSL backend uses `mutable` to choose between `const device T*`
        // (read-only) and `device T*` (read-write). We derive the flag from
        // the address space recorded on the global variable by vyre-emit-naga:
        // `AddressSpace::Storage { access: LOAD }` is read-only; any access
        // that includes STORE is mutable. Hardcoding `true` prevents Metal's
        // compiler from alias-analysing read-only inputs and can produce
        // undefined behaviour under strict Metal validation.
        let is_read_only =
            matches!(global.space, AddressSpace::Storage { access } if access == StorageAccess::LOAD);
        resources.resources.insert(
            binding,
            BindTarget {
                buffer: Some(buffer),
                texture: None,
                sampler: None,
                mutable: !is_read_only,
            },
        );
        max_buffer = Some(max_buffer.map_or(buffer, |max| max.max(buffer)));
    }
    let sizes_buffer_index = max_buffer
        .map(|max| {
            max.checked_add(1).ok_or_else(|| EmitError::BindingMap {
                group: 0,
                binding: u32::from(max),
                reason: "no free Metal buffer slot remains for Naga's _buffer_sizes sidecar"
                    .to_string(),
            })
        })
        .transpose()?;
    resources.sizes_buffer = sizes_buffer_index;
    let mut map = BTreeMap::new();
    map.insert(entry_point.to_string(), resources);
    Ok(MetalResourceMap {
        per_entry_point: map,
        sizes_buffer_index,
    })
}

struct MetalResourceMap {
    per_entry_point: BTreeMap<String, EntryPointResources>,
    sizes_buffer_index: Option<u8>,
}

struct MetalBindingIndexMap {
    by_slot: BTreeMap<u32, u8>,
    by_resource: BTreeMap<(u32, u32), u8>,
}

fn metal_binding_indices(desc: &KernelDescriptor) -> Result<MetalBindingIndexMap, EmitError> {
    let mut by_slot = BTreeMap::new();
    let mut by_resource = BTreeMap::new();
    for slot in &desc.bindings.slots {
        let Some(group) = metal_resource_group(slot) else {
            continue;
        };
        let metal_buffer_index =
            u8::try_from(by_slot.len()).map_err(|error| EmitError::BindingMap {
                group,
                binding: slot.slot,
                reason: format!(
                    "too many resource-bound buffers for Metal's flat buffer namespace: {error}"
                ),
            })?;
        if by_slot.insert(slot.slot, metal_buffer_index).is_some() {
            return Err(EmitError::BindingMap {
                group,
                binding: slot.slot,
                reason: "duplicate descriptor slot in Metal resource binding map".to_string(),
            });
        }
        if by_resource
            .insert((group, slot.slot), metal_buffer_index)
            .is_some()
        {
            return Err(EmitError::BindingMap {
                group,
                binding: slot.slot,
                reason: "duplicate resource binding in Metal resource binding map".to_string(),
            });
        }
    }
    Ok(MetalBindingIndexMap {
        by_slot,
        by_resource,
    })
}

fn metal_resource_indices_from_module(
    module: &naga::Module,
) -> Result<BTreeMap<(u32, u32), u8>, EmitError> {
    let mut keys = BTreeMap::new();
    for (_handle, global) in module.global_variables.iter() {
        let Some(binding) = global.binding else {
            continue;
        };
        if keys.insert((binding.group, binding.binding), ()).is_some() {
            return Err(EmitError::BindingMap {
                group: binding.group,
                binding: binding.binding,
                reason: "duplicate Naga resource binding in Metal resource map".to_string(),
            });
        }
    }
    let mut indices = BTreeMap::new();
    for (index, key) in keys.keys().copied().enumerate() {
        let metal_buffer_index = u8::try_from(index).map_err(|error| EmitError::BindingMap {
            group: key.0,
            binding: key.1,
            reason: format!(
                "too many Naga resource bindings for Metal's flat buffer namespace: {error}"
            ),
        })?;
        indices.insert(key, metal_buffer_index);
    }
    Ok(indices)
}

fn metal_resource_group(slot: &BindingSlot) -> Option<u32> {
    match slot.memory_class {
        MemoryClass::Uniform => Some(1),
        MemoryClass::Global | MemoryClass::Constant => Some(0),
        MemoryClass::Shared | MemoryClass::Scratch => None,
    }
}

fn metal_bindings(
    desc: &KernelDescriptor,
    slot_indices: &BTreeMap<u32, u8>,
) -> Result<Vec<MetalBindingMetadata>, EmitError> {
    let mut out = Vec::with_capacity(desc.bindings.slots.len());
    for slot in &desc.bindings.slots {
        let Some(group) = metal_resource_group(slot) else {
            continue;
        };
        let metal_buffer_index =
            slot_indices
                .get(&slot.slot)
                .copied()
                .ok_or_else(|| EmitError::BindingMap {
                    group,
                    binding: slot.slot,
                    reason: "descriptor slot was missing from the dense Metal buffer index map"
                        .to_string(),
                })?;
        out.push(MetalBindingMetadata {
            name: slot.name.clone(),
            slot: slot.slot,
            metal_buffer_index,
            element_type: format!("{:?}", slot.element_type),
            element_count: slot.element_count,
            memory_class: format!("{:?}", slot.memory_class),
            visibility: format!("{:?}", slot.visibility),
        });
    }
    Ok(out)
}

fn metal_threadgroup_memories(
    desc: &KernelDescriptor,
) -> Result<Vec<MetalThreadgroupMemoryMetadata>, EmitError> {
    let mut out = Vec::new();
    for slot in &desc.bindings.slots {
        if slot.memory_class != MemoryClass::Shared {
            continue;
        }
        let threadgroup_index =
            u8::try_from(out.len()).map_err(|error| EmitError::BindingMap {
                group: 0,
                binding: slot.slot,
                reason: format!(
                    "too many threadgroup-memory bindings for Metal's flat threadgroup namespace: {error}"
                ),
            })?;
        let element_count = slot.element_count.ok_or_else(|| EmitError::BindingMap {
            group: 0,
            binding: slot.slot,
            reason: "threadgroup memory requires static element_count metadata".to_string(),
        })?;
        let element_size =
            u64::try_from(slot.element_type.min_bytes().max(4)).map_err(|error| {
                EmitError::BindingMap {
                    group: 0,
                    binding: slot.slot,
                    reason: format!("threadgroup element byte size does not fit u64: {error}"),
                }
            })?;
        let byte_length = u64::from(element_count)
            .checked_mul(element_size)
            .ok_or_else(|| EmitError::BindingMap {
                group: 0,
                binding: slot.slot,
                reason: "threadgroup byte length overflows u64".to_string(),
            })?;
        out.push(MetalThreadgroupMemoryMetadata {
            name: slot.name.clone(),
            slot: slot.slot,
            threadgroup_index,
            byte_length,
            aligned_byte_length: align_u64(byte_length, 16)?,
        });
    }
    Ok(out)
}

fn align_u64(value: u64, alignment: u64) -> Result<u64, EmitError> {
    value
        .checked_add(alignment - 1)
        .map(|rounded| rounded / alignment * alignment)
        .ok_or_else(|| EmitError::BindingMap {
            group: 0,
            binding: 0,
            reason: format!("Metal alignment to {alignment} bytes overflows u64"),
        })
}

fn descriptor_blake3(desc: &KernelDescriptor) -> Result<String, EmitError> {
    let bytes = bincode::serde::encode_to_vec(desc, bincode::config::standard())
        .map_err(|error| EmitError::DescriptorHash(error.to_string()))?;
    Ok(hex_encode(blake3::hash(&bytes).as_bytes()))
}

fn artifact_to_json(artifact: &MetalArtifact) -> Result<String, EmitError> {
    serde_json::to_string_pretty(artifact)
        .map_err(|error| EmitError::ArtifactSerialization(error.to_string()))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::{BinOp, DataType, Expr, Node, Program};
    use vyre_lower::{
        BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
        KernelOp, KernelOpKind, LiteralValue, MemoryClass,
    };

    fn empty_kernel() -> KernelDescriptor {
        KernelDescriptor {
            id: "empty".into(),
            bindings: BindingLayout { slots: vec![] },
            dispatch: Dispatch::new(64, 1, 1),
            body: KernelBody {
                ops: vec![],
                child_bodies: vec![],
                literals: vec![],
            },
        }
    }

    fn one_store_kernel() -> KernelDescriptor {
        KernelDescriptor {
            id: "store_one".into(),
            bindings: BindingLayout {
                slots: vec![BindingSlot {
                    slot: 0,
                    element_type: DataType::U32,
                    element_count: Some(1),
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
    fn empty_kernel_emits_compute_msl() {
        let msl = emit(&empty_kernel()).unwrap();
        assert!(
            msl.contains("kernel"),
            "MSL source must include a Metal kernel entry point"
        );
        assert!(
            msl.contains("main"),
            "MSL source must include the canonical main entry point"
        );
    }

    #[test]
    fn one_store_kernel_emits_buffer_binding_metadata() {
        let artifact = emit_artifact(&one_store_kernel()).unwrap();
        assert_eq!(artifact.target, "native_module");
        assert!(
            artifact.msl.contains(&artifact.entry_point),
            "artifact entry point must name the actual emitted MSL function"
        );
        assert_eq!(artifact.workgroup_size, [64, 1, 1]);
        assert_eq!(artifact.bindings.len(), 1);
        assert_eq!(artifact.bindings[0].name, "out");
        assert_eq!(artifact.bindings[0].metal_buffer_index, 0);
        assert_eq!(artifact.sizes_buffer_index, Some(1));
        assert!(artifact.threadgroup_memories.is_empty());
        assert!(!artifact.msl.is_empty());
    }

    #[test]
    fn artifact_json_is_deterministic_for_same_descriptor() {
        let desc = one_store_kernel();
        let left = emit_artifact_json(&desc).unwrap();
        let right = emit_artifact_json(&desc).unwrap();
        assert_eq!(left, right);
    }

    #[test]
    fn emit_optimized_returns_stats_and_source() {
        let desc = KernelDescriptor {
            id: "dead_add".into(),
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
                        kind: KernelOpKind::BinOpKind(BinOp::Add),
                        operands: vec![0, 1],
                        result: Some(2),
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(1), LiteralValue::U32(2)],
            },
        };
        let (msl, stats) = emit_optimized_with_stats(&desc).unwrap();
        assert!(msl.contains("kernel"));
        assert!(stats.iterations >= 1);
    }

    #[test]
    fn missing_entry_point_returns_actionable_error() {
        let module = vyre_emit_naga::emit(&empty_kernel()).unwrap();
        let options = MetalEmitOptions {
            entry_point: "not_main".to_string(),
            ..MetalEmitOptions::default()
        };
        let error = emit_from_naga_module(&module, &options).unwrap_err();
        let text = error.to_string();
        assert!(text.contains("Fix:"));
        assert!(text.contains("not_main"));
    }

    #[test]
    fn descriptor_slot_above_metal_flat_limit_is_remapped() {
        let mut desc = one_store_kernel();
        desc.bindings.slots[0].slot = 300;
        desc.body.ops[2].operands[0] = 300;
        let artifact = emit_artifact(&desc).unwrap();
        assert_eq!(artifact.bindings[0].slot, 300);
        assert_eq!(artifact.bindings[0].metal_buffer_index, 0);
        assert_eq!(artifact.sizes_buffer_index, Some(1));
    }

    #[test]
    fn workgroup_slot_is_not_a_metal_buffer_binding() {
        let mut desc = one_store_kernel();
        desc.bindings.slots.push(BindingSlot {
            slot: 1 << 24,
            element_type: DataType::U32,
            element_count: Some(4),
            memory_class: MemoryClass::Shared,
            visibility: BindingVisibility::ReadWrite,
            name: "tile".into(),
        });
        desc.bindings.slots.sort_by_key(|slot| slot.slot);
        let artifact = emit_artifact(&desc).unwrap();
        assert_eq!(artifact.bindings.len(), 1);
        assert_eq!(artifact.bindings[0].name, "out");
        assert_eq!(artifact.bindings[0].metal_buffer_index, 0);
        assert_eq!(artifact.sizes_buffer_index, Some(1));
        assert_eq!(artifact.threadgroup_memories.len(), 1);
        assert_eq!(artifact.threadgroup_memories[0].name, "tile");
        assert_eq!(artifact.threadgroup_memories[0].slot, 1 << 24);
        assert_eq!(artifact.threadgroup_memories[0].threadgroup_index, 0);
        assert_eq!(artifact.threadgroup_memories[0].byte_length, 16);
        assert_eq!(artifact.threadgroup_memories[0].aligned_byte_length, 16);
    }

    #[test]
    fn metal_resource_count_without_sidecar_room_is_rejected() {
        let mut desc = empty_kernel();
        desc.bindings.slots = (0..=255)
            .map(|slot| BindingSlot {
                slot,
                element_type: DataType::U32,
                element_count: Some(1),
                memory_class: MemoryClass::Global,
                visibility: BindingVisibility::ReadOnly,
                name: format!("buf{slot}"),
            })
            .collect();
        let error = emit_artifact(&desc).unwrap_err();
        let text = error.to_string();
        assert!(text.contains("Fix:"));
        assert!(text.contains("_buffer_sizes"));
    }

    #[test]
    fn trap_sidecar_compare_exchange_emits_msl_helper() {
        let program = Program::wrapped(vec![], [64, 1, 1], vec![Node::trap(Expr::u32(7), "fault")]);
        let desc = vyre_lower::lower(&program).expect("Fix: trap programs must descriptor-lower");
        let artifact = emit_artifact(&desc).expect("Fix: trap descriptors must emit Metal MSL");

        assert!(
            artifact
                .msl
                .contains("naga_atomic_compare_exchange_weak_explicit"),
            "Fix: trap/CAS Metal MSL must include Naga's compare-exchange helper."
        );
        let sidecar = artifact
            .bindings
            .iter()
            .find(|binding| binding.name == vyre_lower::TRAP_SIDECAR_NAME)
            .expect("Fix: trap sidecar must stay host-bound in Metal metadata.");
        assert_eq!(
            sidecar.element_count,
            Some(vyre_lower::TRAP_SIDECAR_WORDS),
            "Fix: trap sidecar metadata must preserve the four-word runtime ABI."
        );
    }

    /// A read-only binding must produce `const device` in MSL, not a mutable
    /// `device` pointer. Before this fix `mutable: true` was hardcoded for
    /// every binding regardless of `BindingVisibility`, which prevented Naga's
    /// MSL backend from emitting `const device T*` and could cause silent
    /// miscompilation under Metal's strict aliasing rules.
    #[test]
    fn readonly_binding_emits_const_device_pointer() {
        let desc = KernelDescriptor {
            id: "readonly_smoke".into(),
            bindings: BindingLayout {
                slots: vec![
                    BindingSlot {
                        slot: 0,
                        element_type: DataType::U32,
                        element_count: Some(16),
                        memory_class: MemoryClass::Global,
                        visibility: BindingVisibility::ReadOnly,
                        name: "input".into(),
                    },
                    BindingSlot {
                        slot: 1,
                        element_type: DataType::U32,
                        element_count: Some(16),
                        memory_class: MemoryClass::Global,
                        visibility: BindingVisibility::ReadWrite,
                        name: "output".into(),
                    },
                ],
            },
            dispatch: Dispatch::new(16, 1, 1),
            body: KernelBody {
                ops: vec![
                    KernelOp {
                        kind: KernelOpKind::Literal,
                        operands: vec![0],
                        result: Some(0),
                    },
                    KernelOp {
                        kind: KernelOpKind::LoadGlobal,
                        operands: vec![0, 0],
                        result: Some(1),
                    },
                    KernelOp {
                        kind: KernelOpKind::StoreGlobal,
                        operands: vec![1, 0, 1],
                        result: None,
                    },
                ],
                child_bodies: vec![],
                literals: vec![LiteralValue::U32(0)],
            },
        };
        let msl = emit(&desc)
            .expect("Fix: read-only + read-write kernel must emit MSL without error.");
        // Naga's MSL backend emits a read-only storage buffer as a CONST
        // reference (`device <type> const& name`) and a read-write buffer as a
        // mutable reference (`device <type>& name`). The read-only "input"
        // binding (BindingVisibility::ReadOnly -> AddressSpace access LOAD ->
        // BindTarget.mutable = false) MUST be const-qualified; the read-write
        // "output" binding MUST NOT be. Before the fix `mutable: true` was
        // hardcoded for every binding, regressing "input" to a mutable
        // `device input_elements&` and losing Metal's read-only aliasing
        // guarantees. (Naga spells it `device T const&`, not `const device T*`.)
        assert!(
            msl.contains("input_elements const&"),
            "read-only `input` binding must emit a `device <type> const&` reference, not a mutable one. MSL excerpt:\n{}",
            &msl[..msl.len().min(800)]
        );
        assert!(
            !msl.contains("output_elements const&"),
            "read-write `output` binding must NOT be const-qualified. MSL excerpt:\n{}",
            &msl[..msl.len().min(800)]
        );
    }
}
