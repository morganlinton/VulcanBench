//! Lower a `vyre_foundation::Program` into a substrate-neutral
//! `KernelDescriptor`.
//!
//! This module is the shared boundary between high-level vyre IR and
//! emitter input. It preserves supported IR semantics in descriptor
//! form and returns an explicit [`LowerError`] when the input is
//! invalid.

use crate::descriptor::{
    BindingLayout, BindingSlot, BindingVisibility, Dispatch, KernelBody, KernelDescriptor,
    KernelOp, KernelOpKind, LiteralValue, MemoryClass, OpaqueExprData, OpaqueNodeData,
    TRAP_SIDECAR_NAME, TRAP_SIDECAR_WORDS,
};
use crate::error::LowerError;
mod scope;
use rustc_hash::{FxHashMap, FxHashSet};
use scope::VarScope;
use std::sync::Arc;
use vyre_foundation::ir::model::node::node_op_id;
use vyre_foundation::ir::{
    AtomicOp, BinOp, BufferAccess, BufferDecl, DataType, Expr, Ident, MemoryKind, Node, Program,
    SubgroupReduceOp,
};

/// Maximum nested-body depth before lowering refuses with
/// `LowerError::NestingTooDeep`. 64 levels is generous; real programs
/// rarely exceed 10.
const MAX_NESTING_DEPTH: usize = 64;

/// First slot value reserved for `MemoryClass::Shared` / `MemoryClass::Scratch`
/// bindings. Host-bound bindings (`Global`/`Constant`/`Uniform`) use slots
/// 0..WORKGROUP_SLOT_BASE so that backend bind-group layouts (capped at 1000
/// bindings on wgpu) never see a Shared slot. Any rewrite that allocates
/// new Shared/Scratch bindings must seed its `next_slot` cursor at or above
/// this constant to avoid colliding with host slots in `BindingLayout.slots`.
pub(crate) const WORKGROUP_SLOT_BASE: u32 = 1 << 24;

/// Lower a vyre Program to the substrate-neutral kernel descriptor.
///
/// # Errors
///
/// Returns [`LowerError`] when the input references undeclared buffers,
/// exceeds the supported structured nesting depth, or uses an IR
/// construct with invalid operands.
pub fn lower(program: &Program) -> Result<KernelDescriptor, LowerError> {
    let mut ctx = LowerCtx::new(program)?;
    let mut body = empty_body_with_capacity(estimated_root_op_capacity(program));
    ctx.lower_nodes(program.entry(), &mut body, 0)?;
    if body_contains_trap(&body) {
        ctx.add_trap_sidecar_binding()?;
    }

    Ok(KernelDescriptor {
        id: fingerprint_id(program),
        bindings: BindingLayout {
            slots: ctx.bindings,
        },
        dispatch: Dispatch {
            workgroup_size: program.workgroup_size(),
        },
        body,
    })
}

struct LowerCtx {
    bindings: Vec<BindingSlot>,
    buffer_slots: FxHashMap<Ident, u32>,
    slot_memory_classes: FxHashMap<u32, MemoryClass>,
    scope: VarScope,
    next_value: u32,
    /// Stack of "currently active loop carriers"  -  one frame per
    /// enclosing `Node::Loop` we are inside of. An `Assign(name, ..)`
    /// whose `name` is in any active frame commits its new value
    /// directly to the function-local via `LoopCarrierEnd` and then
    /// re-reads via `LoopCarrier`, bypassing the if-then phi-merge
    /// path. The Select-based merge cannot represent the per-iteration
    /// state correctly because the carrier's authoritative storage
    /// lives in the function-local, not in any SSA value.
    active_carriers: Vec<FxHashSet<Ident>>,
}

impl LowerCtx {
    fn new(program: &Program) -> Result<Self, LowerError> {
        let mut bindings = Vec::with_capacity(program.buffers().len());
        let mut buffer_slots = FxHashMap::default();
        let mut slot_memory_classes = FxHashMap::default();
        // Soundness: split slot allocation by memory class. Host-bound
        // buffers (Global, Constant) keep their declared binding ids
        // because the dispatch path looks them up by the same
        // BufferDecl::binding() value. Workgroup/Scratch buffers are
        // SM-local  -  they don't get bound by the host  -  so they live
        // in a high range starting at WORKGROUP_SLOT_BASE that cannot
        // collide with host-bound slots. Without this split,
        // multiple `BufferDecl::workgroup(...)` calls (which all
        // default to binding=0) collided with the host-bound input
        // and forced the output's slot to be auto-renumbered, which
        // then broke the dispatch path's slot-id-keyed lookup.
        let mut host_used_slots = FxHashSet::default();
        let mut host_next_free_slot = 0u32;
        let mut shared_next_slot = WORKGROUP_SLOT_BASE;
        for buffer in program.buffers() {
            let mc = memory_class(buffer)?;
            let slot = match mc {
                MemoryClass::Shared | MemoryClass::Scratch => {
                    let s = shared_next_slot;
                    shared_next_slot = shared_next_slot
                        .checked_add(1)
                        .ok_or(LowerError::OperandIdOverflow)?;
                    s
                }
                MemoryClass::Global | MemoryClass::Constant | MemoryClass::Uniform => {
                    let requested = buffer.binding();
                    let s = if host_used_slots.insert(requested) {
                        requested
                    } else {
                        while host_used_slots.contains(&host_next_free_slot)
                            || host_next_free_slot >= WORKGROUP_SLOT_BASE
                        {
                            host_next_free_slot = host_next_free_slot
                                .checked_add(1)
                                .ok_or(LowerError::OperandIdOverflow)?;
                        }
                        host_used_slots.insert(host_next_free_slot);
                        host_next_free_slot
                    };
                    while host_used_slots.contains(&host_next_free_slot)
                        || host_next_free_slot >= WORKGROUP_SLOT_BASE
                    {
                        host_next_free_slot = host_next_free_slot
                            .checked_add(1)
                            .ok_or(LowerError::OperandIdOverflow)?;
                    }
                    s
                }
            };
            buffer_slots.insert(Ident::from(Arc::clone(&buffer.name)), slot);
            slot_memory_classes.insert(slot, mc);
            bindings.push(BindingSlot {
                slot,
                element_type: buffer.element.clone(),
                element_count: (buffer.count != 0).then_some(buffer.count),
                memory_class: mc,
                visibility: binding_visibility(&buffer.access),
                name: buffer.name().to_owned(),
            });
        }
        bindings.sort_by_key(|slot| slot.slot);
        Ok(Self {
            bindings,
            buffer_slots,
            slot_memory_classes,
            scope: VarScope::default(),
            next_value: 0,
            active_carriers: Vec::new(),
        })
    }

    fn is_active_carrier(&self, name: &Ident) -> bool {
        self.active_carriers
            .iter()
            .any(|frame| frame.contains(name))
    }

    fn lower_nodes(
        &mut self,
        nodes: &[Node],
        body: &mut KernelBody,
        depth: usize,
    ) -> Result<(), LowerError> {
        if depth > MAX_NESTING_DEPTH {
            return Err(LowerError::NestingTooDeep(depth));
        }
        for node in nodes {
            self.lower_node(node, body, depth)?;
        }
        Ok(())
    }

