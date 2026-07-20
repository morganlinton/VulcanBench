//! Module construction layer: `emit_uncached` orchestration plus the
//! one-shot helpers it needs (binding insertion, scalar-type creation,
//! atomic-slot scan, trap-sidecar discovery, builtin function-arg push).
//!
//! `BodyBuilder` is the per-function emission state and lives in
//! `mod.rs`; everything below the BodyBuilder boundary is op-emit logic
//! split into its own files. This file is the *outside-in* layer.

use rustc_hash::{FxHashMap, FxHashSet};
use std::num::NonZeroU32;

use naga::{
    AddressSpace, ArraySize, Binding, BuiltIn, EntryPoint, Function, FunctionArgument,
    GlobalVariable, Module, ResourceBinding, Scalar, ScalarKind, ShaderStage, Span, StorageAccess,
    Type, TypeInner, VectorSize,
};
use vyre_foundation::ir::DataType;
use vyre_lower::{
    BindingSlot, BindingVisibility, KernelBody, KernelDescriptor, KernelOpKind, MemoryClass,
    TRAP_SIDECAR_NAME,
};

use super::BodyBuilder;
use crate::EmitError;

/// Backend-owned WGSL identifier for the trap sidecar global. The WGSL name is
/// internal to the shader — the host binds the sidecar by descriptor slot name
/// (`vyre_lower::TRAP_SIDECAR_NAME` = `__vyre_descriptor_trap_sidecar`) plus the
/// numeric `@group/@binding`, never by the WGSL identifier — so the wgpu Naga
/// backend names its own sidecar global. `trap_nodes_lower_to_backend_sidecar`
/// pins this contract.
const WGSL_TRAP_SIDECAR_GLOBAL: &str = "vyre_wgpu_trap_sidecar";

#[derive(Clone, Copy)]
pub(super) struct TypeHandles {
    pub(super) bool_ty: naga::Handle<Type>,
    pub(super) u32_ty: naga::Handle<Type>,
    pub(super) i32_ty: naga::Handle<Type>,
    pub(super) f32_ty: naga::Handle<Type>,
    pub(super) f64_ty: naga::Handle<Type>,
    pub(super) u64_ty: naga::Handle<Type>,
    pub(super) i64_ty: naga::Handle<Type>,
    /// WGSL has no native 64-bit integer, so buffer-backed `U64`/`I64`/`Vec2U32`
    /// values are represented as `vec2<u32>` (low word `.x`, high word `.y`).
    /// `u64_ty`/`i64_ty` above stay native scalars for the subgroup/literal
    /// paths that emit them directly under a u64 capability.
    pub(super) vec2_u32_ty: naga::Handle<Type>,
    pub(super) vec3_u32_ty: naga::Handle<Type>,
    pub(super) vec4_u32_ty: naga::Handle<Type>,
    pub(super) atomic_compare_exchange_u32_ty: naga::Handle<Type>,
}

#[derive(Clone, Copy)]
pub(super) struct Builtins {
    pub(super) global: u32,
    pub(super) workgroup: u32,
    pub(super) local: u32,
    pub(super) subgroup_local: Option<u32>,
    pub(super) subgroup_size: Option<u32>,
}

impl Builtins {
    fn push(function: &mut Function, types: TypeHandles, uses_subgroup: bool) -> Self {
        let subgroup_local = uses_subgroup.then(|| {
            push_builtin_arg(
                function,
                "_vyre_subgroup_local_id",
                types.u32_ty,
                BuiltIn::SubgroupInvocationId,
            )
        });
        let subgroup_size = uses_subgroup.then(|| {
            push_builtin_arg(
                function,
                "_vyre_subgroup_size",
                types.u32_ty,
                BuiltIn::SubgroupSize,
            )
        });
        Self {
            global: push_builtin_arg(
                function,
                "_vyre_global_id",
                types.vec3_u32_ty,
                BuiltIn::GlobalInvocationId,
            ),
            workgroup: push_builtin_arg(
                function,
                "_vyre_workgroup_id",
                types.vec3_u32_ty,
                BuiltIn::WorkGroupId,
            ),
            local: push_builtin_arg(
                function,
                "_vyre_local_id",
                types.vec3_u32_ty,
                BuiltIn::LocalInvocationId,
            ),
            subgroup_local,
            subgroup_size,
        }
    }
}

