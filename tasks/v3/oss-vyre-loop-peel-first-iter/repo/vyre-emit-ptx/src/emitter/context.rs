use std::fmt::Write as _;

use rustc_hash::{FxHashMap, FxHashSet};
use vyre_lower::{
    BindingLayout, BindingSlot, KernelDescriptor, KernelOpKind, MemoryClass, TRAP_SIDECAR_NAME,
};

use super::names::sanitize_param_name;
use super::BodyCtx;
use crate::reg::{PtxType, Reg};
use crate::{EmitError, PtxEmitOptions};

impl<'a> BodyCtx<'a> {
    pub(super) fn new(
        desc: &'a KernelDescriptor,
        bindings: &'a BindingLayout,
        options: PtxEmitOptions,
        read_only_cache_slots: FxHashSet<u32>,
        text_capacity: usize,
        op_capacity: usize,
    ) -> Self {
        let slot_count = bindings.slots.len();
        let full_workgroup_entry = requires_full_workgroup_entry(desc);
        let slot_to_binding = bindings
            .slots
            .iter()
            .map(|binding| (binding.slot, binding))
            .collect::<FxHashMap<_, _>>();
        let mut this = Self {
            options,
            text: String::with_capacity(text_capacity),
            next_pred: 1,
            next_b16: 0,
            next_u32: 27,
            next_i32: 0,
            next_f32: 0,
            next_u64: 1,
            next_label: 0,
            operand_to_reg: FxHashMap::with_capacity_and_hasher(op_capacity, Default::default()),
            u32_literals: FxHashMap::with_capacity_and_hasher(op_capacity / 4, Default::default()),
            slot_to_ptr: FxHashMap::with_capacity_and_hasher(slot_count, Default::default()),
            slot_to_shared_symbol: FxHashMap::with_capacity_and_hasher(
                slot_count,
                Default::default(),
            ),
            read_only_cache_slots,
            pending_cp_async_tags: FxHashSet::with_capacity_and_hasher(4, Default::default()),
            loop_indices: FxHashMap::with_capacity_and_hasher(8, Default::default()),
            named_carriers: FxHashMap::with_capacity_and_hasher(16, Default::default()),
            named_carrier_result_ids: FxHashMap::with_capacity_and_hasher(16, Default::default()),
            slot_to_length_reg: FxHashMap::with_capacity_and_hasher(slot_count, Default::default()),
            slot_to_binding,
            full_workgroup_entry,
            grid_barrier_index: 0,
        };
        this.emit_thread_geometry();
        this
    }

    fn emit_thread_geometry(&mut self) {
        self.text.push_str("    // global_invocation_id\n");
        self.text.push_str("    mov.u32 %r0, %ctaid.x;\n");
        self.text.push_str("    mov.u32 %r1, %ntid.x;\n");
        self.text.push_str("    mov.u32 %r2, %tid.x;\n");
        self.text.push_str("    mad.lo.u32 %r3, %r0, %r1, %r2;\n");
        self.text.push_str("    mov.u32 %r4, %ctaid.y;\n");
        self.text.push_str("    mov.u32 %r5, %ntid.y;\n");
        self.text.push_str("    mov.u32 %r6, %tid.y;\n");
        self.text.push_str("    mad.lo.u32 %r7, %r4, %r5, %r6;\n");
        self.text.push_str("    mov.u32 %r8, %ctaid.z;\n");
        self.text.push_str("    mov.u32 %r9, %ntid.z;\n");
        self.text.push_str("    mov.u32 %r24, %tid.z;\n");
        self.text
            .push_str("    mad.lo.u32 %r25, %r8, %r9, %r24;\n\n");
    }

    pub(super) fn alloc_label(&mut self, prefix: &str) -> String {
        let id = self.next_label;
        self.next_label += 1;
        format!("$L_{prefix}_{id}")
    }

    pub(super) fn alloc(&mut self, ty: PtxType) -> Reg {
        let id = match ty {
            PtxType::B16 => {
                let i = self.next_b16;
                self.next_b16 += 1;
                i
            }
            PtxType::Bool => {
                let i = self.next_pred;
                self.next_pred += 1;
                i
            }
            PtxType::U32 => {
                let i = self.next_u32;
                self.next_u32 += 1;
                i
            }
            PtxType::I32 => {
                let i = self.next_i32;
                self.next_i32 += 1;
                i
            }
            PtxType::F32 => {
                let i = self.next_f32;
                self.next_f32 += 1;
                i
            }
            PtxType::U64 => {
                let i = self.next_u64;
                self.next_u64 += 1;
                i
            }
        };
        Reg(ty, id)
    }