    fn lower_node(
        &mut self,
        node: &Node,
        body: &mut KernelBody,
        depth: usize,
    ) -> Result<(), LowerError> {
        match node {
            Node::Region {
                generator,
                body: region,
                ..
            } => self.lower_child_node(
                body,
                depth,
                region.as_ref(),
                KernelOpKind::Region {
                    generator: generator.shared_text(),
                },
            ),
            Node::Block(region) => {
                self.lower_child_node(body, depth, region, KernelOpKind::StructuredBlock)
            }
            Node::Let { name, value } => {
                let id = self.lower_expr(value, body)?;
                let id = if let Expr::Var(source) = value {
                    if self.is_active_carrier(source) {
                        self.copy_value(body, id)?
                    } else {
                        id
                    }
                } else {
                    id
                };
                self.scope.bind(name.clone(), id);
                Ok(())
            }
            Node::Assign { name, value } => {
                let id = self.lower_expr(value, body)?;
                if self.is_active_carrier(name) {
                    // Assign of an active loop carrier: commit the new
                    // value to the function-local via LoopCarrierEnd,
                    // then re-read so subsequent in-scope references
                    // pick up a fresh SSA id sourced from the local.
                    // This bypasses if-then phi-merge for carrier vars
                    // because the merge's seed-vs-then Select cannot
                    // represent the per-iteration state  -  the
                    // authoritative store is the function-local.
                    body.ops.push(KernelOp {
                        kind: KernelOpKind::LoopCarrierEnd {
                            name: name.shared_text(),
                        },
                        operands: vec![id],
                        result: None,
                    });
                    let read_id = self.alloc_value()?;
                    body.ops.push(KernelOp {
                        kind: KernelOpKind::LoopCarrier {
                            name: name.shared_text(),
                        },
                        operands: Vec::new(),
                        result: Some(read_id),
                    });
                    self.scope.bind(name.clone(), read_id);
                } else {
                    self.scope.bind(name.clone(), id);
                }
                Ok(())
            }
            Node::Store {
                buffer,
                index,
                value,
            } => {
                let slot = self.buffer_slot(buffer)?;
                let index_id = self.lower_expr(index, body)?;
                let value_id = self.lower_expr(value, body)?;
                body.ops.push(KernelOp {
                    kind: self.store_kind(slot, buffer)?,
                    operands: vec![slot, index_id, value_id],
                    result: None,
                });
                Ok(())
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                let cond_id = self.lower_expr(cond, body)?;
                let incoming_scope = self.scope.snapshot();
                let mut if_carriers = collect_carrier_names(then, &incoming_scope, None);
                for name in collect_carrier_names(otherwise, &incoming_scope, None) {
                    if !if_carriers.contains(&name) {
                        if_carriers.push(name);
                    }
                }
                for name in &if_carriers {
                    let seed_id = incoming_scope
                        .get(name)
                        .copied()
                        .unwrap_or_else(|| unreachable!("if carrier must have incoming binding"));
                    body.ops.push(KernelOp {
                        kind: KernelOpKind::LoopCarrierInit {
                            name: name.shared_text(),
                        },
                        operands: vec![seed_id],
                        result: None,
                    });
                }
                let mut then_body = empty_body_for_nodes(then);
                self.scope.restore(incoming_scope.clone());
                for name in &if_carriers {
                    let read_id = self.alloc_value()?;
                    then_body.ops.push(KernelOp {
                        kind: KernelOpKind::LoopCarrier {
                            name: name.shared_text(),
                        },
                        operands: Vec::new(),
                        result: Some(read_id),
                    });
                    self.scope.bind(name.clone(), read_id);
                }
                let mut carrier_frame: FxHashSet<Ident> = FxHashSet::default();
                for name in &if_carriers {
                    carrier_frame.insert(name.clone());
                }
                self.active_carriers.push(carrier_frame);
                self.lower_nodes(then, &mut then_body, depth + 1)?;
                self.active_carriers.pop();
                let then_id = push_child(body, then_body)?;
                if otherwise.is_empty() {
                    self.scope.restore(incoming_scope.clone());
                    body.ops.push(KernelOp {
                        kind: KernelOpKind::StructuredIfThen,
                        operands: vec![cond_id, then_id],
                        result: None,
                    });
                } else {
                    let mut else_body = empty_body_for_nodes(otherwise);
                    self.scope.restore(incoming_scope.clone());
                    for name in &if_carriers {
                        let read_id = self.alloc_value()?;
                        else_body.ops.push(KernelOp {
                            kind: KernelOpKind::LoopCarrier {
                                name: name.shared_text(),
                            },
                            operands: Vec::new(),
                            result: Some(read_id),
                        });
                        self.scope.bind(name.clone(), read_id);
                    }
                    let mut carrier_frame: FxHashSet<Ident> = FxHashSet::default();
                    for name in &if_carriers {
                        carrier_frame.insert(name.clone());
                    }
                    self.active_carriers.push(carrier_frame);
                    self.lower_nodes(otherwise, &mut else_body, depth + 1)?;
                    self.active_carriers.pop();
                    let else_id = push_child(body, else_body)?;
                    self.scope.restore(incoming_scope.clone());
                    body.ops.push(KernelOp {
                        kind: KernelOpKind::StructuredIfThenElse,
                        operands: vec![cond_id, then_id, else_id],
                        result: None,
                    });
                }
                for name in &if_carriers {
                    self.emit_loop_carrier_read(body, name)?;
                }
                Ok(())
            }
            Node::Loop {
                var,
                from,
                to,
                body: loop_body,
            } => {
                let from_id = self.lower_expr(from, body)?;
                let to_id = self.lower_expr(to, body)?;
                let incoming_scope = self.scope.snapshot();

                // Identify source-level variables that are reassigned inside
                // the loop body AND were already bound in the incoming scope.
                // These are the loop carriers  -  their per-iteration value
                // must round-trip through a function-local because the SSA
                // operand of in-body reads is baked at lowering time and
                // would otherwise stay anchored to the pre-loop seed,
                // making `Assign` inside a loop have no observable effect
                // across iterations.
                let carrier_names =
                    collect_carrier_names(loop_body, &incoming_scope, Some(var));

                // Pre-loop init: in the parent body, store the seed value
                // into each carrier slot.
                for name in &carrier_names {
                    let seed_id = incoming_scope
                        .get(name)
                        .copied()
                        .unwrap_or_else(|| unreachable!("carrier name has incoming binding"));
                    body.ops.push(KernelOp {
                        kind: KernelOpKind::LoopCarrierInit {
                            name: name.shared_text(),
                        },
                        operands: vec![seed_id],
                        result: None,
                    });
                }

                let mut child = empty_body_for_nodes(loop_body);
                let loop_index_id = self.alloc_value()?;
                child.ops.push(KernelOp {
                    kind: KernelOpKind::LoopIndex {
                        loop_var: var.shared_text(),
                    },
                    operands: Vec::new(),
                    result: Some(loop_index_id),
                });
                self.scope.bind(var.clone(), loop_index_id);

                // First op of each iteration: re-read the carrier slot so
                // every in-body reference to the source-level variable
                // resolves to the latest value committed by the previous
                // iteration (or the pre-loop seed on iteration 0).
                for name in &carrier_names {
                    let read_id = self.alloc_value()?;
                    child.ops.push(KernelOp {
                        kind: KernelOpKind::LoopCarrier {
                            name: name.shared_text(),
                        },
                        operands: Vec::new(),
                        result: Some(read_id),
                    });
                    self.scope.bind(name.clone(), read_id);
                }

                let mut carrier_frame: FxHashSet<Ident> = FxHashSet::default();
                for name in &carrier_names {
                    carrier_frame.insert(name.clone());
                }
                self.active_carriers.push(carrier_frame);
                self.lower_nodes(loop_body, &mut child, depth + 1)?;
                self.active_carriers.pop();
                let loop_exit_scope = self.scope.snapshot();

                self.scope
                    .restore_loop_exit(incoming_scope, &loop_exit_scope, var);
                let child_id = push_child(body, child)?;
                body.ops.push(KernelOp {
                    kind: KernelOpKind::StructuredForLoop {
                        loop_var: var.shared_text(),
                    },
                    operands: vec![from_id, to_id, child_id],
                    result: None,
                });

                // Post-loop: emit a fresh LoopCarrier read in the parent
                // so post-loop references to each carrier name resolve to
                // the loop's final stored value. Rebind in scope so
                // `Var(name)` reads downstream resolve to this id rather
                // than the pre-loop seed.
                for name in &carrier_names {
                    let post_id = self.alloc_value()?;
                    body.ops.push(KernelOp {
                        kind: KernelOpKind::LoopCarrier {
                            name: name.shared_text(),
                        },
                        operands: Vec::new(),
                        result: Some(post_id),
                    });
                    self.scope.bind(name.clone(), post_id);
                }
                Ok(())
            }
            Node::Barrier { ordering } => {
                body.ops.push(KernelOp {
                    kind: KernelOpKind::Barrier {
                        ordering: *ordering,
                    },
                    operands: Vec::new(),
                    result: None,
                });
                Ok(())
            }
            Node::IndirectDispatch {
                count_buffer,
                count_offset,
            } => {
                let slot = self.buffer_slot(count_buffer)?;
                body.ops.push(KernelOp {
                    kind: KernelOpKind::IndirectDispatch {
                        count_offset: *count_offset,
                    },
                    operands: vec![slot],
                    result: None,
                });
                Ok(())
            }
            Node::AsyncLoad {
                source,
                destination,
                offset,
                size,
                tag,
            } => self.lower_async_copy(
                body,
                KernelOpKind::AsyncLoad {
                    tag: tag.shared_text(),
                },
                source,
                destination,
                offset,
                size,
            ),
            Node::AsyncStore {
                source,
                destination,
                offset,
                size,
                tag,
            } => self.lower_async_copy(
                body,
                KernelOpKind::AsyncStore {
                    tag: tag.shared_text(),
                },
                source,
                destination,
                offset,
                size,
            ),
            Node::AsyncWait { tag } => {
                body.ops.push(KernelOp {
                    kind: KernelOpKind::AsyncWait {
                        tag: tag.shared_text(),
                    },
                    operands: Vec::new(),
                    result: None,
                });
                Ok(())
            }
            Node::Trap { address, tag } => {
                let address_id = self.lower_expr(address, body)?;
                body.ops.push(KernelOp {
                    kind: KernelOpKind::Trap {
                        tag: tag.shared_text(),
                    },
                    operands: vec![address_id],
                    result: None,
                });
                Ok(())
            }
            Node::Resume { tag } => {
                body.ops.push(KernelOp {
                    kind: KernelOpKind::Resume {
                        tag: tag.shared_text(),
                    },
                    operands: Vec::new(),
                    result: None,
                });
                Ok(())
            }
            Node::Return => {
                body.ops.push(KernelOp {
                    kind: KernelOpKind::Return,
                    operands: Vec::new(),
                    result: None,
                });
                Ok(())
            }
            Node::Opaque(extension) => {
                body.ops.push(KernelOp {
                    kind: KernelOpKind::OpaqueNode(Box::new(OpaqueNodeData {
                        extension_kind: extension.extension_kind().to_owned(),
                        payload: extension.wire_payload(),
                    })),
                    operands: Vec::new(),
                    result: None,
                });
                Ok(())
            }
            other => Err(LowerError::UnsupportedConstruct(format!(
                "node `{}` has no KernelDescriptor lowering. Fix: add a KernelOpKind mapping before routing this program through vyre-lower.",
                node_op_id(other)
            ))),
        }
    }