fn push_builtin_arg(
    function: &mut Function,
    name: &str,
    ty: naga::Handle<Type>,
    builtin: BuiltIn,
) -> u32 {
    let index = function.arguments.len() as u32;
    function.arguments.push(FunctionArgument {
        name: Some(name.to_owned()),
        ty,
        binding: Some(Binding::BuiltIn(builtin)),
    });
    index
}

struct ModuleBuilder {
    module: Module,
    types: TypeHandles,
    bindings: FxHashMap<u32, naga::Handle<GlobalVariable>>,
    binding_types: FxHashMap<u32, naga::Handle<Type>>,
    binding_counts: FxHashMap<u32, Option<u32>>,
    /// Source-level `DataType` per binding slot. WGSL packs every
    /// sub-word scalar (U8/I8/U16/I16/Bool) into `array<u32>` /
    /// `array<i32>` storage; LoadGlobal/LoadShared/LoadConstant on a
    /// byte-element slot needs to honor the byte-addressing the
    /// reference evaluator uses, which means the emitter must extract
    /// the correct lane out of the loaded word. This map lets the body
    /// builder distinguish "real word" from "packed byte" at op-emit
    /// time without losing the IR-level type information that
    /// `binding_types` (the naga handle) collapses.
    binding_data_types: FxHashMap<u32, DataType>,
}

impl ModuleBuilder {
    fn new() -> Self {
        let mut module = Module::default();
        let bool_ty = insert_scalar(&mut module, ScalarKind::Bool, 1);
        let u32_ty = insert_scalar(&mut module, ScalarKind::Uint, 4);
        let i32_ty = insert_scalar(&mut module, ScalarKind::Sint, 4);
        let f32_ty = insert_scalar(&mut module, ScalarKind::Float, 4);
        let f64_ty = insert_scalar(&mut module, ScalarKind::Float, 8);
        let u64_ty = insert_scalar(&mut module, ScalarKind::Uint, 8);
        let i64_ty = insert_scalar(&mut module, ScalarKind::Sint, 8);
        let vec2_u32_ty = module.types.insert(
            Type {
                name: Some("__vyre_vec2_u32".to_owned()),
                inner: TypeInner::Vector {
                    size: VectorSize::Bi,
                    scalar: Scalar {
                        kind: ScalarKind::Uint,
                        width: 4,
                    },
                },
            },
            Span::UNDEFINED,
        );
        let vec3_u32_ty = module.types.insert(
            Type {
                name: Some("__vyre_vec3_u32".to_owned()),
                inner: TypeInner::Vector {
                    size: VectorSize::Tri,
                    scalar: Scalar {
                        kind: ScalarKind::Uint,
                        width: 4,
                    },
                },
            },
            Span::UNDEFINED,
        );
        let vec4_u32_ty = module.types.insert(
            Type {
                name: Some("__vyre_vec4_u32".to_owned()),
                inner: TypeInner::Vector {
                    size: VectorSize::Quad,
                    scalar: Scalar {
                        kind: ScalarKind::Uint,
                        width: 4,
                    },
                },
            },
            Span::UNDEFINED,
        );
        let atomic_compare_exchange_u32_ty = module.types.insert(
            Type {
                name: Some("__atomic_compare_exchange_result_u32".to_owned()),
                inner: TypeInner::Struct {
                    members: vec![
                        naga::StructMember {
                            name: Some("old_value".to_owned()),
                            ty: u32_ty,
                            binding: None,
                            offset: 0,
                        },
                        naga::StructMember {
                            name: Some("exchanged".to_owned()),
                            ty: bool_ty,
                            binding: None,
                            offset: 4,
                        },
                    ],
                    span: 8,
                },
            },
            Span::UNDEFINED,
        );
        module.special_types.predeclared_types.insert(
            naga::PredeclaredType::AtomicCompareExchangeWeakResult(Scalar {
                kind: ScalarKind::Uint,
                width: 4,
            }),
            atomic_compare_exchange_u32_ty,
        );
        Self {
            module,
            types: TypeHandles {
                bool_ty,
                u32_ty,
                i32_ty,
                f32_ty,
                f64_ty,
                u64_ty,
                i64_ty,
                vec2_u32_ty,
                vec3_u32_ty,
                vec4_u32_ty,
                atomic_compare_exchange_u32_ty,
            },
            bindings: FxHashMap::default(),
            binding_types: FxHashMap::default(),
            binding_counts: FxHashMap::default(),
            binding_data_types: FxHashMap::default(),
        }
    }