    pub(super) fn preload_bindings(&mut self, desc: &KernelDescriptor) -> Result<(), EmitError> {
        for binding in &desc.bindings.slots {
            if matches!(binding.memory_class, MemoryClass::Shared) {
                let element_count =
                    binding
                        .element_count
                        .ok_or_else(|| EmitError::InvalidBinding {
                            slot: binding.slot,
                            reason: "shared bindings need a fixed element_count for PTX allocation"
                                .into(),
                        })?;
                let byte_len = element_count
                    .checked_mul(binding.element_type.size_bytes().unwrap_or(0) as u32)
                    .filter(|bytes| *bytes > 0)
                    .ok_or_else(|| EmitError::InvalidBinding {
                        slot: binding.slot,
                        reason: "shared binding byte length overflowed or used an unsized type"
                            .into(),
                    })?;
                let symbol = format!("shared_buf_{}", binding.slot);
                let _ = writeln!(self.text, "    .shared .align 4 .b8 {symbol}[{byte_len}];");
                self.slot_to_shared_symbol.insert(binding.slot, symbol);
                continue;
            }
            if binding.name == TRAP_SIDECAR_NAME {
                continue;
            }
            if matches!(binding.memory_class, MemoryClass::Scratch) {
                return Err(EmitError::InvalidBinding {
                    slot: binding.slot,
                    reason: "scratch bindings must be resolved before PTX emission".into(),
                });
            }
            let param_reg = self.alloc(PtxType::U64);
            let global_reg = self.alloc(PtxType::U64);
            let _ = writeln!(
                self.text,
                "    ld.param.u64    {param_reg}, [_arg_{}];",
                sanitize_param_name(&binding.name, binding.slot)
            );
            let _ = writeln!(
                self.text,
                "    cvta.to.global.u64 {global_reg}, {param_reg};"
            );
            self.slot_to_ptr.insert(binding.slot, global_reg);
        }
        self.text
            .push_str("    // Load params metadata (element_count = first u32)\n");
        self.text
            .push_str("    ld.param.u64    %rd0, [params_buf];\n");
        for binding in &desc.bindings.slots {
            if matches!(binding.memory_class, MemoryClass::Shared)
                || binding.name == TRAP_SIDECAR_NAME
            {
                continue;
            }
            let len_reg = self.alloc(PtxType::U32);
            // Checked arithmetic: slot * 4 + 4 is the byte offset into the
            // params-buffer header for this binding's element-count word.
            // A slot number large enough to overflow u32 here would produce a
            // wrapped address and silently read the wrong length register,
            // causing every subsequent bounds-check predicate to be wrong.
            let byte_offset = binding
                .slot
                .checked_mul(4)
                .and_then(|v| v.checked_add(4))
                .ok_or_else(|| EmitError::InvalidBinding {
                    slot: binding.slot,
                    reason: "slot byte offset (4 + slot * 4) overflows u32".into(),
                })?;
            let _ = writeln!(
                self.text,
                "    ld.global.ca.u32    {len_reg}, [%rd0 + {byte_offset}];"
            );
            self.slot_to_length_reg.insert(binding.slot, len_reg);
        }
        if self.full_workgroup_entry {
            self.text.push_str(
                "    // Full-workgroup entry: keep every lane live before barriers/shared memory.\n\n",
            );
        } else {
            self.text.push_str("    ld.global.ca.u32   %r26, [%rd0];\n");
            self.text.push_str("    setp.ge.u32     %p0, %r3, %r26;\n");
            self.text.push_str("    @%p0 bra $L_exit;\n\n");
        }
        Ok(())
    }

    pub(super) fn binding_for_slot(&self, slot: u32) -> Result<&BindingSlot, EmitError> {
        self.slot_to_binding
            .get(&slot)
            .copied()
            .ok_or_else(|| EmitError::InvalidBinding {
                slot,
                reason: "binding not declared".into(),
            })
    }

    pub(super) fn load_space_for(
        &self,
        binding_slot: u32,
        memory_class: MemoryClass,
    ) -> &'static str {
        match memory_class {
            MemoryClass::Global if self.read_only_cache_slots.contains(&binding_slot) => {
                "global.nc"
            }
            MemoryClass::Global | MemoryClass::Constant | MemoryClass::Uniform => "global",
            MemoryClass::Shared => "shared",
            MemoryClass::Scratch => "global",
        }
    }
}

fn requires_full_workgroup_entry(desc: &KernelDescriptor) -> bool {
    desc.bindings
        .slots
        .iter()
        .any(|binding| matches!(binding.memory_class, MemoryClass::Shared))
        || desc
            .ops_iter()
            .any(|op| matches!(op.kind, KernelOpKind::Barrier { .. }))
}