    fn lower_child_node(
        &mut self,
        body: &mut KernelBody,
        depth: usize,
        nodes: &[Node],
        kind: KernelOpKind,
    ) -> Result<(), LowerError> {
        let incoming_scope = self.scope.snapshot();

        // Names reassigned inside the region whose pre-region binding lives
        // in the enclosing scope. The parent body cannot reference an SSA
        // id emitted inside the child KernelBody (Naga's `Statement::Block`
        // closes the inner scope), so reassignments must round-trip through
        // a function-local. Reuses the same `LoopCarrierInit/LoopCarrier/
        // LoopCarrierEnd` machinery loops use  -  the local-allocation and
        // store/load semantics are identical; only the iteration is absent.
        let region_carriers = collect_carrier_names(nodes, &incoming_scope, None);

        // Pre-region: in the parent body, store each carrier's incoming SSA
        // value into the function-local. Idempotent across nested regions
        // sharing a name (the emitter dedupes named-carrier locals).
        for name in &region_carriers {
            let seed_id = incoming_scope
                .get(name)
                .copied()
                .unwrap_or_else(|| unreachable!("region carrier must have incoming binding"));
            body.ops.push(KernelOp {
                kind: KernelOpKind::LoopCarrierInit {
                    name: name.shared_text(),
                },
                operands: vec![seed_id],
                result: None,
            });
        }

        let mut child = empty_body_for_nodes(nodes);

        // Top of region: reload each carrier so reads inside resolve to a
        // fresh in-region SSA id sourced from the local.
        for name in &region_carriers {
            let read_id = self.alloc_value()?;
            child.ops.push(KernelOp {
                kind: KernelOpKind::LoopCarrier {
                    name: name.shared_text(),
                },
                operands: Vec::new(),
                result: Some(read_id),
            });
            self.scope.bind(name.clone(), read_id);
        }

        // Mark the carriers active so `Node::Assign { name, .. }` inside
        // the body emits `LoopCarrierEnd` (commit to local) followed by
        // `LoopCarrier` (re-read) instead of just rebinding the SSA id  -
        // the rebind alone would leak an in-region SSA into the parent.
        let mut carrier_frame: FxHashSet<Ident> = FxHashSet::default();
        for name in &region_carriers {
            carrier_frame.insert(name.clone());
        }
        self.active_carriers.push(carrier_frame);
        self.lower_nodes(nodes, &mut child, depth + 1)?;
        self.active_carriers.pop();

        // Discard in-region `Let`-introduced bindings: they're scoped to
        // the child KernelBody and would otherwise leak into the parent's
        // name table where their SSA ids are out of scope. Carriers will
        // be rebound to fresh post-region read ids below.
        self.scope.restore(incoming_scope);

        let child_id = push_child(body, child)?;
        body.ops.push(KernelOp {
            kind,
            operands: vec![child_id],
            result: None,
        });

        // Post-region: in the parent body, reload each carrier so
        // subsequent reads see the in-region final value. Rebind in scope
        // so `Var(name)` downstream resolves to this id rather than the
        // pre-region seed. Without this read, `n_tokens=0` is the symptom:
        // the in-region final value of `tok_idx` (and every other Assign'd
        // name) was emitted as an SSA id local to the child KernelBody,
        // out of scope from the parent's reads.
        for name in &region_carriers {
            let post_id = self.alloc_value()?;
            body.ops.push(KernelOp {
                kind: KernelOpKind::LoopCarrier {
                    name: name.shared_text(),
                },
                operands: Vec::new(),
                result: Some(post_id),
            });
            self.scope.bind(name.clone(), post_id);
        }
        Ok(())
    }

    fn lower_async_copy(
        &mut self,
        body: &mut KernelBody,
        kind: KernelOpKind,
        source: &Ident,
        destination: &Ident,
        offset: &Expr,
        size: &Expr,
    ) -> Result<(), LowerError> {
        let source_slot = self.buffer_slot(source)?;
        let destination_slot = self.buffer_slot(destination)?;
        let offset_id = self.lower_expr(offset, body)?;
        let size_id = self.lower_expr(size, body)?;
        body.ops.push(KernelOp {
            kind,
            operands: vec![source_slot, destination_slot, offset_id, size_id],
            result: None,
        });
        Ok(())
    }