    fn add_binding(&mut self, binding: &BindingSlot, is_atomic: bool) -> Result<(), EmitError> {
        let scalar_ty = self.scalar_type(&binding.element_type, binding.slot)?;
        // WGSL identifier for this binding's global (and its derived type
        // aliases). For the trap sidecar the wgpu backend owns the name; every
        // other binding keeps its descriptor name. The numeric @group/@binding
        // (not this string) is what connects the global to the descriptor slot.
        let wgsl_name = if binding.name == TRAP_SIDECAR_NAME {
            WGSL_TRAP_SIDECAR_GLOBAL.to_owned()
        } else {
            binding.name.clone()
        };
        let element_ty = if is_atomic {
            let scalar = match &self.module.types[scalar_ty].inner {
                TypeInner::Scalar(s) => *s,
                _ => {
                    return Err(EmitError::InvalidBinding {
                        slot: binding.slot,
                        reason: "atomic-targeted binding must wrap a scalar element type".into(),
                    });
                }
            };
            self.module.types.insert(
                Type {
                    name: Some(format!("{wgsl_name}_atomic")),
                    inner: TypeInner::Atomic(scalar),
                },
                Span::UNDEFINED,
            )
        } else {
            scalar_ty
        };
        let stride = binding.element_type.min_bytes().max(4) as u32;
        let size = match binding.memory_class {
            MemoryClass::Shared => {
                let count = binding
                    .element_count
                    .ok_or_else(|| EmitError::InvalidBinding {
                        slot: binding.slot,
                        reason: "shared bindings require a static element_count".to_owned(),
                    })?;
                ArraySize::Constant(NonZeroU32::new(count).ok_or_else(|| {
                    EmitError::InvalidBinding {
                        slot: binding.slot,
                        reason: "shared bindings require element_count > 0".to_owned(),
                    }
                })?)
            }
            _ => ArraySize::Dynamic,
        };
        let array_ty = self.module.types.insert(
            Type {
                name: Some(format!("{wgsl_name}_elements")),
                inner: TypeInner::Array {
                    base: element_ty,
                    size,
                    stride,
                },
            },
            Span::UNDEFINED,
        );
        let global = self.module.global_variables.append(
            GlobalVariable {
                name: Some(wgsl_name),
                space: address_space(binding),
                binding: resource_binding(binding),
                ty: array_ty,
                init: None,
            },
            Span::UNDEFINED,
        );
        self.bindings.insert(binding.slot, global);
        self.binding_types.insert(binding.slot, element_ty);
        self.binding_counts.insert(
            binding.slot,
            if binding.memory_class == MemoryClass::Shared {
                binding.element_count
            } else {
                None
            },
        );
        self.binding_data_types
            .insert(binding.slot, binding.element_type.clone());
        Ok(())
    }