    fn lower_expr(&mut self, expr: &Expr, body: &mut KernelBody) -> Result<u32, LowerError> {
        match expr {
            Expr::LitU32(value) => self.literal(body, LiteralValue::U32(*value)),
            Expr::LitI32(value) => self.literal(body, LiteralValue::I32(*value)),
            Expr::LitF32(value) => self.literal(body, LiteralValue::F32(*value)),
            Expr::LitBool(value) => self.literal(body, LiteralValue::Bool(*value)),
            Expr::Var(name) => self.scope.get(name).ok_or_else(|| {
                LowerError::InvalidProgram(format!(
                    "variable `{name}` is referenced before binding. Fix: emit a Let/Assign before use."
                ))
            }),
            Expr::Load { buffer, index } => {
                let slot = self.buffer_slot(buffer)?;
                let index_id = self.lower_expr(index, body)?;
                let result = self.alloc_value()?;
                body.ops.push(KernelOp {
                    kind: self.load_kind(slot),
                    operands: vec![slot, index_id],
                    result: Some(result),
                });
                Ok(result)
            }
            Expr::BufLen { buffer } => {
                let slot = self.buffer_slot(buffer)?;
                let result = self.alloc_value()?;
                body.ops.push(KernelOp {
                    kind: KernelOpKind::BufferLength,
                    operands: vec![slot],
                    result: Some(result),
                });
                Ok(result)
            }
            Expr::InvocationId { axis } => {
                self.builtin_axis(body, KernelOpKind::GlobalInvocationId, *axis)
            }
            Expr::WorkgroupId { axis } => {
                self.builtin_axis(body, KernelOpKind::WorkgroupId, *axis)
            }
            Expr::LocalId { axis } => {
                self.builtin_axis(body, KernelOpKind::LocalInvocationId, *axis)
            }
            Expr::BinOp { op, left, right } => {
                // Subgroup/wave ops are spelled as binary ops at the Program
                // level but have dedicated subgroup KernelOps (and emit to
                // `subgroup*` WGSL statements, not a BinaryOperator). Route them
                // before the generic `BinOpKind` path, which has no Naga operator
                // and would fail closed at emit. Operand contract mirrors the
                // canonical `Expr::Subgroup*` lowering below.
                match op {
                    BinOp::Shuffle => {
                        let value_id = self.lower_expr(left, body)?;
                        let lane_id = self.lower_expr(right, body)?;
                        self.binary(body, KernelOpKind::SubgroupShuffle, value_id, lane_id)
                    }
                    BinOp::WaveBroadcast => {
                        let value_id = self.lower_expr(left, body)?;
                        let lane_id = self.lower_expr(right, body)?;
                        self.binary(body, KernelOpKind::SubgroupBroadcast, value_id, lane_id)
                    }
                    BinOp::Ballot => {
                        // Ballot is unary (predicate). The binary spelling's
                        // right operand is unused, so it is not lowered.
                        let cond_id = self.lower_expr(left, body)?;
                        self.unary(body, KernelOpKind::SubgroupBallot, cond_id)
                    }
                    BinOp::WaveReduce => {
                        // The only subgroup reduce KernelOp is Add (sum-reduce),
                        // and the binary spelling carries no reduce-op selector,
                        // so WaveReduce lowers to a subgroup sum across the wave.
                        let value_id = self.lower_expr(left, body)?;
                        self.unary(
                            body,
                            KernelOpKind::SubgroupReduce {
                                op: SubgroupReduceOp::Add,
                            },
                            value_id,
                        )
                    }
                    _ => {
                        let left_id = self.lower_expr(left, body)?;
                        let right_id = self.lower_expr(right, body)?;
                        self.binary(body, KernelOpKind::BinOpKind(*op), left_id, right_id)
                    }
                }
            }
            Expr::UnOp { op, operand } => {
                let operand_id = self.lower_expr(operand, body)?;
                self.unary(body, KernelOpKind::UnOpKind(op.clone()), operand_id)
            }
            Expr::Call { op_id, args } => {
                let mut operands = Vec::with_capacity(args.len());
                for arg in args {
                    operands.push(self.lower_expr(arg, body)?);
                }
                let result = self.alloc_value()?;
                body.ops.push(KernelOp {
                    kind: KernelOpKind::Call {
                        op_id: op_id.shared_text(),
                    },
                    operands,
                    result: Some(result),
                });
                Ok(result)
            }
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => {
                let cond_id = self.lower_expr(cond, body)?;
                let true_id = self.lower_expr(true_val, body)?;
                let false_id = self.lower_expr(false_val, body)?;
                let result = self.alloc_value()?;
                body.ops.push(KernelOp {
                    kind: KernelOpKind::Select,
                    operands: vec![cond_id, true_id, false_id],
                    result: Some(result),
                });
                Ok(result)
            }
            Expr::Cast { target, value } => {
                let value_id = self.lower_expr(value, body)?;
                self.unary(
                    body,
                    KernelOpKind::Cast {
                        target: target.clone(),
                    },
                    value_id,
                )
            }
            Expr::Fma { a, b, c } => {
                let a_id = self.lower_expr(a, body)?;
                let b_id = self.lower_expr(b, body)?;
                let c_id = self.lower_expr(c, body)?;
                let result = self.alloc_value()?;
                body.ops.push(KernelOp {
                    kind: KernelOpKind::Fma,
                    operands: vec![a_id, b_id, c_id],
                    result: Some(result),
                });
                Ok(result)
            }
            Expr::Atomic {
                op,
                buffer,
                index,
                expected,
                value,
                ordering,
            } => {
                let slot = self.buffer_slot(buffer)?;
                let index_id = self.lower_expr(index, body)?;
                let value_id = self.lower_expr(value, body)?;
                let operands = if matches!(
                    op,
                    AtomicOp::CompareExchange | AtomicOp::CompareExchangeWeak
                ) {
                    let Some(expected) = expected else {
                        return Err(LowerError::InvalidProgram(
                            "atomic compare-exchange is missing expected value. Fix: set Expr::Atomic.expected.".into(),
                        ));
                    };
                    let expected_id = self.lower_expr(expected, body)?;
                    vec![slot, index_id, expected_id, value_id]
                } else {
                    vec![slot, index_id, value_id]
                };
                let result = self.alloc_value()?;
                body.ops.push(KernelOp {
                    kind: KernelOpKind::Atomic {
                        op: *op,
                        ordering: *ordering,
                    },
                    operands,
                    result: Some(result),
                });
                Ok(result)
            }
            Expr::SubgroupBallot { cond } => {
                let cond_id = self.lower_expr(cond, body)?;
                self.unary(body, KernelOpKind::SubgroupBallot, cond_id)
            }
            Expr::SubgroupShuffle { value, lane } => {
                let value_id = self.lower_expr(value, body)?;
                let lane_id = self.lower_expr(lane, body)?;
                self.binary(body, KernelOpKind::SubgroupShuffle, value_id, lane_id)
            }
            Expr::SubgroupReduce { op, value } => {
                let value_id = self.lower_expr(value, body)?;
                self.unary(body, KernelOpKind::SubgroupReduce { op: *op }, value_id)
            }
            Expr::SubgroupLocalId => self.simple_result(body, KernelOpKind::SubgroupLocalId),
            Expr::SubgroupSize => self.simple_result(body, KernelOpKind::SubgroupSize),
            Expr::Opaque(extension) => {
                let result = self.alloc_value()?;
                body.ops.push(KernelOp {
                    kind: KernelOpKind::OpaqueExpr(Box::new(OpaqueExprData {
                        extension_id: opaque_extension_id(&**extension),
                        extension_kind: extension.extension_kind().to_owned(),
                        payload: extension.wire_payload(),
                    })),
                    operands: Vec::new(),
                    result: Some(result),
                });
                Ok(result)
            }
            other => Err(LowerError::UnsupportedConstruct(format!(
                "expression `{other:?}` has no KernelDescriptor lowering. Fix: add a descriptor op mapping."
            ))),
        }
    }

    fn buffer_slot(&self, buffer: &Ident) -> Result<u32, LowerError> {
        self.buffer_slots
            .get(buffer)
            .copied()
            .ok_or_else(|| LowerError::UndeclaredBuffer(buffer.to_string()))
    }

    fn load_kind(&self, slot: u32) -> KernelOpKind {
        self.slot_memory_classes
            .get(&slot)
            .copied()
            .map(|memory_class| match memory_class {
                MemoryClass::Shared => KernelOpKind::LoadShared,
                MemoryClass::Constant | MemoryClass::Uniform => KernelOpKind::LoadConstant,
                MemoryClass::Global | MemoryClass::Scratch => KernelOpKind::LoadGlobal,
            })
            .unwrap_or(KernelOpKind::LoadGlobal)
    }

    fn store_kind(&self, slot: u32, buffer: &Ident) -> Result<KernelOpKind, LowerError> {
        match self.slot_memory_classes.get(&slot).copied() {
            Some(MemoryClass::Shared) => Ok(KernelOpKind::StoreShared),
            Some(MemoryClass::Constant | MemoryClass::Uniform) => Err(LowerError::InvalidProgram(format!(
                "Store to constant/uniform-class buffer `{buffer}` is invalid  -  read-only at the dispatch boundary. Fix: change the buffer's MemoryKind to Global or its access to ReadWrite."
            ))),
            Some(MemoryClass::Global | MemoryClass::Scratch) => Ok(KernelOpKind::StoreGlobal),
            None => Ok(KernelOpKind::StoreGlobal),
        }
    }

    fn add_trap_sidecar_binding(&mut self) -> Result<(), LowerError> {
        if self
            .buffer_slots
            .contains_key(&Ident::from(TRAP_SIDECAR_NAME))
        {
            return Err(LowerError::UnsupportedConstruct(format!(
                "program declares reserved trap sidecar buffer `{TRAP_SIDECAR_NAME}`. Fix: choose a user buffer name outside the `__vyre_*` namespace."
            )));
        }
        // Only consider host-visible slots when picking the next trap sidecar
        // slot. Shared/Scratch slots live in the WORKGROUP_SLOT_BASE (1<<24)
        // range and are not host-bound; mixing them in here would push the
        // trap sidecar  -  which IS host-bound  -  past the wgpu max binding
        // index (1000) and the layout validator would reject it.
        let next_slot = self
            .bindings
            .iter()
            .filter(|binding| {
                !matches!(
                    binding.memory_class,
                    MemoryClass::Shared | MemoryClass::Scratch,
                )
            })
            .map(|binding| binding.slot)
            .max()
            .map_or(Ok(0), |slot| {
                slot.checked_add(1).ok_or(LowerError::OperandIdOverflow)
            })?;
        self.buffer_slots
            .insert(Ident::from(TRAP_SIDECAR_NAME), next_slot);
        self.slot_memory_classes
            .insert(next_slot, MemoryClass::Global);
        self.bindings.push(BindingSlot {
            slot: next_slot,
            element_type: DataType::U32,
            element_count: Some(TRAP_SIDECAR_WORDS),
            memory_class: MemoryClass::Global,
            visibility: BindingVisibility::ReadWrite,
            name: TRAP_SIDECAR_NAME.to_owned(),
        });
        self.bindings.sort_by_key(|slot| slot.slot);
        Ok(())
    }

    fn literal(&mut self, body: &mut KernelBody, literal: LiteralValue) -> Result<u32, LowerError> {
        let literal_index = push_literal(body, literal)?;
        let result = self.alloc_value()?;
        body.ops.push(KernelOp {
            kind: KernelOpKind::Literal,
            operands: vec![literal_index],
            result: Some(result),
        });
        Ok(result)
    }

    fn builtin_axis(
        &mut self,
        body: &mut KernelBody,
        kind: KernelOpKind,
        axis: u8,
    ) -> Result<u32, LowerError> {
        if axis > 2 {
            return Err(LowerError::InvalidProgram(format!(
                "builtin axis {axis} is out of range. Fix: use axis 0, 1, or 2."
            )));
        }
        let result = self.alloc_value()?;
        body.ops.push(KernelOp {
            kind,
            operands: vec![u32::from(axis)],
            result: Some(result),
        });
        Ok(result)
    }

    fn simple_result(
        &mut self,
        body: &mut KernelBody,
        kind: KernelOpKind,
    ) -> Result<u32, LowerError> {
        let result = self.alloc_value()?;
        body.ops.push(KernelOp {
            kind,
            operands: Vec::new(),
            result: Some(result),
        });
        Ok(result)
    }

    fn copy_value(&mut self, body: &mut KernelBody, operand: u32) -> Result<u32, LowerError> {
        let result = self.alloc_value()?;
        body.ops.push(KernelOp {
            kind: KernelOpKind::Copy,
            operands: vec![operand],
            result: Some(result),
        });
        Ok(result)
    }

    fn unary(
        &mut self,
        body: &mut KernelBody,
        kind: KernelOpKind,
        operand: u32,
    ) -> Result<u32, LowerError> {
        let result = self.alloc_value()?;
        body.ops.push(KernelOp {
            kind,
            operands: vec![operand],
            result: Some(result),
        });
        Ok(result)
    }

    fn binary(
        &mut self,
        body: &mut KernelBody,
        kind: KernelOpKind,
        left: u32,
        right: u32,
    ) -> Result<u32, LowerError> {
        let result = self.alloc_value()?;
        body.ops.push(KernelOp {
            kind,
            operands: vec![left, right],
            result: Some(result),
        });
        Ok(result)
    }

    fn alloc_value(&mut self) -> Result<u32, LowerError> {
        let id = self.next_value;
        self.next_value = self
            .next_value
            .checked_add(1)
            .ok_or(LowerError::OperandIdOverflow)?;
        Ok(id)
    }

    fn emit_loop_carrier_read(
        &mut self,
        body: &mut KernelBody,
        name: &Ident,
    ) -> Result<(), LowerError> {
        let result = self.alloc_value()?;
        body.ops.push(KernelOp {
            kind: KernelOpKind::LoopCarrier {
                name: name.shared_text(),
            },
            operands: Vec::new(),
            result: Some(result),
        });
        self.scope.bind(name.clone(), result);
        Ok(())
    }
}

/// Walk a `Node::Loop` / `Node::Region` / `Node::Block` body and collect
/// every source-level variable name that:
///   1. Appears on the left of an `Assign` somewhere inside the body
///      (including nested If/Block/Region/Loop scopes); AND
///   2. Was already bound in the incoming scope (so the assignment
///      mutates an outer binding, not a body-local `Let`).
///
/// These are the names whose final value must escape the body via a
/// function-local: for a `Loop` it is the per-iteration carrier, for a
/// `Region`/`Block` it is the region-exit phi-merge. Loop callers pass
/// `Some(loop_var)` to skip the loop-induction variable (handled by
/// `LoopIndex` / `LoopCarrierEnd` is not emitted for it). Region/Block
/// callers pass `None`.
///
/// Order is the deterministic order names are first observed during a
/// pre-order walk, so the emitted op stream is stable across runs.

fn collect_carrier_names(
    body: &[Node],
    incoming_scope: &scope::ScopeSnapshot,
    loop_var: Option<&Ident>,
) -> Vec<Ident> {
    let mut seen: FxHashSet<Ident> = FxHashSet::default();
    let mut order: Vec<Ident> = Vec::new();
    let mut local_lets: Vec<FxHashSet<Ident>> = vec![FxHashSet::default()];

    fn walk(
        nodes: &[Node],
        incoming_scope: &scope::ScopeSnapshot,
        loop_var: Option<&Ident>,
        seen: &mut FxHashSet<Ident>,
        order: &mut Vec<Ident>,
        local_lets: &mut Vec<FxHashSet<Ident>>,
    ) {
        for node in nodes {
            match node {
                Node::Let { name, .. } => {
                    if let Some(top) = local_lets.last_mut() {
                        top.insert(name.clone());
                    }
                }
                Node::Assign { name, .. } => {
                    if let Some(lv) = loop_var {
                        if name == lv {
                            continue;
                        }
                    }
                    let shadowed = local_lets.iter().any(|frame| frame.contains(name));
                    if shadowed {
                        continue;
                    }
                    if !incoming_scope.contains_key(name) {
                        continue;
                    }
                    if seen.insert(name.clone()) {
                        order.push(name.clone());
                    }
                }
                Node::Block(inner) => {
                    local_lets.push(FxHashSet::default());
                    walk(inner, incoming_scope, loop_var, seen, order, local_lets);
                    local_lets.pop();
                }
                Node::If {
                    then, otherwise, ..
                } => {
                    local_lets.push(FxHashSet::default());
                    walk(then, incoming_scope, loop_var, seen, order, local_lets);
                    local_lets.pop();
                    local_lets.push(FxHashSet::default());
                    walk(otherwise, incoming_scope, loop_var, seen, order, local_lets);
                    local_lets.pop();
                }
                Node::Loop {
                    var: inner_var,
                    body: inner_body,
                    ..
                } => {
                    local_lets.push({
                        let mut s = FxHashSet::default();
                        s.insert(inner_var.clone());
                        s
                    });
                    walk(
                        inner_body,
                        incoming_scope,
                        loop_var,
                        seen,
                        order,
                        local_lets,
                    );
                    local_lets.pop();
                }
                Node::Region { body: inner, .. } => {
                    local_lets.push(FxHashSet::default());
                    walk(inner, incoming_scope, loop_var, seen, order, local_lets);
                    local_lets.pop();
                }
                _ => {}
            }
        }
    }

    walk(
        body,
        incoming_scope,
        loop_var,
        &mut seen,
        &mut order,
        &mut local_lets,
    );
    order
}

fn body_contains_trap(body: &KernelBody) -> bool {
    body.ops
        .iter()
        .any(|op| matches!(op.kind, KernelOpKind::Trap { .. }))
        || body.child_bodies.iter().any(body_contains_trap)
}

fn opaque_extension_id(extension: &dyn vyre_foundation::ir::ExprNode) -> u32 {
    u32::from_le_bytes(
        extension.stable_fingerprint()[0..4]
            .try_into()
            .unwrap_or_else(|_| unreachable!("slice length is fixed")),
    )
}

fn empty_body_for_nodes(nodes: &[Node]) -> KernelBody {
    empty_body_with_capacity(estimated_node_slice_op_capacity(nodes))
}

fn empty_body_with_capacity(op_capacity: usize) -> KernelBody {
    KernelBody {
        ops: Vec::with_capacity(op_capacity),
        child_bodies: Vec::with_capacity(estimated_child_body_capacity(op_capacity)),
        literals: Vec::with_capacity(op_capacity / 3),
    }
}

fn estimated_root_op_capacity(program: &Program) -> usize {
    let stats = program.stats();
    stats
        .instruction_count
        .saturating_add(stats.node_count as u64)
        .saturating_add(4)
        .min(usize::MAX as u64) as usize
}

fn estimated_node_slice_op_capacity(nodes: &[Node]) -> usize {
    nodes
        .len()
        .saturating_mul(2)
        .saturating_add(estimated_child_body_capacity(nodes.len()))
}

fn estimated_child_body_capacity(parent_ops: usize) -> usize {
    parent_ops.min(16)
}

fn push_literal(body: &mut KernelBody, literal: LiteralValue) -> Result<u32, LowerError> {
    let index = u32::try_from(body.literals.len()).map_err(|_| LowerError::OperandIdOverflow)?;
    body.literals.push(literal);
    Ok(index)
}