    fn scalar_type(
        &self,
        data_type: &DataType,
        slot: u32,
    ) -> Result<naga::Handle<Type>, EmitError> {
        match data_type {
            DataType::Bool => Ok(self.types.u32_ty),
            DataType::U8 | DataType::U16 | DataType::U32 => Ok(self.types.u32_ty),
            DataType::I8 | DataType::I16 | DataType::I32 => Ok(self.types.i32_ty),
            DataType::F32 => Ok(self.types.f32_ty),
            // WGSL has no native 64-bit integer: back U64/I64 (and the explicit
            // Vec2U32 pair) with `vec2<u32>` (low word `.x`, high word `.y`).
            // Componentwise bitwise ops stay correct; carry-dependent arithmetic
            // is rejected in emit_binop until a carry-propagating pass lands.
            DataType::U64 | DataType::I64 | DataType::Vec2U32 => Ok(self.types.vec2_u32_ty),
            DataType::Vec4U32 => Ok(self.types.vec4_u32_ty),
            // `Bytes` is NOT a 4-byte word: lowering it as `array<u32>` would
            // silently reinterpret packed bytes as words (a recall/correctness
            // hazard). Require the explicit pack-to-u32 pre-pass instead.
            DataType::Bytes => Err(EmitError::InvalidBinding {
                slot,
                reason: "raw `Bytes` buffers must run the pack-to-u32 pre-pass before Naga \
                         emission; lowering them directly as `array<u32>` would reinterpret \
                         packed bytes as words. Fix: run the byte-packing pre-pass."
                    .to_owned(),
            }),
            DataType::F16 => Err(EmitError::InvalidBinding {
                slot,
                reason: "`F16` buffers need WGSL `enable f16;`, which this Naga stack cannot \
                         parse yet. Fix: keep half data packed as u32 until the f16 extension \
                         is wired."
                    .to_owned(),
            }),
            // A 4-byte array element is just a u32 word; any other element size
            // needs a struct-backed array (WGSL `array<u32>` cannot represent a
            // non-word stride without silently truncating).
            DataType::Array { element_size } if *element_size == 4 => Ok(self.types.u32_ty),
            DataType::Array { element_size } => Err(EmitError::InvalidBinding {
                slot,
                reason: format!(
                    "array element size {element_size} is not a 4-byte word and needs a \
                     struct-backed array lowering, not `array<u32>`. Fix: add struct-backed \
                     array emission or pack the elements to 4-byte words."
                ),
            }),
            other => Err(EmitError::InvalidBinding {
                slot,
                reason: format!(
                    "data type `{other:?}` is not representable by the scalar Naga emitter"
                ),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

    #[test]
    fn storage_buffer_len_emits_runtime_array_length_for_counted_bindings() {
        let program = Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(8),
                BufferDecl::storage("out", 1, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::buf_len("input"))],
        );
        let descriptor =
            vyre_lower::lower(&program).expect("Fix: counted storage buf_len program must lower");
        let module =
            emit_uncached(&descriptor).expect("Fix: counted storage buf_len descriptor must emit");
        let function = &module.entry_points[0].function;

        assert!(
            function
                .expressions
                .iter()
                .any(|(_, expr)| matches!(expr, naga::Expression::ArrayLength(_))),
            "Fix: storage BufferLength must emit runtime arrayLength so WGSL cache keys can ignore per-dispatch storage counts without reusing stale static bounds."
        );
    }
}

fn insert_scalar(module: &mut Module, kind: ScalarKind, width: u8) -> naga::Handle<Type> {
    module.types.insert(
        Type {
            name: None,
            inner: TypeInner::Scalar(Scalar { kind, width }),
        },
        Span::UNDEFINED,
    )
}

fn address_space(binding: &BindingSlot) -> AddressSpace {
    match binding.memory_class {
        MemoryClass::Shared => AddressSpace::WorkGroup,
        // True `var<uniform>` would require 16-byte (vec4) stride
        // for any inner array  -  `array<u32>` alone fails Naga's
        // alignment validation. Keep the WGSL address space as
        // storage(LOAD); `resource_binding` still routes uniforms
        // to group 1 so the layout-builder side is unambiguous.
        // When the lowering grows a packed-vec4 uniform variant, swap
        // this arm back to `AddressSpace::Uniform`.
        MemoryClass::Uniform | MemoryClass::Constant => AddressSpace::Storage {
            access: StorageAccess::LOAD,
        },
        MemoryClass::Global | MemoryClass::Scratch => AddressSpace::Storage {
            access: storage_access(binding.visibility),
        },
    }
}

fn storage_access(visibility: BindingVisibility) -> StorageAccess {
    match visibility {
        BindingVisibility::ReadOnly => StorageAccess::LOAD,
        // WGSL/Naga reject storage-buffer declarations with store-only access.
        // Preserve the IR's write-only ownership at the binding-plan layer, but
        // emit a validator-compatible storage address space for shader modules.
        BindingVisibility::WriteOnly => StorageAccess::LOAD | StorageAccess::STORE,
        BindingVisibility::ReadWrite => StorageAccess::LOAD | StorageAccess::STORE,
    }
}

fn resource_binding(binding: &BindingSlot) -> Option<ResourceBinding> {
    match binding.memory_class {
        MemoryClass::Shared | MemoryClass::Scratch => None,
        MemoryClass::Uniform => Some(ResourceBinding {
            group: 1,
            binding: binding.slot,
        }),
        MemoryClass::Global | MemoryClass::Constant => Some(ResourceBinding {
            group: 0,
            binding: binding.slot,
        }),
    }
}

fn body_uses_subgroup(body: &KernelBody) -> bool {
    body.ops.iter().any(|op| {
        matches!(
            op.kind,
            KernelOpKind::SubgroupBallot
                | KernelOpKind::SubgroupShuffle
                | KernelOpKind::SubgroupBroadcast
                | KernelOpKind::SubgroupReduce { .. }
                | KernelOpKind::SubgroupLocalId
                | KernelOpKind::SubgroupSize
        )
    }) || body.child_bodies.iter().any(body_uses_subgroup)
}

fn body_uses_trap(body: &KernelBody) -> bool {
    body.ops
        .iter()
        .any(|op| matches!(op.kind, KernelOpKind::Trap { .. }))
        || body.child_bodies.iter().any(body_uses_trap)
}

/// Walk the descriptor body and return the set of binding slots that
/// are accessed by an `Atomic` op or by trap sidecar emission. Naga's
/// validator requires the array element to be `atomic<u32>` (not just
/// `u32`) for any binding that is the target of an atomic operation;
/// otherwise it rejects the module with `InvalidAtomic(InvalidPointer)`.
fn collect_atomic_binding_slots(desc: &KernelDescriptor) -> FxHashSet<u32> {
    use FxHashSet;

    fn walk(body: &KernelBody, out: &mut FxHashSet<u32>) {
        for op in &body.ops {
            if matches!(op.kind, KernelOpKind::Atomic { .. }) {
                if let Some(&slot) = op.operands.first() {
                    out.insert(slot);
                }
            }
        }
        for child in &body.child_bodies {
            walk(child, out);
        }
    }
    let mut out = FxHashSet::default();
    walk(&desc.body, &mut out);
    if body_uses_trap(&desc.body) {
        if let Some(slot) = desc
            .bindings
            .slots
            .iter()
            .find(|b| b.name == TRAP_SIDECAR_NAME)
            .map(|b| b.slot)
        {
            out.insert(slot);
        }
    }
    out
}

fn descriptor_trap_sidecar_slot(desc: &KernelDescriptor) -> Result<Option<u32>, EmitError> {
    if !body_uses_trap(&desc.body) {
        return Ok(None);
    }
    let slot = desc
        .bindings
        .slots
        .iter()
        .find(|binding| binding.name == TRAP_SIDECAR_NAME)
        .ok_or_else(|| {
            EmitError::InvalidDescriptor(format!(
                "Trap op requires descriptor binding `{TRAP_SIDECAR_NAME}`. Fix: lower through vyre-lower so the trap sidecar binding is inserted."
            ))
        })?;
    if slot.element_type != DataType::U32
        || slot.element_count.unwrap_or(0) < vyre_lower::TRAP_SIDECAR_WORDS
        || !matches!(slot.visibility, BindingVisibility::ReadWrite)
    {
        return Err(EmitError::InvalidBinding {
            slot: slot.slot,
            reason: format!(
                "trap sidecar `{TRAP_SIDECAR_NAME}` must be a read-write u32 buffer with at least {} words",
                vyre_lower::TRAP_SIDECAR_WORDS
            ),
        });
    }
    Ok(Some(slot.slot))
}

fn descriptor_trap_tag_codes(body: &KernelBody) -> FxHashMap<vyre_lower::descriptor::Name, u32> {
    fn walk(
        body: &KernelBody,
        tags: &mut FxHashMap<vyre_lower::descriptor::Name, u32>,
        next: &mut u32,
    ) {
        for op in &body.ops {
            if let KernelOpKind::Trap { tag } = &op.kind {
                tags.entry(tag.clone()).or_insert_with(|| {
                    let code = *next;
                    *next = next.saturating_add(1);
                    code
                });
            }
        }
        for child in &body.child_bodies {
            walk(child, tags, next);
        }
    }
    let mut tags = FxHashMap::default();
    let mut next = 1;
    walk(body, &mut tags, &mut next);
    tags
}

pub(crate) fn emit_uncached(desc: &KernelDescriptor) -> Result<naga::Module, EmitError> {
    let mut builder = ModuleBuilder::new();
    let atomic_slots = collect_atomic_binding_slots(desc);
    for binding in &desc.bindings.slots {
        builder.add_binding(binding, atomic_slots.contains(&binding.slot))?;
    }
    let trap_sidecar_slot = descriptor_trap_sidecar_slot(desc)?;
    let trap_tag_codes = descriptor_trap_tag_codes(&desc.body);

    let mut function = Function::default();
    function.name = Some("main".to_owned());
    let builtins = Builtins::push(&mut function, builder.types, body_uses_subgroup(&desc.body));
    let mut body_builder = BodyBuilder {
        function: &mut function,
        values: FxHashMap::default(),
        value_types: FxHashMap::default(),
        globals: &builder.bindings,
        binding_types: &builder.binding_types,
        binding_counts: &builder.binding_counts,
        binding_data_types: &builder.binding_data_types,
        builtins,
        types: builder.types,
        loop_locals: FxHashMap::default(),
        loop_types: FxHashMap::default(),
        loop_carrier_targets: FxHashSet::default(),
        loop_carrier_locals: FxHashMap::default(),
        child_body_depth: 0,
        block_scoped_locals: FxHashMap::default(),
        named_carrier_locals: FxHashMap::default(),
        named_carrier_types: FxHashMap::default(),
        named_carrier_result_ids: FxHashMap::default(),
        trap_sidecar_slot,
        trap_tag_codes,
        op_dispatch_routes: Default::default(),
    };
    body_builder.emit_body(&desc.body)?;

    builder.module.entry_points.push(EntryPoint {
        name: "main".to_owned(),
        stage: ShaderStage::Compute,
        early_depth_test: None,
        workgroup_size: desc.dispatch.workgroup_size,
        workgroup_size_overrides: None,
        function,
    });

    Ok(builder.module)
}

// `lib.rs` calls into the cache layer first, then `emit_uncached` here.
// Re-export the cache wrapper from this module so the `crate::emit`
// boundary stays unchanged.