fn push_child(body: &mut KernelBody, child: KernelBody) -> Result<u32, LowerError> {
    let index =
        u32::try_from(body.child_bodies.len()).map_err(|_| LowerError::OperandIdOverflow)?;
    body.child_bodies.push(child);
    Ok(index)
}

fn memory_class(buffer: &BufferDecl) -> Result<MemoryClass, LowerError> {
    match (buffer.kind, &buffer.access) {
        (MemoryKind::Persistent, _) => Err(LowerError::UnsupportedConstruct(format!(
            "Persistent memory buffer `{}` cannot be lowered as a direct GPU binding. Fix: stage Persistent data through the host transfer path using AsyncLoad/AsyncStore into Global/Readonly memory before concrete GPU emission.",
            buffer.name()
        ))),
        (MemoryKind::Shared, _) | (_, BufferAccess::Workgroup) => Ok(MemoryClass::Shared),
        (MemoryKind::Local, _) => Ok(MemoryClass::Scratch),
        (MemoryKind::Uniform | MemoryKind::Push, _) | (_, BufferAccess::Uniform) => {
            Ok(MemoryClass::Uniform)
        }
        (MemoryKind::Readonly, _) | (_, BufferAccess::ReadOnly) => Ok(MemoryClass::Constant),
        (MemoryKind::Global, _) => Ok(MemoryClass::Global),
        (other, _) => Err(LowerError::UnsupportedConstruct(format!(
            "MemoryKind::{other:?} for buffer `{}` is not supported by neutral lowering. Fix: map the buffer to Global, Shared, Uniform, Readonly, Push, or Local before emission.",
            buffer.name()
        ))),
    }
}

fn binding_visibility(access: &BufferAccess) -> BindingVisibility {
    match access {
        BufferAccess::ReadOnly | BufferAccess::Uniform => BindingVisibility::ReadOnly,
        BufferAccess::WriteOnly => BindingVisibility::WriteOnly,
        _ => BindingVisibility::ReadWrite,
    }
}

fn fingerprint_id(program: &Program) -> String {
    // Direct hex table lookup is ~100x faster than per-byte write!() with
    // formatter dispatch. fingerprint is a fixed 32 bytes, so the output
    // is exactly 64 hex chars.
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let fingerprint = program.fingerprint();
    let mut out = String::with_capacity(fingerprint.len() * 2);
    for &byte in fingerprint.iter() {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// Public re-export of the binding-construction shape so emitters can
/// build descriptors in tests without going through `lower()`.
pub fn binding_slot(
    slot: u32,
    name: impl Into<String>,
    element_type: DataType,
    element_count: Option<u32>,
    memory_class: MemoryClass,
    visibility: BindingVisibility,
) -> BindingSlot {
    BindingSlot {
        slot,
        element_type,
        element_count,
        memory_class,
        visibility,
        name: name.into(),
    }
}

/// Public helper for a scalar-store op (used by descriptor-building
/// tests in this crate and by emitter integration tests).
pub fn store_global(
    slot_operand_id: u32,
    index_operand_id: u32,
    value_operand_id: u32,
) -> KernelOp {
    KernelOp {
        kind: KernelOpKind::StoreGlobal,
        operands: vec![slot_operand_id, index_operand_id, value_operand_id],
        result: None,
    }
}

/// Public helper for a u32 literal op.
pub fn literal_u32(literal_pool_index: u32, result_id: u32) -> KernelOp {
    KernelOp {
        kind: KernelOpKind::Literal,
        operands: vec![literal_pool_index],
        result: Some(result_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_empty_wrapped_program_preserves_region() {
        let program = vyre_foundation::ir::Program::wrapped(vec![], [1, 1, 1], vec![]);
        let desc = lower(&program).unwrap();
        assert_eq!(desc.dispatch.workgroup_size, [1, 1, 1]);
        assert!(desc.bindings.slots.is_empty());
        assert_eq!(desc.body.ops.len(), 1);
        assert!(matches!(desc.body.ops[0].kind, KernelOpKind::Region { .. }));
    }

    #[test]
    fn binding_slot_helper_records_inputs() {
        let s = binding_slot(
            3,
            "scratch",
            DataType::F32,
            Some(64),
            MemoryClass::Shared,
            BindingVisibility::ReadWrite,
        );
        assert_eq!(s.slot, 3);
        assert_eq!(s.name, "scratch");
        assert_eq!(s.element_type, DataType::F32);
        assert_eq!(s.element_count, Some(64));
        assert_eq!(s.memory_class, MemoryClass::Shared);
        assert_eq!(s.visibility, BindingVisibility::ReadWrite);
    }

    #[test]
    fn store_global_helper_packs_three_operands() {
        let op = store_global(0, 1, 2);
        assert_eq!(op.kind, KernelOpKind::StoreGlobal);
        assert_eq!(op.operands, vec![0, 1, 2]);
        assert_eq!(op.result, None);
    }

    #[test]
    fn literal_u32_helper_assigns_result_id() {
        let op = literal_u32(5, 42);
        assert_eq!(op.kind, KernelOpKind::Literal);
        assert_eq!(op.operands, vec![5]);
        assert_eq!(op.result, Some(42));
    }

    #[test]
    fn lower_assigns_unique_descriptor_slots_for_duplicate_program_bindings() {
        use vyre_foundation::ir::{BufferDecl, Expr, Node};

        let program = Program::wrapped(
            vec![
                BufferDecl::workgroup("scratch", 16, DataType::U32),
                BufferDecl::output("out", 0, DataType::U32).with_count(1),
            ],
            [64, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u32(1))],
        );

        let desc = lower(&program).expect("Fix: duplicate Program bindings must descriptor-lower");

        assert_eq!(desc.bindings.slots.len(), 2);
        assert_ne!(desc.bindings.slots[0].slot, desc.bindings.slots[1].slot);
        assert!(crate::verify::verify(&desc).is_ok());
    }

    #[test]
    fn lower_trap_inserts_descriptor_sidecar_binding() {
        use vyre_foundation::ir::{Expr, Node};

        let program = Program::wrapped(
            vec![],
            [64, 1, 1],
            vec![Node::trap(Expr::u32(7), "page-fault")],
        );

        let desc = lower(&program).expect("Fix: trap programs must descriptor-lower");
        let sidecar = desc
            .bindings
            .slots
            .iter()
            .find(|slot| slot.name == TRAP_SIDECAR_NAME)
            .expect("Fix: trap sidecar binding must be inserted");
        assert_eq!(sidecar.element_type, DataType::U32);
        assert_eq!(sidecar.element_count, Some(TRAP_SIDECAR_WORDS));
        assert!(matches!(sidecar.visibility, BindingVisibility::ReadWrite));
        assert!(crate::verify::verify(&desc).is_ok());
    }

    #[test]
    fn trap_sidecar_slot_stays_in_host_range_when_program_has_workgroup_buffer() {
        // Regression: trap sidecar must skip Shared/Scratch slots when
        // picking its slot id. Workgroup-class slots live in the
        // 1<<24 reserved range and a host-bound binding past wgpu's
        // 1000-binding limit fails layout creation.
        use vyre_foundation::ir::{BufferDecl, Expr, Node};

        let program = Program::wrapped(
            vec![
                BufferDecl::output("out", 0, DataType::U32).with_count(1),
                BufferDecl::workgroup("scratch", 16, DataType::U32),
            ],
            [64, 1, 1],
            vec![
                Node::store("out", Expr::u32(0), Expr::u32(1)),
                Node::trap(Expr::u32(7), "fault"),
            ],
        );

        let desc = lower(&program).expect("Fix: trap + workgroup programs must lower");
        let sidecar = desc
            .bindings
            .slots
            .iter()
            .find(|slot| slot.name == TRAP_SIDECAR_NAME)
            .expect("Fix: trap sidecar must be present");
        assert!(
            sidecar.slot < 1024,
            "trap sidecar slot must stay in the host-bindable range; got {}",
            sidecar.slot,
        );
    }

    #[test]
    fn lower_opaque_expr_preserves_kind_and_payload() {
        use vyre_foundation::ir::{BufferDecl, Expr, Node};

        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![Node::store("out", Expr::u32(0), Expr::u64(42))],
        );

        let desc = lower(&program).expect("Fix: opaque literals must descriptor-lower");
        fn find_opaque_expr(body: &KernelBody) -> Option<(&String, &Vec<u8>)> {
            body.ops
                .iter()
                .find_map(|op| match &op.kind {
                    KernelOpKind::OpaqueExpr(data) => Some((&data.extension_kind, &data.payload)),
                    _ => None,
                })
                .or_else(|| body.child_bodies.iter().find_map(find_opaque_expr))
        }

        let opaque =
            find_opaque_expr(&desc.body).expect("Fix: opaque expression op must be present");
        assert_eq!(opaque.0, "vyre.literal.u64");
        assert_eq!(opaque.1, &42u64.to_le_bytes().to_vec());
    }

    #[test]
    fn loop_variable_lowers_to_child_loop_index_result() {
        use vyre_foundation::ir::{BufferDecl, Expr, Node};

        let program = Program::wrapped(
            vec![
                BufferDecl::storage("input", 0, BufferAccess::ReadOnly, DataType::U32),
                BufferDecl::output("out", 1, DataType::U32).with_count(1),
            ],
            [1, 1, 1],
            vec![Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::buf_len("input"),
                vec![Node::store(
                    "out",
                    Expr::u32(0),
                    Expr::load("input", Expr::var("i")),
                )],
            )],
        );

        let desc = lower(&program).expect("Fix: loop variable must descriptor-lower");
        assert!(crate::verify::verify(&desc).is_ok());
        let (loop_body, loop_op) =
            find_loop(&desc.body).expect("Fix: structured loop op must be present");
        let child = &loop_body.child_bodies[loop_op.operands[2] as usize];
        assert!(
            matches!(
                child.ops.first().map(|op| &op.kind),
                Some(KernelOpKind::LoopIndex { loop_var }) if loop_var.as_ref() == "i"
            ),
            "loop body must materialize the induction value before lowering input[i]"
        );

        fn find_loop(body: &KernelBody) -> Option<(&KernelBody, &KernelOp)> {
            for op in &body.ops {
                if matches!(op.kind, KernelOpKind::StructuredForLoop { .. }) {
                    return Some((body, op));
                }
            }
            body.child_bodies.iter().find_map(find_loop)
        }
    }

    #[test]
    fn loop_variable_does_not_clobber_same_named_outer_binding() {
        use vyre_foundation::ir::{BufferDecl, Expr, Node};

        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![
                Node::let_bind("i", Expr::u32(9)),
                Node::loop_for("i", Expr::u32(0), Expr::u32(1), vec![]),
                Node::store("out", Expr::u32(0), Expr::var("i")),
            ],
        );

        let desc = lower(&program).expect("Fix: shadowed loop variable must descriptor-lower");
        assert!(crate::verify::verify(&desc).is_ok());
        let store = find_store(&desc.body).expect("Fix: post-loop store must be present");
        assert_eq!(
            store.operands[2], 0,
            "post-loop read must use the outer i binding, not the loop induction result"
        );

        fn find_store(body: &KernelBody) -> Option<&KernelOp> {
            body.ops
                .iter()
                .find(|op| matches!(op.kind, KernelOpKind::StoreGlobal))
                .or_else(|| body.child_bodies.iter().find_map(find_store))
        }
    }

    #[test]
    fn if_else_branches_lower_from_the_same_incoming_scope() {
        use vyre_foundation::ir::{BufferDecl, Expr, Node};

        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![
                Node::let_bind("x", Expr::u32(1)),
                Node::if_then_else(
                    Expr::bool(true),
                    vec![Node::assign("x", Expr::add(Expr::var("x"), Expr::u32(1)))],
                    vec![Node::store("out", Expr::u32(0), Expr::var("x"))],
                ),
            ],
        );

        let desc = lower(&program).expect("Fix: if/else must descriptor-lower");
        assert!(crate::verify::verify(&desc).is_ok());
        let (_, if_op) = find_if_else(&desc.body).expect("Fix: if/else op must be present");
        let parent = find_parent_body_containing_op(&desc.body, if_op as *const KernelOp)
            .expect("Fix: if op parent body must be found");
        let else_body = &parent.child_bodies[if_op.operands[2] as usize];
        let else_store = else_body
            .ops
            .iter()
            .find(|op| matches!(op.kind, KernelOpKind::StoreGlobal))
            .expect("Fix: else branch must contain the store");
        let else_carrier = else_body
            .ops
            .iter()
            .find(
                |op| matches!(&op.kind, KernelOpKind::LoopCarrier { name } if name.as_ref() == "x"),
            )
            .expect(
                "Fix: else branch must read x through the if carrier seeded from incoming scope",
            );
        let else_carrier_id = else_carrier
            .result
            .expect("Fix: else carrier read must produce an SSA result");
        assert_eq!(
            else_store.operands[2], else_carrier_id,
            "else branch must read the incoming x through its carrier, not the result assigned only by then"
        );

        fn find_if_else(body: &KernelBody) -> Option<(&KernelBody, &KernelOp)> {
            for op in &body.ops {
                if matches!(op.kind, KernelOpKind::StructuredIfThenElse) {
                    return Some((body, op));
                }
            }
            body.child_bodies.iter().find_map(find_if_else)
        }

        fn find_parent_body_containing_op(
            body: &KernelBody,
            target: *const KernelOp,
        ) -> Option<&KernelBody> {
            if body.ops.iter().any(|op| std::ptr::eq(op, target)) {
                return Some(body);
            }
            body.child_bodies
                .iter()
                .find_map(|child| find_parent_body_containing_op(child, target))
        }
    }

    #[test]
    fn loop_carrier_mutated_in_if_then_is_visible_to_next_sibling() {
        use vyre_foundation::ir::{BufferDecl, Expr, Node};

        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![
                Node::let_bind("x", Expr::u32(0)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::u32(1),
                    vec![
                        Node::if_then(Expr::bool(true), vec![Node::assign("x", Expr::u32(7))]),
                        Node::if_then(
                            Expr::bool(true),
                            vec![Node::store("out", Expr::u32(0), Expr::var("x"))],
                        ),
                    ],
                ),
            ],
        );

        let desc =
            lower(&program).expect("Fix: conditional carrier mutation must descriptor-lower");
        assert!(crate::verify::verify(&desc).is_ok());
        let (parent, loop_op) =
            find_loop(&desc.body).expect("Fix: structured loop op must be present");
        let child = &parent.child_bodies[loop_op.operands[2] as usize];
        let first_if_idx = child
            .ops
            .iter()
            .position(|op| matches!(op.kind, KernelOpKind::StructuredIfThen))
            .expect("Fix: first conditional assignment must lower to StructuredIfThen");
        let carrier_idx = child
            .ops
            .iter()
            .enumerate()
            .skip(first_if_idx + 1)
            .find_map(|(idx, op)| match &op.kind {
                KernelOpKind::LoopCarrier { name } if name.as_ref() == "x" => Some(idx),
                _ => None,
            })
            .expect("Fix: parent loop body must reread x carrier after conditional mutation");
        let carrier_result = child.ops[carrier_idx]
            .result
            .expect("Fix: carrier read must produce an SSA result");
        let second_if = child
            .ops
            .iter()
            .skip(carrier_idx + 1)
            .find(|op| matches!(op.kind, KernelOpKind::StructuredIfThen))
            .expect("Fix: second conditional store must lower after carrier reread");
        let store_body = &child.child_bodies[second_if.operands[1] as usize];
        let store = store_body
            .ops
            .iter()
            .find(|op| matches!(op.kind, KernelOpKind::StoreGlobal))
            .expect("Fix: second conditional body must store x");
        assert_eq!(
            store.operands[2], carrier_result,
            "sibling after conditional carrier mutation must read the fresh carrier value"
        );

        fn find_loop(body: &KernelBody) -> Option<(&KernelBody, &KernelOp)> {
            for op in &body.ops {
                if matches!(op.kind, KernelOpKind::StructuredForLoop { .. }) {
                    return Some((body, op));
                }
            }
            body.child_bodies.iter().find_map(find_loop)
        }
    }

    #[test]
    fn max_nesting_depth_constant_is_documented() {
        assert_eq!(MAX_NESTING_DEPTH, 64);
    }

    /// Region phi-merge: a `Node::Region` whose body reassigns an
    /// outer-bound variable must publish the in-region final value back
    /// to the parent body via a function-local. Without the
    /// `LoopCarrierInit/LoopCarrier/LoopCarrierEnd` round-trip, the
    /// in-region SSA id is local to the child KernelBody and the parent
    /// reads the pre-region seed (the `n_tokens=0` GPU-lex symptom).
    #[test]
    fn region_publishes_inner_assign_to_parent_via_carrier() {
        use std::sync::Arc;
        use vyre_foundation::ir::{BufferDecl, Expr, Ident, Node};

        // ```
        // let x = 0;
        // region "phase" { x = 7; }
        // store(out[0], x)
        // ```
        // The post-region store must read the in-region value (7), not
        // the pre-region seed (0). Lowering must emit a parent-body
        // `LoopCarrier { name: "x" }` after the `Region` op whose result
        // id feeds the `StoreGlobal`.
        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![
                Node::let_bind("x", Expr::u32(0)),
                Node::Region {
                    generator: Ident::from("phase"),
                    source_region: None,
                    body: Arc::new(vec![Node::assign("x", Expr::u32(7))]),
                },
                Node::store("out", Expr::u32(0), Expr::var("x")),
            ],
        );

        let desc = lower(&program).expect("Fix: region with inner assign must descriptor-lower");
        assert!(crate::verify::verify(&desc).is_ok());

        // Outer-most kernel body: a single `Region { generator: c_lexer }`
        // wraps the entry tree (program::wrapped). Drill into it.
        let entry = &desc.body;
        assert_eq!(
            entry.ops.len(),
            1,
            "wrapped program has one entry-region op"
        );
        let entry_region_op = &entry.ops[0];
        let entry_region_body = &entry.child_bodies[entry_region_op.operands[0] as usize];

        // Find the explicit `Region { generator: "phase" }` op inside
        // the entry region.
        let phase_pos = entry_region_body
            .ops
            .iter()
            .position(|op| {
                matches!(&op.kind, KernelOpKind::Region { generator } if generator.as_ref() == "phase")
            })
            .expect("Fix: phase Region op must be lowered");
        let phase_op = &entry_region_body.ops[phase_pos];

        // Pre-region: must emit `LoopCarrierInit { name: "x" }` BEFORE
        // the `Region` op so the function-local is seeded with the
        // pre-region value of x.
        let init_pos = entry_region_body
            .ops
            .iter()
            .position(|op| {
                matches!(&op.kind, KernelOpKind::LoopCarrierInit { name } if name.as_ref() == "x")
            })
            .expect("Fix: region must emit LoopCarrierInit for the carried name");
        assert!(
            init_pos < phase_pos,
            "LoopCarrierInit must precede the Region op so the local is seeded before entry"
        );

        // Inside the region body: the `Assign` lowers via the active-
        // carrier path → `LoopCarrierEnd { name: "x" }` (commit) +
        // `LoopCarrier { name: "x" }` (re-read).
        let phase_body_idx = phase_op.operands[0] as usize;
        let phase_body = &entry_region_body.child_bodies[phase_body_idx];
        assert!(
            phase_body
                .ops
                .iter()
                .any(|op| matches!(&op.kind, KernelOpKind::LoopCarrierEnd { name } if name.as_ref() == "x")),
            "in-region Assign must commit to the carrier local via LoopCarrierEnd"
        );

        // Post-region: parent body must re-read the carrier so the
        // subsequent `Var(x)` resolves to the in-region final value.
        let post_read = entry_region_body
            .ops
            .iter()
            .enumerate()
            .find(|(idx, op)| {
                *idx > phase_pos
                    && matches!(&op.kind, KernelOpKind::LoopCarrier { name } if name.as_ref() == "x")
            })
            .expect("Fix: region must emit a post-Region LoopCarrier read for the carried name");
        let post_read_id = post_read
            .1
            .result
            .expect("Fix: post-region LoopCarrier produces an SSA id");

        // The store must consume the post-region read id, not the
        // pre-region seed.
        let store = entry_region_body
            .ops
            .iter()
            .find(|op| matches!(op.kind, KernelOpKind::StoreGlobal))
            .expect("Fix: post-region store must lower into the parent body");
        assert_eq!(
            store.operands[2], post_read_id,
            "post-region Var(x) read must resolve to the carrier publish id, not the pre-region seed"
        );
    }

    /// Region phi-merge negative: a `Node::Region` whose body does NOT
    /// reassign any outer name must NOT emit any `LoopCarrierInit` /
    /// `LoopCarrierEnd` / `LoopCarrier` ops for region-merge purposes.
    /// (Loop-driven carriers from any enclosing Loop scope are a
    /// separate machinery  -  this test runs at root scope so none are
    /// expected.)
    #[test]
    fn region_without_inner_assign_emits_no_carrier_ops() {
        use std::sync::Arc;
        use vyre_foundation::ir::{BufferDecl, Expr, Ident, Node};

        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![
                Node::let_bind("x", Expr::u32(7)),
                Node::Region {
                    generator: Ident::from("read_only_phase"),
                    source_region: None,
                    body: Arc::new(vec![Node::store("out", Expr::u32(0), Expr::var("x"))]),
                },
            ],
        );

        let desc = lower(&program).expect("Fix: read-only region must descriptor-lower");
        assert!(crate::verify::verify(&desc).is_ok());

        fn count_carrier_ops(body: &KernelBody) -> usize {
            body.ops
                .iter()
                .filter(|op| {
                    matches!(
                        op.kind,
                        KernelOpKind::LoopCarrier { .. }
                            | KernelOpKind::LoopCarrierInit { .. }
                            | KernelOpKind::LoopCarrierEnd { .. }
                    )
                })
                .count()
                + body
                    .child_bodies
                    .iter()
                    .map(count_carrier_ops)
                    .sum::<usize>()
        }
        assert_eq!(
            count_carrier_ops(&desc.body),
            0,
            "no in-region reassignment ⇒ no carrier ops (would be decoration otherwise)"
        );
    }

    /// Region phi-merge nested: a Region inside a Loop whose body
    /// reassigns a loop-carrier-eligible name must commit through the
    /// SAME named-carrier local  -  the Loop's pre-loop init and the
    /// inner Region's pre-region init both target the same slot, so
    /// the next iteration's top-of-loop read sees the in-region final
    /// value of the previous iteration.
    #[test]
    fn region_inside_loop_shares_named_carrier_slot() {
        use std::sync::Arc;
        use vyre_foundation::ir::{BufferDecl, Expr, Ident, Node};

        let program = Program::wrapped(
            vec![BufferDecl::output("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            vec![
                Node::let_bind("acc", Expr::u32(0)),
                Node::loop_for(
                    "i",
                    Expr::u32(0),
                    Expr::u32(4),
                    vec![Node::Region {
                        generator: Ident::from("step"),
                        source_region: None,
                        body: Arc::new(vec![Node::assign(
                            "acc",
                            Expr::add(Expr::var("acc"), Expr::u32(1)),
                        )]),
                    }],
                ),
                Node::store("out", Expr::u32(0), Expr::var("acc")),
            ],
        );

        let desc = lower(&program).expect("Fix: loop+region+assign must descriptor-lower");
        assert!(crate::verify::verify(&desc).is_ok());

        // Locate the StructuredForLoop op and its body.
        fn find_loop(body: &KernelBody) -> Option<(&KernelBody, &KernelOp)> {
            for op in &body.ops {
                if matches!(op.kind, KernelOpKind::StructuredForLoop { .. }) {
                    return Some((body, op));
                }
            }
            body.child_bodies.iter().find_map(find_loop)
        }
        let (loop_parent, loop_op) =
            find_loop(&desc.body).expect("Fix: StructuredForLoop must be lowered");
        let loop_body = &loop_parent.child_bodies[loop_op.operands[2] as usize];

        // Loop body must contain the inner Region op.
        let region_op = loop_body
            .ops
            .iter()
            .find(|op| {
                matches!(&op.kind, KernelOpKind::Region { generator } if generator.as_ref() == "step")
            })
            .expect("Fix: inner Region must lower inside the loop body");
        let region_body = &loop_body.child_bodies[region_op.operands[0] as usize];

        // The inner region's body must commit to the `acc` carrier
        // local on its Assign  -  the same local the Loop uses, since
        // emit-naga keys named-carrier locals by name.
        assert!(
            region_body
                .ops
                .iter()
                .any(|op| matches!(&op.kind, KernelOpKind::LoopCarrierEnd { name } if name.as_ref() == "acc")),
            "Assign inside loop+region must commit through the named carrier local"
        );

        // Post-loop: the parent body's StoreGlobal must read the
        // post-loop carrier publish (loop's existing post-loop emission)
        //  -  proving the in-region commit propagates out of the loop.
        let store = desc
            .body
            .child_bodies
            .iter()
            .flat_map(|child| child.ops.iter())
            .find(|op| matches!(op.kind, KernelOpKind::StoreGlobal))
            .expect("Fix: post-loop store must lower");
        assert!(
            !store.operands.is_empty(),
            "post-loop store must read the published carrier"
        );
    }
}
