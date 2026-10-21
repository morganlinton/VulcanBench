//! `KernelOp` → naga emit dispatcher. The big match arm  -  every
//! `KernelOpKind` variant routes to its emit helper from here. Plus the
//! two helpers that only `emit_op` calls (`global_invocation_axis`,
//! `emit_opaque_expr`).

use std::fmt::Write as _;
use std::mem::{self, Discriminant};

use naga::{BinaryOperator, Expression, Literal, LocalVariable, ScalarKind, Span, Statement};
use rustc_hash::FxHashMap;
use vyre_foundation::ir::{BinOp, DataType, UnOp};
use vyre_lower::{KernelBody, KernelOp, KernelOpKind, LiteralValue};

use super::op_lookup::{
    barrier_flags, binary_math_function, binary_operator, naga_literal, scalar_cast_target,
    unary_math_function, unary_operator, unpack_shift_mask,
};
use super::BodyBuilder;
use crate::EmitError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum OpDispatchRoute {
    Literal,
    LocalInvocationId,
    GlobalInvocationId,
    WorkgroupId,
    SubgroupLocalId,
    SubgroupSize,
    LoopIndex,
    BufferLength,
    Load,
    Store,
    Copy,
    BinOpKind,
    UnOpKind,
    Cast,
    Select,
    Fma,
    StructuredIfThen,
    StructuredIfThenElse,
    StructuredBlock,
    StructuredForLoop,
    AsyncLoad,
    AsyncStore,
    AsyncWait,
    Trap,
    Resume,
    Barrier,
    Return,
    SubgroupBallot,
    SubgroupReduce,
    SubgroupShuffle,
    SubgroupBroadcast,
    Atomic,
    IndirectDispatch,
    MatrixMma,
    Call,
    OpaqueExpr,
    OpaqueNode,
    LoopCarrierInit,
    LoopCarrier,
    LoopCarrierEnd,
}

pub(super) struct OpDispatchRouteCache {
    routes: FxHashMap<Discriminant<KernelOpKind>, OpDispatchRoute>,
    #[cfg(test)]
    hits: usize,
}

impl Default for OpDispatchRouteCache {
    fn default() -> Self {
        Self {
            routes: FxHashMap::default(),
            #[cfg(test)]
            hits: 0,
        }
    }
}

impl OpDispatchRouteCache {
    fn route(&mut self, kind: &KernelOpKind) -> OpDispatchRoute {
        let key = mem::discriminant(kind);
        if let Some(route) = self.routes.get(&key).copied() {
            #[cfg(test)]
            {
                self.hits += 1;
            }
            return route;
        }
        let route = classify_op_dispatch_route(kind);
        self.routes.insert(key, route);
        route
    }
}

#[cfg(test)]
pub(crate) fn op_dispatch_route_cache_probe(kinds: &[KernelOpKind]) -> (bool, usize) {
    let mut cache = OpDispatchRouteCache::default();
    let mut parity = true;
    for kind in kinds {
        let uncached = classify_op_dispatch_route(kind);
        let cached = cache.route(kind);
        parity &= uncached == cached;
    }
    (parity, cache.hits)
}

fn classify_op_dispatch_route(kind: &KernelOpKind) -> OpDispatchRoute {
    match kind {
        KernelOpKind::Literal => OpDispatchRoute::Literal,
        KernelOpKind::LocalInvocationId => OpDispatchRoute::LocalInvocationId,
        KernelOpKind::GlobalInvocationId => OpDispatchRoute::GlobalInvocationId,
        KernelOpKind::WorkgroupId => OpDispatchRoute::WorkgroupId,
        KernelOpKind::SubgroupLocalId => OpDispatchRoute::SubgroupLocalId,
        KernelOpKind::SubgroupSize => OpDispatchRoute::SubgroupSize,
        KernelOpKind::LoopIndex { .. } => OpDispatchRoute::LoopIndex,
        KernelOpKind::BufferLength => OpDispatchRoute::BufferLength,
        KernelOpKind::LoadGlobal | KernelOpKind::LoadShared | KernelOpKind::LoadConstant => {
            OpDispatchRoute::Load
        }
        KernelOpKind::StoreGlobal | KernelOpKind::StoreShared => OpDispatchRoute::Store,
        KernelOpKind::Copy => OpDispatchRoute::Copy,
        KernelOpKind::BinOpKind(_) => OpDispatchRoute::BinOpKind,
        KernelOpKind::UnOpKind(_) => OpDispatchRoute::UnOpKind,
        KernelOpKind::Cast { .. } => OpDispatchRoute::Cast,
        KernelOpKind::Select => OpDispatchRoute::Select,
        KernelOpKind::Fma => OpDispatchRoute::Fma,
        KernelOpKind::StructuredIfThen => OpDispatchRoute::StructuredIfThen,
        KernelOpKind::StructuredIfThenElse => OpDispatchRoute::StructuredIfThenElse,
        KernelOpKind::StructuredBlock | KernelOpKind::Region { .. } => {
            OpDispatchRoute::StructuredBlock
        }
        KernelOpKind::StructuredForLoop { .. } => OpDispatchRoute::StructuredForLoop,
        KernelOpKind::AsyncLoad { .. } => OpDispatchRoute::AsyncLoad,
        KernelOpKind::AsyncStore { .. } => OpDispatchRoute::AsyncStore,
        KernelOpKind::AsyncWait { .. } => OpDispatchRoute::AsyncWait,
        KernelOpKind::Trap { .. } => OpDispatchRoute::Trap,
        KernelOpKind::Resume { .. } => OpDispatchRoute::Resume,
        KernelOpKind::Barrier { .. } => OpDispatchRoute::Barrier,
        KernelOpKind::Return => OpDispatchRoute::Return,
        KernelOpKind::SubgroupBallot => OpDispatchRoute::SubgroupBallot,
        KernelOpKind::SubgroupReduce { .. } => OpDispatchRoute::SubgroupReduce,
        KernelOpKind::SubgroupShuffle => OpDispatchRoute::SubgroupShuffle,
        KernelOpKind::SubgroupBroadcast => OpDispatchRoute::SubgroupBroadcast,
        KernelOpKind::Atomic { .. } => OpDispatchRoute::Atomic,
        KernelOpKind::IndirectDispatch { .. } => OpDispatchRoute::IndirectDispatch,
        KernelOpKind::MatrixMma { .. } => OpDispatchRoute::MatrixMma,
        KernelOpKind::Call { .. } => OpDispatchRoute::Call,
        KernelOpKind::OpaqueExpr(_) => OpDispatchRoute::OpaqueExpr,
        KernelOpKind::OpaqueNode(_) => OpDispatchRoute::OpaqueNode,
        KernelOpKind::LoopCarrierInit { .. } => OpDispatchRoute::LoopCarrierInit,
        KernelOpKind::LoopCarrier { .. } => OpDispatchRoute::LoopCarrier,
        KernelOpKind::LoopCarrierEnd { .. } => OpDispatchRoute::LoopCarrierEnd,
    }
}

macro_rules! with_route_kind {
    ($op:expr, $route:expr, $pattern:pat => $body:expr) => {
        match &$op.kind {
            $pattern => $body,
            _ => Err(route_mismatch($route)),
        }
    };
}

fn route_mismatch(route: OpDispatchRoute) -> EmitError {
    let mut message: String = Default::default();
    message.push_str("internal Naga op-dispatch route mismatch for ");
    let _ = write!(&mut message, "{route:?}");
    EmitError::InvalidDescriptor(message)
}

fn missing_literal_pool_index_message(literal_index: u32) -> String {
    let mut message: String = Default::default();
    message.push_str("literal op references missing literal-pool index ");
    let _ = write!(&mut message, "{literal_index}");
    message
}

fn missing_binding_slot_message(kind: &KernelOpKind) -> String {
    let mut message: String = Default::default();
    let _ = write!(&mut message, "{kind:?} missing binding slot");
    message
}

fn non_byte_load_route_message(data_type: DataType) -> String {
    let mut message: String = Default::default();
    message.push_str("emit_byte_element_load called with non-byte DataType ");
    let _ = write!(&mut message, "{data_type:?}");
    message.push_str("; this is an emitter routing bug");
    message
}

fn call_reached_message(op_id: &str) -> String {
    let mut message: String = Default::default();
    message.push_str("Call op `");
    message.push_str(op_id);
    message.push_str(
        "` reached descriptor Naga emission. Fix: expand calls into KernelDescriptor ops before emission.",
    );
    message
}

fn opaque_node_message(extension_kind: &str, payload_len: usize) -> String {
    let mut message: String = Default::default();
    message.push_str("opaque node `");
    message.push_str(extension_kind);
    message.push_str("` with ");
    let _ = write!(&mut message, "{payload_len}");
    message.push_str(
        " payload bytes has no descriptor Naga lowering. Fix: lower this extension into concrete KernelDescriptor ops before descriptor emission.",
    );
    message
}

fn wide_literal_payload_message(extension_kind: &str, payload_len: usize) -> String {
    let mut message: String = Default::default();
    message.push_str("wide-literal opaque `");
    message.push_str(extension_kind);
    message.push_str("` carries ");
    let _ = write!(&mut message, "{payload_len}");
    message.push_str(
        " payload bytes, expected 8. Fix: encode literals through Expr::u64/i64/f64 builders.",
    );
    message
}

fn wide_literal_kind_gate_message(kind: &str) -> String {
    let mut message: String = Default::default();
    message.push_str("wide-literal kind `");
    message.push_str(kind);
    message.push_str(
        "` reached descriptor opaque emission after the kind gate. Fix: update the kind gate and decoder together.",
    );
    message
}

fn opaque_expression_message(extension_kind: &str, extension_id: u32) -> String {
    let mut message: String = Default::default();
    message.push_str("opaque expression `");
    message.push_str(extension_kind);
    message.push_str("` (id=");
    let _ = write!(&mut message, "{extension_id:#010x}");
    message.push_str(
        ") has no descriptor Naga lowering. Fix: lower this extension into concrete KernelDescriptor ops or add a descriptor extension emitter before Naga emission.",
    );
    message
}

impl BodyBuilder<'_> {
    pub(super) fn emit_body(&mut self, body: &KernelBody) -> Result<(), EmitError> {
        for op in &body.ops {
            self.emit_op(body, op)?;
            // A `Trap` lowers to an unconditional `Return` for every lane (see
            // emit_trap), and a `Return` terminates the block. naga rejects any
            // statement after a `Return` in the same block
            // (`InstructionsAfterReturn`), so stop emitting this block's
            // remaining ops — they are unreachable by the trap/return semantics.
            if matches!(op.kind, KernelOpKind::Trap { .. } | KernelOpKind::Return) {
                break;
            }
        }
        Ok(())
    }

    pub(super) fn emit_op(&mut self, body: &KernelBody, op: &KernelOp) -> Result<(), EmitError> {
        let route = self.op_dispatch_routes.route(&op.kind);
        match route {
            OpDispatchRoute::Literal => {
                let literal_index = *op.operands.first().ok_or_else(|| {
                    EmitError::InvalidDescriptor("literal op missing literal-pool index".into())
                })?;
                let literal = body.literals.get(literal_index as usize).ok_or_else(|| {
                    EmitError::InvalidDescriptor(missing_literal_pool_index_message(literal_index))
                })?;
                let handle = if let LiteralValue::F32(value) = literal {
                    if value.is_finite() {
                        self.append_expr(Expression::Literal(naga_literal(literal)?))
                    } else {
                        // Naga's `Literal::F32` rejects NaN/Inf even though
                        // WGSL can represent the exact bit pattern via
                        // `bitcast<f32>(u32_bits)`. Preserve the IR literal
                        // byte-for-byte instead of weakening ops that use
                        // `-inf` as a sentinel, e.g. top-k initializers.
                        let bits = self.append_expr(Expression::Literal(Literal::U32(
                            value.to_bits(),
                        )));
                        self.append_expr(Expression::As {
                            expr: bits,
                            kind: ScalarKind::Float,
                            convert: None,
                        })
                    }
                } else {
                    self.append_expr(Expression::Literal(naga_literal(literal)?))
                };
                let ty = self.literal_type(literal);
                self.bind_result_typed(op, handle, ty)
            }
            OpDispatchRoute::LocalInvocationId => self.emit_builtin_axis(op, self.builtins.local),
            OpDispatchRoute::GlobalInvocationId => self.emit_builtin_axis(op, self.builtins.global),
            OpDispatchRoute::WorkgroupId => self.emit_builtin_axis(op, self.builtins.workgroup),
            OpDispatchRoute::SubgroupLocalId => {
                self.emit_scalar_builtin(op, self.builtins.subgroup_local, "SubgroupLocalId")
            }
            OpDispatchRoute::SubgroupSize => {
                self.emit_scalar_builtin(op, self.builtins.subgroup_size, "SubgroupSize")
            }
            OpDispatchRoute::LoopIndex => with_route_kind!(
                op,
                route,
                KernelOpKind::LoopIndex { loop_var } => self.emit_loop_index(op, loop_var)
            ),
            OpDispatchRoute::BufferLength => {
                let slot = *op.operands.first().ok_or_else(|| {
                    EmitError::InvalidDescriptor("BufferLength op missing binding slot".into())
                })?;
                let value = self.buffer_len_expr(slot)?;
                self.bind_result_typed(op, value, self.types.u32_ty)
            }
            OpDispatchRoute::Load => {
                let slot = *op.operands.first().ok_or_else(|| {
                    EmitError::InvalidDescriptor(missing_binding_slot_message(&op.kind))
                })?;
                // Byte-element bindings (U8/I8) are packed into array<u32>
                // by the WGSL emitter (no native byte storage). The IR-level
                // index is a byte address (matching reference-eval); extract
                // the right lane from the loaded word so the wire-correct
                // byte reaches the consumer.
                let data_type = self.binding_data_types.get(&slot).cloned();
                if let Some(dt @ (DataType::U8 | DataType::I8)) = data_type {
                    return self.emit_byte_element_load(op, slot, dt);
                }
                let pointer = self.binding_element_pointer(op, 0, 1)?;
                let value = self.append_expr(Expression::Load { pointer });
                let ty =
                    *self
                        .binding_types
                        .get(&slot)
                        .ok_or_else(|| EmitError::InvalidBinding {
                            slot,
                            reason: "no scalar type was recorded for this slot".into(),
                        })?;
                self.bind_result_typed(op, value, ty)
            }
            OpDispatchRoute::Store => {
                let slot = self.slot_operand(op, 0)?;
                // Byte-element bindings (U8/I8) need a read-modify-write
                // through the array<u32> word so the byte at `index`
                // changes without clobbering the three adjacent bytes
                // packed into the same u32. Naive Store would write the
                // value as a u32 to the byte address, corrupting the
                // surrounding bytes  -  the same byte/word-addressing
                // mismatch the LoadGlobal byte-extract path closed.
                let data_type = self.binding_data_types.get(&slot).cloned();
                if matches!(data_type, Some(DataType::U8) | Some(DataType::I8)) {
                    return self.emit_byte_element_store(op, slot);
                }
                let pointer = self.binding_element_pointer(op, 0, 1)?;
                let raw_value = self.value_operand(op, 2)?;
                let value = match self.binding_types.get(&slot).copied() {
                    Some(ty) => self.coerce_value_to_type(raw_value, ty),
                    None => raw_value,
                };
                self.function
                    .body
                    .push(Statement::Store { pointer, value }, Span::UNDEFINED);
                Ok(())
            }
            OpDispatchRoute::Copy => {
                let value = self.value_operand(op, 0)?;
                let ty = self.value_type_operand(op, 0)?;
                let local = self.function.local_variables.append(
                    LocalVariable {
                        name: None,
                        ty,
                        init: None,
                    },
                    Span::UNDEFINED,
                );
                let value = self.coerce_value_to_type(value, ty);
                let pointer = self.append_expr(Expression::LocalVariable(local));
                self.function
                    .body
                    .push(Statement::Store { pointer, value }, Span::UNDEFINED);
                let pointer = self.append_expr(Expression::LocalVariable(local));
                let snapshot = self.append_expr(Expression::Load { pointer });
                self.bind_result_typed(op, snapshot, ty)
            }
            OpDispatchRoute::BinOpKind => with_route_kind!(
                op,
                route,
                KernelOpKind::BinOpKind(binop) => self.emit_binop(op, *binop)
            ),
            OpDispatchRoute::UnOpKind => with_route_kind!(op, route, KernelOpKind::UnOpKind(unop) => {
                let expr = self.value_operand(op, 0)?;
                let ty = match unop {
                    UnOp::LogicalNot | UnOp::IsNan | UnOp::IsInf | UnOp::IsFinite => {
                        self.types.bool_ty
                    }
                    _ => self.value_type_operand(op, 0)?,
                };
                // 64-bit gate (mirrors the binop gate): U64/I64 are backed by
                // vec2<u32>. A Naga unary applied to the pair runs PER-WORD, so
                // popcount/clz/ctz/reverse/negate on a 64-bit value would be
                // SILENTLY WRONG (popcount/clz/ctz count a single word;
                // reverse_bits reverses each word without swapping them; negate
                // carries no borrow). Only bitwise NOT is correct componentwise.
                // Fail closed (Law 10) rather than emit a per-word result.
                let operand_is_u64 = self
                    .value_type_operand(op, 0)
                    .map(|h| h == self.types.vec2_u32_ty)
                    .unwrap_or(false);
                if operand_is_u64 && !matches!(unop, UnOp::BitNot) {
                    return Err(EmitError::NagaConstructionFailed(format!(
                        "64-bit (U64/I64) unary `{unop:?}` is not lowered: the \
                         vec2<u32> backing would apply it per-word, so the 64-bit \
                         result would be silently wrong (popcount/clz/ctz count a \
                         single word; reverse_bits does not swap words; negate \
                         carries no borrow). Only bitwise NOT is correct \
                         componentwise on a 64-bit value. Fix: add a cross-word \
                         U64 emulation pass before this op reaches Naga emission."
                    )));
                }
                // Naga's `LogicalNot` requires a Bool operand. When the
                // operand was published via a u32 carrier local (e.g. a
                // bool result that was bind_result_typed as u32 because
                // an upstream op flagged it as numeric), the cached Load
                // returns u32 and naga rejects with
                // `InvalidUnaryOperandType(LogicalNot, ...)`. Coerce
                // explicitly via the same path used for `if` conditions.
                let expr = if matches!(unop, UnOp::LogicalNot) {
                    self.ensure_bool_condition(expr)
                } else {
                    expr
                };
                let value = if matches!(unop, UnOp::Reciprocal) {
                    let one = self.append_expr(Expression::Literal(Literal::F32(1.0)));
                    self.append_expr(Expression::Binary {
                        op: BinaryOperator::Divide,
                        left: one,
                        right: expr,
                    })
                } else if matches!(unop, UnOp::IsNan) {
                    self.append_expr(Expression::Binary {
                        op: BinaryOperator::NotEqual,
                        left: expr,
                        right: expr,
                    })
                } else if matches!(unop, UnOp::IsInf | UnOp::IsFinite) {
                    let abs = self.append_expr(Expression::Math {
                        fun: naga::MathFunction::Abs,
                        arg: expr,
                        arg1: None,
                        arg2: None,
                        arg3: None,
                    });
                    let max = self.append_expr(Expression::Literal(Literal::F32(f32::MAX)));
                    let op = if matches!(unop, UnOp::IsFinite) {
                        BinaryOperator::LessEqual
                    } else {
                        BinaryOperator::Greater
                    };
                    self.append_expr(Expression::Binary {
                        op,
                        left: abs,
                        right: max,
                    })
                } else if let Some((shift, mask)) = unpack_shift_mask(unop) {
                    // Nibble/byte unpack has no Naga intrinsic; lower to an
                    // explicit `(value >> shift) & mask` on u32 (semantics match
                    // ir_eval). Without this the emitter rejected with "unary op
                    // `Unpack4Low` has no direct Naga unary operator".
                    //
                    // Unpack is UNSIGNED bit extraction. Reinterpret the source
                    // to u32 first so a signed source (e.g. a load from an i32
                    // buffer, whose kind does not resolve through the
                    // `Load(Access)` chain) does not emit `ShiftRight(i32, u32)`
                    // / `And(i32, u32)`, which naga rejects. A source already
                    // known to be u32 is left untouched (no redundant bitcast).
                    let expr = if matches!(
                        self.scalar_kind_of_expression(expr, 0),
                        Some(ScalarKind::Uint)
                    ) {
                        expr
                    } else {
                        self.append_expr(Expression::As {
                            expr,
                            kind: ScalarKind::Uint,
                            convert: None,
                        })
                    };
                    let shifted = if shift == 0 {
                        expr
                    } else {
                        let shift_lit =
                            self.append_expr(Expression::Literal(Literal::U32(shift)));
                        self.append_expr(Expression::Binary {
                            op: BinaryOperator::ShiftRight,
                            left: expr,
                            right: shift_lit,
                        })
                    };
                    let mask_lit = self.append_expr(Expression::Literal(Literal::U32(mask)));
                    self.append_expr(Expression::Binary {
                        op: BinaryOperator::And,
                        left: shifted,
                        right: mask_lit,
                    })
                } else if let Some(fun) = unary_math_function(unop) {
                    self.append_expr(Expression::Math {
                        fun,
                        arg: expr,
                        arg1: None,
                        arg2: None,
                        arg3: None,
                    })
                } else if matches!(unop, UnOp::Negate)
                    && self.scalar_kind_of_expression(expr, 0) == Some(ScalarKind::Uint)
                {
                    // WGSL/naga reject unary minus on an UNSIGNED operand
                    // (`InvalidUnaryOperandType(Negate)`), but vyre's typecheck
                    // legalises integer `Negate` exactly for `u32` (the total,
                    // wrapping case — raw i32 negate is rejected upstream for its
                    // i32::MIN overflow), and the reference oracle defines it as
                    // `0u32.wrapping_sub(v)`. Emitting `Unary(Negate, u32)` made
                    // every u32 negate validate + run correctly on the CPU oracle
                    // yet HARD-FAIL at GPU dispatch — a front-end-accepts /
                    // backend-can't-emit coherence gap (Law 10). naga's `Subtract`
                    // on a Uint wraps in two's complement, so synthesize the
                    // unsigned negate as `0u - v`, matching the oracle exactly.
                    let zero = self.append_expr(Expression::Literal(Literal::U32(0)));
                    self.append_expr(Expression::Binary {
                        op: BinaryOperator::Subtract,
                        left: zero,
                        right: expr,
                    })
                } else {
                    let naga_op = unary_operator(unop)?;
                    self.append_expr(Expression::Unary { op: naga_op, expr })
                };
                self.bind_result_typed(op, value, ty)
            }),
            OpDispatchRoute::Cast => with_route_kind!(op, route, KernelOpKind::Cast { target } => {
                let expr = self.value_operand(op, 0)?;
                // A multi-word-backed SOURCE (U64/I64/Vec2U32 -> vec2<u32>,
                // Vec4U32 -> vec4<u32>) cannot go through the scalar `As` path or
                // the scalar-source widening path below: both assume a scalar
                // source, so a plain `As` over the whole vector (or coercing the
                // vector to u32) yields invalid WGSL (InvalidStoreTypes / Compose
                // arity). Lower EVERY cast from a wide source by extracting its
                // lanes explicitly, matching the reference + PTX:
                //   * 2-word target (U64/I64/Vec2U32) -> low two words;
                //   * Vec4U32 -> the four words (identity, vec4 source only);
                //   * scalar integer -> low word (lane 0), truncated, then the
                //     scalar cast reinterprets it (u64->u32 keeps the low 32 bits);
                //   * F32 -> reconstruct (low | high<<32) then ConvertUToF, so the
                //     high word is NOT dropped (matches the reference u64 as f32);
                //   * Bool -> truthy over ALL source words: OR the lanes, != 0.
                let source_lanes = self.value_type_operand(op, 0).ok().and_then(|h| {
                    if h == self.types.vec4_u32_ty {
                        Some(4u32)
                    } else if h == self.types.vec2_u32_ty {
                        Some(2u32)
                    } else {
                        None
                    }
                });
                if let Some(lanes) = source_lanes {
                    let lane_handles: Vec<_> = (0..lanes)
                        .map(|i| {
                            self.append_expr(Expression::AccessIndex { base: expr, index: i })
                        })
                        .collect();
                    let low = lane_handles[0];
                    match target {
                        DataType::U64 | DataType::I64 | DataType::Vec2U32 => {
                            let composed = self.append_expr(Expression::Compose {
                                ty: self.types.vec2_u32_ty,
                                components: vec![low, lane_handles[1]],
                            });
                            return self
                                .bind_result_typed(op, composed, self.types.vec2_u32_ty);
                        }
                        DataType::Vec4U32 => {
                            if lanes < 4 {
                                return Err(EmitError::NagaConstructionFailed(format!(
                                    "cast to Vec4U32 from a {lanes}-word source is not \
                                     representable: only a 4-word (Vec4U32) source can \
                                     widen to Vec4U32. Fix: route through a Vec2U32 \
                                     intermediate or zero-fill the upper lanes explicitly."
                                )));
                            }
                            let composed = self.append_expr(Expression::Compose {
                                ty: self.types.vec4_u32_ty,
                                components: lane_handles,
                            });
                            return self
                                .bind_result_typed(op, composed, self.types.vec4_u32_ty);
                        }
                        DataType::F32 => {
                            let low_u64 = self.append_expr(Expression::As {
                                expr: low,
                                kind: ScalarKind::Uint,
                                convert: Some(8),
                            });
                            let high_u64 = self.append_expr(Expression::As {
                                expr: lane_handles[1],
                                kind: ScalarKind::Uint,
                                convert: Some(8),
                            });
                            let shift = self
                                .append_expr(Expression::Literal(naga::Literal::U32(32)));
                            let shift_u64 = self.append_expr(Expression::As {
                                expr: shift,
                                kind: ScalarKind::Uint,
                                convert: Some(8),
                            });
                            let high_shifted = self.append_expr(Expression::Binary {
                                op: BinaryOperator::ShiftLeft,
                                left: high_u64,
                                right: shift_u64,
                            });
                            let full = self.append_expr(Expression::Binary {
                                op: BinaryOperator::InclusiveOr,
                                left: low_u64,
                                right: high_shifted,
                            });
                            let value = self.append_expr(Expression::As {
                                expr: full,
                                kind: ScalarKind::Float,
                                convert: Some(4),
                            });
                            let ty = self.type_for_data_type(target)?;
                            return self.bind_result_typed(op, value, ty);
                        }
                        DataType::Bool => {
                            let mut merged = low;
                            for &lane in &lane_handles[1..] {
                                merged = self.append_expr(Expression::Binary {
                                    op: BinaryOperator::InclusiveOr,
                                    left: merged,
                                    right: lane,
                                });
                            }
                            let zero = self
                                .append_expr(Expression::Literal(naga::Literal::U32(0)));
                            let value = self.append_expr(Expression::Binary {
                                op: BinaryOperator::NotEqual,
                                left: merged,
                                right: zero,
                            });
                            let ty = self.type_for_data_type(target)?;
                            return self.bind_result_typed(op, value, ty);
                        }
                        _ => {
                            let (kind, width) = scalar_cast_target(target)?;
                            let value = self.append_expr(Expression::As {
                                expr: low,
                                kind,
                                convert: Some(width),
                            });
                            // A 64-bit source narrowing to u8/u16/i8/i16 keeps
                            // only the low word's low bits — truncate the `As`
                            // result to the narrow target width (matches the
                            // oracle's `u64 as u8` low-byte semantics).
                            let value = self.apply_narrow_mask(value, target);
                            let ty = self.type_for_data_type(target)?;
                            return self.bind_result_typed(op, value, ty);
                        }
                    }
                }
                // Detect a float source via BOTH the bound type handle AND the
                // scalar-kind resolver: `value_type_operand == f32_ty` catches
                // buffer loads (scalar_kind_of_expression returns None through an
                // Access->Load chain), while `scalar_kind_of_expression == Float`
                // catches computed floats (arithmetic results) whose type handle
                // may not be a literal `f32_ty`. Either alone leaves a silent-skip
                // hole. Neither over-matches a non-float.
                let source_is_f32 = self
                    .value_type_operand(op, 0)
                    .map(|h| h == self.types.f32_ty)
                    .unwrap_or(false)
                    || self.scalar_kind_of_expression(expr, 0) == Some(ScalarKind::Float);
                // A float source converts numerically ONLY to u32/i32 (saturating,
                // below), bool (truthy), or f32 (identity). The foundation cast
                // table (`validate::cast::cast_is_valid`) rejects f32 -> {U8, U16,
                // I8, I16, U64, I64, Vec2U32, Vec4U32} for exactly this reason. But
                // the Program-compat `emit_module` path does NOT run full
                // validation, so such a cast can reach here — and the paths below
                // would SILENTLY miscompile it: the U64/I64/Vec2U32 wide path
                // reinterprets the float through a u32 coerce (dropping the high
                // word), and a narrow int target (U8/U16/I8/I16, all backed by a
                // 32-bit scalar) takes a bare `As` that skips the saturating guard
                // (NaN -> undefined, overflow -> FClamp divergence) AND does not
                // narrow. Fail closed (Law 10), mirroring the `Bytes` arm in
                // `scalar_cast_target`, and name the fix.
                if source_is_f32
                    && matches!(
                        target,
                        DataType::U8
                            | DataType::U16
                            | DataType::I8
                            | DataType::I16
                            | DataType::U64
                            | DataType::I64
                            | DataType::Vec2U32
                            | DataType::Vec4U32
                    )
                {
                    return Err(EmitError::NagaConstructionFailed(format!(
                        "cast from f32 to `{target:?}` has no defined conversion: a \
                         float source converts only to u32/i32 (saturating), bool \
                         (truthy), or f32. Fix: cast the f32 to u32 or i32 first, then \
                         narrow or widen the integer."
                    )));
                }
                if matches!(target, DataType::U64 | DataType::I64 | DataType::Vec2U32) {
                    // WGSL has no native 64-bit integer; U64/I64 are backed by
                    // vec2<u32> (low word `.x`, high word `.y`). The low word is
                    // always the source's 32-bit pattern. The HIGH word depends
                    // on what the cast means:
                    //   * `Vec2U32` is a STRUCTURAL 2-word vector — lane 1 is
                    //     zero-filled (matches the reference `widen_to_words` /
                    //     `cast_to_vec2` zero-pad), never sign-extended.
                    //   * `U64`/`I64` are 64-bit INTEGERS — the high word must
                    //     extend per the SOURCE's signedness. A signed (i32)
                    //     source SIGN-extends so a negative value carries its
                    //     full two's-complement high word (matching the PTX
                    //     `cvt.s64.s32` path and Rust `i32 as i64`); an unsigned
                    //     source zero-extends. Zeroing the high word
                    //     unconditionally — as this did before — silently turned
                    //     every negative `i32 -> i64/u64` into a large positive
                    //     value (Law 10 miscompile). This stays componentwise
                    //     (the high word is derived from the low word's sign bit,
                    //     no cross-lane carry), unlike 64-bit arithmetic which
                    //     emit_binop still rejects until a carry pass lands.
                    let src_is_signed = matches!(
                        self.scalar_kind_of_expression(expr, 0),
                        Some(ScalarKind::Sint)
                    );
                    let low = self.coerce_value_to_type(expr, self.types.u32_ty);
                    let high = if src_is_signed
                        && matches!(target, DataType::U64 | DataType::I64)
                    {
                        // sign_bit = low >> 31 (logical shift on a u32 → 0 or 1);
                        // high = sign_bit * 0xFFFF_FFFF → 0x0000_0000 when the
                        // sign bit is clear, 0xFFFF_FFFF when set. No branch, no
                        // carry — a pure componentwise sign replicate.
                        let thirty_one =
                            self.append_expr(Expression::Literal(naga::Literal::U32(31)));
                        let sign_bit = self.append_expr(Expression::Binary {
                            op: BinaryOperator::ShiftRight,
                            left: low,
                            right: thirty_one,
                        });
                        let all_ones = self
                            .append_expr(Expression::Literal(naga::Literal::U32(0xFFFF_FFFF)));
                        self.append_expr(Expression::Binary {
                            op: BinaryOperator::Multiply,
                            left: sign_bit,
                            right: all_ones,
                        })
                    } else {
                        self.append_expr(Expression::Literal(naga::Literal::U32(0)))
                    };
                    let value = self.append_expr(Expression::Compose {
                        ty: self.types.vec2_u32_ty,
                        components: vec![low, high],
                    });
                    return self.bind_result_typed(op, value, self.types.vec2_u32_ty);
                }
                let (kind, width) = scalar_cast_target(target)?;
                // Float->{U32,I32}: naga's SPIR-V backend lowers `As` as
                // `FClamp(x, min_repr, max_repr)` then ConvertFTo{U,S}. That
                // DIVERGES from the reference oracle (Rust saturating `as`) two
                // ways, both confirmed on a live 5090:
                //   1. OVERFLOW: FClamp pins to the largest *f32-representable*
                //      value <= target max (i32 -> 2147483520, u32 -> 4294967040),
                //      but the oracle + PTX `cvt.rzi.sat` saturate to the exact
                //      INTEGER max (i32::MAX, u32::MAX). Diverges for EVERY
                //      positive overflow including +inf.
                //   2. NaN: `FClamp(NaN, ..)` is SPIR-V-undefined (observed
                //      i32::MIN on the 5090); the oracle defines NaN -> 0.
                // Restore the reference's saturating semantics explicitly:
                // NaN -> 0, x >= 2^bits -> INT_MAX. The in-range and
                // negative-overflow paths already match (min_repr is exactly
                // f32-representable, so naga's FClamp low bound == the oracle),
                // so they keep naga's `As`. Only a Float source to a 32-bit int
                // target is rewritten; integer->int and narrow targets are
                // unchanged.
                //
                // `source_is_f32` was computed once above (it also gates the
                // fail-closed guard that rejects float -> non-{u32,i32,bool,f32}
                // targets), so by here the only float targets left are U32/I32.
                if source_is_f32 && matches!(target, DataType::U32 | DataType::I32) {
                    let converted = self.append_expr(Expression::As {
                        expr,
                        kind,
                        convert: Some(width),
                    });
                    // is_finite_ordered = (x == x): TRUE for every non-NaN value,
                    // FALSE for NaN. We must NOT use `x != x` here: naga lowers a
                    // float `!=` to the ORDERED `FOrdNotEqual`, and
                    // `FOrdNotEqual(NaN, NaN)` is FALSE (ordered comparisons are
                    // false whenever an operand is NaN) — the canonical `x != x`
                    // NaN idiom is silently dead in naga. `Equal` lowers to
                    // `FOrdEqual`, and `FOrdEqual(NaN, NaN)` is likewise FALSE, so
                    // `x == x` is the portable not-NaN predicate.
                    let is_not_nan = self.append_expr(Expression::Binary {
                        op: BinaryOperator::Equal,
                        left: expr,
                        right: expr,
                    });
                    let (overflow_threshold, int_max, int_zero) = match target {
                        DataType::I32 => (
                            naga::Literal::F32(2_147_483_648.0), // 2^31
                            naga::Literal::I32(i32::MAX),
                            naga::Literal::I32(0),
                        ),
                        // U32 (the only other arm the outer `matches!` allows).
                        _ => (
                            naga::Literal::F32(4_294_967_296.0), // 2^32
                            naga::Literal::U32(u32::MAX),
                            naga::Literal::U32(0),
                        ),
                    };
                    let threshold = self.append_expr(Expression::Literal(overflow_threshold));
                    // too_high = (x >= 2^bits): true for positive overflow and
                    // +inf; false for NaN (ordered compare is false for NaN), so
                    // the outer not-NaN select still wins for NaN regardless of
                    // `converted`/`saturated_high`.
                    let too_high = self.append_expr(Expression::Binary {
                        op: BinaryOperator::GreaterEqual,
                        left: expr,
                        right: threshold,
                    });
                    let max_lit = self.append_expr(Expression::Literal(int_max));
                    let saturated_high = self.append_expr(Expression::Select {
                        condition: too_high,
                        accept: max_lit,
                        reject: converted,
                    });
                    let zero_lit = self.append_expr(Expression::Literal(int_zero));
                    // non-NaN -> the saturated conversion; NaN -> 0 (the oracle's
                    // `NaN as {i32,u32}`).
                    let value = self.append_expr(Expression::Select {
                        condition: is_not_nan,
                        accept: saturated_high,
                        reject: zero_lit,
                    });
                    let ty = self.type_for_data_type(target)?;
                    return self.bind_result_typed(op, value, ty);
                }
                let value = self.append_expr(Expression::As {
                    expr,
                    kind,
                    convert: Some(width),
                });
                // Integer narrowing (u32 -> u8/u16/i8/i16): the `As` above keeps
                // the full 32-bit word; mask/sign-extend to the target width so
                // the emitter matches Rust `as` and the reference oracle.
                let value = self.apply_narrow_mask(value, target);
                let ty = self.type_for_data_type(target)?;
                self.bind_result_typed(op, value, ty)
            }),
            OpDispatchRoute::Select => {
                let condition = self.value_operand(op, 0)?;
                let accept = self.value_operand(op, 1)?;
                let reject = self.value_operand(op, 2)?;
                let condition = self.ensure_bool_condition(condition);
                let ty = self.value_type_operand(op, 1)?;
                // Coerce reject to accept's scalar type. Without this,
                // when accept and reject were each `bind_result_typed`-d
                // with different scalar kinds (e.g. accept=u32 from a
                // numeric op, reject=bool from a comparison), naga
                // rejects the Select with `SelectValuesTypeMismatch`.
                // The pre-publish path masked this by inlining one arm
                // as a literal; explicit `LocalVariable + Load` round-
                // tripping (Q7 carrier mechanism) exposes the mismatch.
                let reject = self.coerce_value_to_type(reject, ty);
                let accept = self.coerce_value_to_type(accept, ty);
                let value = self.append_expr(Expression::Select {
                    condition,
                    accept,
                    reject,
                });
                self.bind_result_typed(op, value, ty)
            }
            OpDispatchRoute::Fma => {
                let arg = self.value_operand(op, 0)?;
                let arg1 = Some(self.value_operand(op, 1)?);
                let arg2 = Some(self.value_operand(op, 2)?);
                let value = self.append_expr(Expression::Math {
                    fun: naga::MathFunction::Fma,
                    arg,
                    arg1,
                    arg2,
                    arg3: None,
                });
                let ty = self.value_type_operand(op, 0)?;
                self.bind_result_typed(op, value, ty)
            }
            OpDispatchRoute::StructuredIfThen => {
                self.emit_structured_if(body, op, &[1])
            }
            OpDispatchRoute::StructuredIfThenElse => {
                self.emit_structured_if(body, op, &[1, 2])
            }
            OpDispatchRoute::StructuredBlock => {
                self.emit_structured_block(body, op)
            }
            OpDispatchRoute::StructuredForLoop => with_route_kind!(
                op,
                route,
                KernelOpKind::StructuredForLoop { loop_var } => {
                self.emit_structured_for_loop(body, op, loop_var)
                }
            ),
            OpDispatchRoute::AsyncLoad => self.emit_async_load(op),
            OpDispatchRoute::AsyncStore => self.emit_async_store(op),
            // AsyncWait is a documented no-op in the Naga backend. The Naga
            // backend lowers AsyncLoad and AsyncStore as fully synchronous
            // counted copy loops — the copy completes before the next op
            // executes. There is no deferred or out-of-order DMA in this path,
            // so no fence or barrier is needed: the copy is already done.
            // Backends that use real hardware async DMA (e.g. PTX cp.async)
            // must emit a hardware-level wait instruction here.
            OpDispatchRoute::AsyncWait => Ok(()),
            OpDispatchRoute::Trap => with_route_kind!(
                op,
                route,
                KernelOpKind::Trap { tag } => self.emit_trap(op, tag)
            ),
            // Resume is a runtime sequencing marker that the Naga backend
            // treats as a no-op. The Trap protocol in this backend emits an
            // unconditional Return after the sidecar write (see emit_trap),
            // so any statements after a Trap are not executed. Resume carries
            // sequencing intent for higher-level passes (scheduling, analysis)
            // but does not map to a Naga IR statement. On backends with real
            // continuations (e.g. PTX setmaxnreg + bar.sync) this must emit
            // the continuation resume instruction.
            OpDispatchRoute::Resume => Ok(()),
            OpDispatchRoute::Barrier => with_route_kind!(op, route, KernelOpKind::Barrier { ordering } => {
                let barrier = barrier_flags(*ordering)?;
                self.function
                    .body
                    .push(Statement::Barrier(barrier), Span::UNDEFINED);
                Ok(())
            }),
            OpDispatchRoute::Return => {
                self.function
                    .body
                    .push(Statement::Return { value: None }, Span::UNDEFINED);
                Ok(())
            }
            OpDispatchRoute::SubgroupBallot => {
                self.emit_subgroup_ballot(op)
            }
            OpDispatchRoute::SubgroupReduce => {
                self.emit_subgroup_reduce(op)
            }
            OpDispatchRoute::SubgroupShuffle => {
                self.emit_subgroup_shuffle(op)
            }
            OpDispatchRoute::SubgroupBroadcast => {
                self.emit_subgroup_broadcast(op)
            }
            OpDispatchRoute::Atomic => with_route_kind!(op, route, KernelOpKind::Atomic {
                op: atomic_op,
                ordering: _,
            } => {
                self.emit_atomic(op, *atomic_op)
            }),
            // IndirectDispatch has no Naga lowering. Naga compute shaders fix
            // the workgroup size in the @workgroup_size attribute at
            // compile time. Writing a dispatch-count buffer at runtime
            // (the IndirectDispatch semantic) is not a shader-internal
            // operation in the WGSL/Naga model — it must be done by the
            // host before launching the next dispatch. Fix: perform the
            // indirect count buffer write on the host side (or via a
            // separate count-kernel dispatch) rather than embedding it in
            // the main compute shader.
            OpDispatchRoute::IndirectDispatch => Err(EmitError::InvalidDescriptor(
                "IndirectDispatch reached the Naga emitter. Naga compute shaders cannot write \
                 dispatch count buffers from within a shader; the workgroup size is fixed at \
                 compile time. Fix: compute and write the indirect count buffer on the host, or \
                 via a dedicated count-kernel dispatch, before launching the indirect dispatch."
                    .into(),
            )),
            OpDispatchRoute::MatrixMma => Err(EmitError::InvalidDescriptor(
                "MatrixMma reached descriptor Naga emission. Fix: route MatrixMma through a concrete tensor-core backend or lower it before Naga emission.".into(),
            )),
            OpDispatchRoute::Call => with_route_kind!(
                op,
                route,
                KernelOpKind::Call { op_id } => {
                    Err(EmitError::InvalidDescriptor(call_reached_message(op_id.as_ref())))
                }
            ),
            OpDispatchRoute::OpaqueExpr => with_route_kind!(op, route, KernelOpKind::OpaqueExpr(data) => {
                self.emit_opaque_expr(op, data.extension_id, &data.extension_kind, &data.payload)
            }),
            OpDispatchRoute::OpaqueNode => with_route_kind!(
                op,
                route,
                KernelOpKind::OpaqueNode(data) => Err(EmitError::InvalidDescriptor(
                    opaque_node_message(&data.extension_kind, data.payload.len())
                ))
            ),
            OpDispatchRoute::LoopCarrierInit => with_route_kind!(
                op,
                route,
                KernelOpKind::LoopCarrierInit { name } => self.emit_loop_carrier_init(op, name)
            ),
            OpDispatchRoute::LoopCarrier => with_route_kind!(
                op,
                route,
                KernelOpKind::LoopCarrier { name } => self.emit_loop_carrier_read(op, name)
            ),
            OpDispatchRoute::LoopCarrierEnd => with_route_kind!(
                op,
                route,
                KernelOpKind::LoopCarrierEnd { name } => self.emit_loop_carrier_end(op, name)
            ),
        }
    }

    /// Truncate a 32-bit scalar cast result to a narrow integer target's width.
    ///
    /// WGSL has no native 8/16-bit integer scalar, so `scalar_cast_target` backs
    /// U8/U16 with a `Uint` (u32) and I8/I16 with a `Sint` (i32) register. The
    /// bare `As` that produces that register does NOT discard the high bits — a
    /// u32 source to a U8 target stays the full word — so a narrowing cast would
    /// silently keep the high bits, diverging from the V035 contract ("narrowing
    /// cast may truncate high bits"), Rust `as u8/u16/i8/i16`, and the reference
    /// oracle (`cast_value`). This applies the missing truncation:
    ///   * U8/U16: bitwise-AND with the low-`width` mask (`& 0xFF` / `& 0xFFFF`).
    ///   * I8/I16: truncate then SIGN-extend from the new top bit via
    ///     `(x << shift) >> shift` with an arithmetic (Sint) right shift — the
    ///     same idiom `emit_byte_element_load` uses for an I8 buffer byte, so e.g.
    ///     `200 as i8 == -56`, `-1 as i8 == -1`. The signed `>>` lowers to a naga
    ///     arithmetic shift (the shift-emit fix keeps the Sint value's signedness
    ///     while forcing the amount to u32).
    /// A non-narrow target returns the value unchanged.
    fn apply_narrow_mask(
        &mut self,
        value: naga::Handle<Expression>,
        target: &DataType,
    ) -> naga::Handle<Expression> {
        match target {
            DataType::U8 | DataType::U16 => {
                let mask = if matches!(target, DataType::U8) {
                    0xFFu32
                } else {
                    0xFFFFu32
                };
                let mask_lit = self.append_expr(Expression::Literal(Literal::U32(mask)));
                self.append_expr(Expression::Binary {
                    op: BinaryOperator::And,
                    left: value,
                    right: mask_lit,
                })
            }
            DataType::I8 | DataType::I16 => {
                let shift = if matches!(target, DataType::I8) {
                    24u32
                } else {
                    16u32
                };
                let shift_lit = self.append_expr(Expression::Literal(Literal::U32(shift)));
                let shifted_left = self.append_expr(Expression::Binary {
                    op: BinaryOperator::ShiftLeft,
                    left: value,
                    right: shift_lit,
                });
                self.append_expr(Expression::Binary {
                    op: BinaryOperator::ShiftRight,
                    left: shifted_left,
                    right: shift_lit,
                })
            }
            _ => value,
        }
    }

    /// Emit `Statement::Block` for `StructuredBlock` / `Region` with the
    /// same Q7 carrier-publish machinery as `emit_structured_if` and
    /// `emit_structured_for_loop`. Any SSA id produced inside the
    /// region's child body that the parent body references after the
    /// region must round-trip through a function-local: the in-region
    /// `Statement::Emit` lives inside the closed inner block, and the
    /// post-region reader needs a fresh `Load` whose Emit lives in the
    /// parent block. Without this, naga's WGSL writer emits `let _eN =
    /// ...;` inside the inner block and the post-region read of `_eN`
    /// trips `no definition in scope` validation. The lowering's
    /// Region phi-merge handles source-level NAMED carriers; this
    /// handles UNNAMED in-region SSA results that escape  -  exactly the
    /// `vyre_loop_carry_<id>` carrier path Loop/If already use.
    pub(super) fn emit_structured_block(
        &mut self,
        body: &KernelBody,
        op: &KernelOp,
    ) -> Result<(), EmitError> {
        let prior_carriers = self.snapshot_loop_carriers();
        let op_pos = body
            .ops
            .iter()
            .position(|candidate| std::ptr::eq(candidate, op))
            .unwrap_or(body.ops.len());
        let child_body_idxs: Vec<u32> = op.operands.iter().take(1).copied().collect();
        let new_targets = self.collect_child_carried_ids(body, op_pos, &child_body_idxs);

        let mut pre_init: Vec<(u32, naga::Handle<Expression>)> = Vec::default();
        for id in &new_targets {
            self.loop_carrier_targets.insert(*id);
            if let Some(handle) = self.value_handle_for_id(*id) {
                pre_init.push((*id, handle));
            }
        }
        for (id, init_handle) in &pre_init {
            let local = self.allocate_carrier_local(*id, init_handle);
            let local_ty = self.function.local_variables[local].ty;
            let init = self.coerce_value_to_type(*init_handle, local_ty);
            let pointer = self.append_expr(Expression::LocalVariable(local));
            self.function.body.push(
                Statement::Store {
                    pointer,
                    value: init,
                },
                Span::UNDEFINED,
            );
        }

        let block = self.child_block(body, op, 0)?;
        self.function
            .body
            .push(Statement::Block(block), Span::UNDEFINED);

        for id in &new_targets {
            if let Some(local) = self.loop_carrier_locals.get(id).copied() {
                let pointer = self.append_expr(Expression::LocalVariable(local));
                let load = self.append_expr(Expression::Load { pointer });
                self.values.insert(*id, load);
            }
        }
        self.restore_loop_carriers(prior_carriers);
        Ok(())
    }

    /// Emit `Statement::If { accept, reject }` for `StructuredIfThen`
    /// (`child_indices=&[1]`) and `StructuredIfThenElse`
    /// (`child_indices=&[1, 2]`) with the same Q7 carrier-publish
    /// machinery that `emit_structured_for_loop` uses. Without the
    /// publish, any value bound inside the if-body and read after the
    /// if surfaces as `no definition in scope for identifier _eN` from
    /// naga's WGSL writer (the `let _eN = ...;` binding lives inside
    /// the if-body's scope; the post-if reader is outside it).
    pub(super) fn emit_structured_if(
        &mut self,
        body: &KernelBody,
        op: &KernelOp,
        child_indices: &[usize],
    ) -> Result<(), EmitError> {
        let prior_carriers = self.snapshot_loop_carriers();
        let op_pos = body
            .ops
            .iter()
            .position(|candidate| std::ptr::eq(candidate, op))
            .unwrap_or(body.ops.len());
        let child_body_idxs: Vec<u32> = child_indices
            .iter()
            .filter_map(|i| op.operands.get(*i).copied())
            .collect();
        let new_targets = self.collect_child_carried_ids(body, op_pos, &child_body_idxs);

        // Pre-if init: for any new carrier whose id had a prior SSA
        // value bound in the parent scope, seed the carrier local so a
        // reader inside the if (or after it on the not-taken path) sees
        // the pre-if value. value_handle_for_id materializes the prior
        // value via fresh Load when the cached handle's emit-block has
        // closed; otherwise it returns the cached handle directly.
        let mut pre_init: Vec<(u32, naga::Handle<Expression>)> = Vec::default();
        for id in &new_targets {
            self.loop_carrier_targets.insert(*id);
            if let Some(handle) = self.value_handle_for_id(*id) {
                pre_init.push((*id, handle));
            }
        }
        for (id, init_handle) in &pre_init {
            let local = self.allocate_carrier_local(*id, init_handle);
            let local_ty = self.function.local_variables[local].ty;
            let init = self.coerce_value_to_type(*init_handle, local_ty);
            let pointer = self.append_expr(Expression::LocalVariable(local));
            self.function.body.push(
                Statement::Store {
                    pointer,
                    value: init,
                },
                Span::UNDEFINED,
            );
        }

        let condition = self.value_operand(op, 0)?;
        let condition = self.ensure_bool_condition(condition);
        let accept = self.child_block(body, op, child_indices[0])?;
        let reject = if child_indices.len() > 1 {
            self.child_block(body, op, child_indices[1])?
        } else {
            naga::Block::new()
        };
        self.function.body.push(
            Statement::If {
                condition,
                accept,
                reject,
            },
            Span::UNDEFINED,
        );

        // Post-if rebind: re-Load every carrier from its function-scope
        // local in the parent block so any subsequent reader resolves
        // to a Load whose Statement::Emit is in the current (parent)
        // body  -  not the now-closed if-body's expression range.
        for id in &new_targets {
            if let Some(local) = self.loop_carrier_locals.get(id).copied() {
                let pointer = self.append_expr(Expression::LocalVariable(local));
                let load = self.append_expr(Expression::Load { pointer });
                self.values.insert(*id, load);
            }
        }
        self.restore_loop_carriers(prior_carriers);
        Ok(())
    }

    /// `BinOpKind` emit  -  bool-vs-numeric widening, literal-pool fold,
    /// and Math-builtin routing live here to keep `emit_op` flat.
    fn emit_binop(&mut self, op: &KernelOp, binop: BinOp) -> Result<(), EmitError> {
        let left = self.value_operand(op, 0)?;
        let right = self.value_operand(op, 1)?;
        // 64-bit gate: U64/I64 are backed by vec2<u32> (the vec2_u32_ty handle).
        // Componentwise bitwise AND/OR/XOR on the pair are mathematically
        // correct, but add/sub/mul/compare/shift need carry/borrow propagation
        // between the low and high words — a componentwise vec2 op would be
        // SILENTLY WRONG arithmetic. Fail closed (Law 10) rather than emit it.
        let lhs_is_u64 = self
            .value_type_operand(op, 0)
            .map(|h| h == self.types.vec2_u32_ty)
            .unwrap_or(false);
        let rhs_is_u64 = self
            .value_type_operand(op, 1)
            .map(|h| h == self.types.vec2_u32_ty)
            .unwrap_or(false);
        if (lhs_is_u64 || rhs_is_u64)
            && !matches!(binop, BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor)
        {
            return Err(EmitError::NagaConstructionFailed(format!(
                "64-bit (U64/I64) `{binop:?}` is not lowered: the vec2<u32> backing \
                 carries no carry/borrow between the low and high words, so a \
                 componentwise op would be silently wrong. Only bitwise AND/OR/XOR \
                 are supported on 64-bit values. Fix: add a carry-propagating U64 \
                 emulation pass before this op reaches Naga emission."
            )));
        }
        if let Some(folded) = self.fold_literal_binop(left, right, binop) {
            let ty = self.binary_result_type(op, binop)?;
            return self.bind_result_typed(op, folded, ty);
        }
        let mut effective_binop = binop;
        let mut left_eff = left;
        let mut right_eff = right;
        if matches!(
            binop,
            BinOp::And | BinOp::Or | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor
        ) {
            let left_ty = self.value_type_operand(op, 0).ok();
            let right_ty = self.value_type_operand(op, 1).ok();
            let left_naga_kind = self.scalar_kind_of_expression(left, 0);
            let right_naga_kind = self.scalar_kind_of_expression(right, 0);
            let left_is_bool = match left_naga_kind {
                Some(naga::ScalarKind::Bool) => true,
                Some(_) => false,
                None => match left_ty {
                    Some(ty) => ty == self.types.bool_ty,
                    None => self.is_bool_expression(left),
                },
            };
            let right_is_bool = match right_naga_kind {
                Some(naga::ScalarKind::Bool) => true,
                Some(_) => false,
                None => match right_ty {
                    Some(ty) => ty == self.types.bool_ty,
                    None => self.is_bool_expression(right),
                },
            };
            if left_is_bool && right_is_bool {
                // both bool → keep bool; binary_operator emits bitwise And/Or
            } else if !left_is_bool && !right_is_bool {
                // both numeric → bitwise as-is
            } else {
                let left_widen_ty = if left_is_bool {
                    Some(self.types.bool_ty)
                } else {
                    left_ty.or(Some(self.types.u32_ty))
                };
                let right_widen_ty = if right_is_bool {
                    Some(self.types.bool_ty)
                } else {
                    right_ty.or(Some(self.types.u32_ty))
                };
                left_eff = self.coerce_to_u32(left, left_widen_ty);
                right_eff = self.coerce_to_u32(right, right_widen_ty);
                effective_binop = match binop {
                    BinOp::And => BinOp::BitAnd,
                    BinOp::Or => BinOp::BitOr,
                    other => other,
                };
            }
        }
        let left_kind = self.scalar_kind_of_expression(left_eff, 0);
        let right_kind = self.scalar_kind_of_expression(right_eff, 0);
        // Comparison and arithmetic BinOps require numeric (non-Bool)
        // operands in WGSL. When the carrier-publish round-trip exposes
        // Bool-typed Loads on either arm, naga rejects with
        // `InvalidBinaryOperandTypes`. Coerce both arms to u32 for the
        // affected ops; Eq/Ne/And/Or are bool-friendly and are routed
        // through the bool-widening branch above.
        let comparison_or_arith = matches!(
            binop,
            BinOp::Lt
                | BinOp::Gt
                | BinOp::Le
                | BinOp::Ge
                | BinOp::Add
                | BinOp::Sub
                | BinOp::Mul
                | BinOp::Div
                | BinOp::Mod
                | BinOp::Shl
                | BinOp::Shr
                | BinOp::Min
                | BinOp::Max
                | BinOp::WrappingAdd
                | BinOp::WrappingSub
                | BinOp::AbsDiff
                | BinOp::RotateLeft
                | BinOp::RotateRight
                | BinOp::MulHigh
                | BinOp::SaturatingAdd
                | BinOp::SaturatingSub
                | BinOp::SaturatingMul
        );
        if comparison_or_arith {
            if matches!(left_kind, Some(naga::ScalarKind::Bool)) {
                left_eff = self.coerce_to_u32(left_eff, Some(self.types.bool_ty));
            }
            if matches!(right_kind, Some(naga::ScalarKind::Bool)) {
                right_eff = self.coerce_to_u32(right_eff, Some(self.types.bool_ty));
            }
        }
        let left_kind = self.scalar_kind_of_expression(left_eff, 0);
        let right_kind = self.scalar_kind_of_expression(right_eff, 0);
        // Shifts are the exception to operand-kind unification: the amount must
        // stay u32 regardless of the value's signedness (the value's signedness
        // selects arithmetic vs logical shift). Coercing the amount to the
        // value's kind here would force `i32 >> 1`'s amount to Sint; the shift
        // block below owns the amount's type (coerce-to-u32 + bit-width mask).
        let is_shift = matches!(effective_binop, BinOp::Shl | BinOp::Shr);
        if !is_shift {
            if let (Some(lk), Some(rk)) = (left_kind, right_kind) {
                if lk != rk {
                    let target = match lk {
                        naga::ScalarKind::Bool => self.types.bool_ty,
                        naga::ScalarKind::Sint => self.types.i32_ty,
                        naga::ScalarKind::Float => self.types.f32_ty,
                        _ => self.types.u32_ty,
                    };
                    right_eff = self.coerce_value_to_type(right_eff, target);
                }
            }
        }
        // Backend contract: the shift amount is taken modulo the bit width (32).
        // The reference oracle masks (`right & 31`) and PTX masks
        // (`and.b32 …,31`), but a bare naga ShiftLeft/ShiftRight leaves an
        // amount >= 32 undefined per the SPIR-V/WGSL shift rules — a silent
        // CPU/GPU divergence (Law 10). Mask here so the wgpu/spirv/metal path
        // matches PTX and the oracle. A known in-range constant amount (the
        // `x >> 16` hot path) is left untouched — it would fold to itself — so
        // the mask only costs an `& 31` on genuinely variable shift counts.
        // (u64 shifts never reach here: the 64-bit gate fails them closed.)
        if is_shift {
            let amount_in_range = matches!(
                self.function.expressions.try_get(right_eff),
                Ok(Expression::Literal(Literal::U32(v))) if *v < 32
            );
            if !amount_in_range {
                right_eff = self.coerce_value_to_type(right_eff, self.types.u32_ty);
                let mask31 = self.append_expr(Expression::Literal(Literal::U32(31)));
                right_eff = self.append_expr(Expression::Binary {
                    op: BinaryOperator::And,
                    left: right_eff,
                    right: mask31,
                });
            }
        }
        // naga's `BinaryOperator::Modulo` lowers to an UNSIGNED remainder on the
        // SPIR-V backend even for SIGNED operands — a vendored-naga bug verified
        // on the 5090: `rem(i32, i32)` of (-7, 3) returned 0 (== unsigned
        // 0xFFFF_FFF9 % 3), not the signed -1, while `div(i32, i32)` of (-7, 3)
        // correctly returned -2 (naga's `Divide` DOES pick SDiv). naga's signed
        // Divide is trustworthy, so synthesize the signed remainder from the
        // truncating-division identity `a - (a / b) * b` (-7 - (-2)*3 = -1),
        // bypassing the buggy Modulo. Unsigned Mod keeps naga's `Modulo` (correct
        // for Uint, plus the divisor-zero guard below). The result type of a Mod
        // is its operand type (`binary_result_type` -> operand 0), so an i32_ty
        // result means signed operands. wgpu/spirv/metal all route through this
        // emitter, so the one fix covers every naga-derived backend.
        let signed_mod = matches!(effective_binop, BinOp::Mod)
            && self.binary_result_type(op, effective_binop)? == self.types.i32_ty;
        let value = if signed_mod {
            let quotient = self.append_expr(Expression::Binary {
                op: BinaryOperator::Divide,
                left: left_eff,
                right: right_eff,
            });
            let product = self.append_expr(Expression::Binary {
                op: BinaryOperator::Multiply,
                left: quotient,
                right: right_eff,
            });
            self.append_expr(Expression::Binary {
                op: BinaryOperator::Subtract,
                left: left_eff,
                right: product,
            })
        } else if let Some(value) =
            self.emit_synthetic_binop(effective_binop, left_eff, right_eff)
        {
            value
        } else if let Some(fun) = binary_math_function(effective_binop) {
            self.append_expr(Expression::Math {
                fun,
                arg: left_eff,
                arg1: Some(right_eff),
                arg2: None,
                arg3: None,
            })
        } else {
            let naga_op = binary_operator(effective_binop)?;
            self.append_expr(Expression::Binary {
                op: naga_op,
                left: left_eff,
                right: right_eff,
            })
        };
        // Div/Mod by zero is backend-divergent: naga 25 overrides a zero
        // divisor to 1 (so `x / 0 == x`, `x % 0 == 0`), while PTX leaves it to
        // unspecified hardware. The vyre-reference oracle documents a single
        // total contract (`u32 x / 0 == u32::MAX`, `x % 0 == 0`) with explicit
        // tests, so a bare Naga `Divide` makes the wgpu backend silently
        // disagree with its own oracle. Force the oracle contract here so every
        // backend is uniform and the CPU oracle stays sound (Law 10). Only the
        // unsigned divisor is guarded — signed div-by-zero / INT_MIN÷-1 are
        // rejected upstream as undefined backend semantics.
        let value = if matches!(binop, BinOp::Div | BinOp::Mod)
            && matches!(right_kind, Some(naga::ScalarKind::Uint))
        {
            let zero = self.append_expr(Expression::Literal(Literal::U32(0)));
            let divisor_is_zero = self.append_expr(Expression::Binary {
                op: BinaryOperator::Equal,
                left: right_eff,
                right: zero,
            });
            let sentinel = if matches!(binop, BinOp::Div) {
                self.append_expr(Expression::Literal(Literal::U32(u32::MAX)))
            } else {
                zero
            };
            self.append_expr(Expression::Select {
                condition: divisor_is_zero,
                accept: sentinel,
                reject: value,
            })
        } else {
            value
        };
        let ty = self.binary_result_type(op, binop)?;
        self.bind_result_typed(op, value, ty)
    }

    fn emit_synthetic_binop(
        &mut self,
        binop: BinOp,
        left: naga::Handle<Expression>,
        right: naga::Handle<Expression>,
    ) -> Option<naga::Handle<Expression>> {
        match binop {
            BinOp::AbsDiff => {
                let left_lt_right = self.append_expr(Expression::Binary {
                    op: BinaryOperator::Less,
                    left,
                    right,
                });
                let hi = self.append_expr(Expression::Select {
                    condition: left_lt_right,
                    accept: right,
                    reject: left,
                });
                let lo = self.append_expr(Expression::Select {
                    condition: left_lt_right,
                    accept: left,
                    reject: right,
                });
                Some(self.append_expr(Expression::Binary {
                    op: BinaryOperator::Subtract,
                    left: hi,
                    right: lo,
                }))
            }
            BinOp::RotateLeft | BinOp::RotateRight => {
                let mask = self.append_expr(Expression::Literal(Literal::U32(31)));
                let shift = self.append_expr(Expression::Binary {
                    op: BinaryOperator::And,
                    left: right,
                    right: mask,
                });
                let thirty_two = self.append_expr(Expression::Literal(Literal::U32(32)));
                let inv_raw = self.append_expr(Expression::Binary {
                    op: BinaryOperator::Subtract,
                    left: thirty_two,
                    right: shift,
                });
                let inv = self.append_expr(Expression::Binary {
                    op: BinaryOperator::And,
                    left: inv_raw,
                    right: mask,
                });
                let (left_shift, right_shift) = if matches!(binop, BinOp::RotateLeft) {
                    (shift, inv)
                } else {
                    (inv, shift)
                };
                let lhs = self.append_expr(Expression::Binary {
                    op: BinaryOperator::ShiftLeft,
                    left,
                    right: left_shift,
                });
                let rhs = self.append_expr(Expression::Binary {
                    op: BinaryOperator::ShiftRight,
                    left,
                    right: right_shift,
                });
                Some(self.append_expr(Expression::Binary {
                    op: BinaryOperator::InclusiveOr,
                    left: lhs,
                    right: rhs,
                }))
            }
            BinOp::MulHigh => Some(self.emit_u32_mul_high(left, right)),
            BinOp::SaturatingAdd => {
                let sum = self.append_expr(Expression::Binary {
                    op: BinaryOperator::Add,
                    left,
                    right,
                });
                let overflow = self.append_expr(Expression::Binary {
                    op: BinaryOperator::Less,
                    left: sum,
                    right: left,
                });
                let max = self.append_expr(Expression::Literal(Literal::U32(u32::MAX)));
                Some(self.append_expr(Expression::Select {
                    condition: overflow,
                    accept: max,
                    reject: sum,
                }))
            }
            BinOp::SaturatingSub => {
                let underflow = self.append_expr(Expression::Binary {
                    op: BinaryOperator::Less,
                    left,
                    right,
                });
                let diff = self.append_expr(Expression::Binary {
                    op: BinaryOperator::Subtract,
                    left,
                    right,
                });
                let zero = self.append_expr(Expression::Literal(Literal::U32(0)));
                Some(self.append_expr(Expression::Select {
                    condition: underflow,
                    accept: zero,
                    reject: diff,
                }))
            }
            BinOp::SaturatingMul => {
                let zero = self.append_expr(Expression::Literal(Literal::U32(0)));
                let max = self.append_expr(Expression::Literal(Literal::U32(u32::MAX)));
                let right_ne_zero = self.append_expr(Expression::Binary {
                    op: BinaryOperator::NotEqual,
                    left: right,
                    right: zero,
                });
                let one = self.append_expr(Expression::Literal(Literal::U32(1)));
                let divisor = self.append_expr(Expression::Select {
                    condition: right_ne_zero,
                    accept: right,
                    reject: one,
                });
                let limit = self.append_expr(Expression::Binary {
                    op: BinaryOperator::Divide,
                    left: max,
                    right: divisor,
                });
                let left_gt_limit = self.append_expr(Expression::Binary {
                    op: BinaryOperator::Greater,
                    left,
                    right: limit,
                });
                let overflow = self.append_expr(Expression::Binary {
                    op: BinaryOperator::LogicalAnd,
                    left: right_ne_zero,
                    right: left_gt_limit,
                });
                let product = self.append_expr(Expression::Binary {
                    op: BinaryOperator::Multiply,
                    left,
                    right,
                });
                Some(self.append_expr(Expression::Select {
                    condition: overflow,
                    accept: max,
                    reject: product,
                }))
            }
            _ => None,
        }
    }

    fn emit_u32_mul_high(
        &mut self,
        left: naga::Handle<Expression>,
        right: naga::Handle<Expression>,
    ) -> naga::Handle<Expression> {
        let mask16 = self.append_expr(Expression::Literal(Literal::U32(0xffff)));
        let shift16 = self.append_expr(Expression::Literal(Literal::U32(16)));
        let al = self.append_expr(Expression::Binary {
            op: BinaryOperator::And,
            left,
            right: mask16,
        });
        let ah = self.append_expr(Expression::Binary {
            op: BinaryOperator::ShiftRight,
            left,
            right: shift16,
        });
        let bl = self.append_expr(Expression::Binary {
            op: BinaryOperator::And,
            left: right,
            right: mask16,
        });
        let bh = self.append_expr(Expression::Binary {
            op: BinaryOperator::ShiftRight,
            left: right,
            right: shift16,
        });
        let p0 = self.append_expr(Expression::Binary {
            op: BinaryOperator::Multiply,
            left: al,
            right: bl,
        });
        let p1 = self.append_expr(Expression::Binary {
            op: BinaryOperator::Multiply,
            left: ah,
            right: bl,
        });
        let p2 = self.append_expr(Expression::Binary {
            op: BinaryOperator::Multiply,
            left: al,
            right: bh,
        });
        let p3 = self.append_expr(Expression::Binary {
            op: BinaryOperator::Multiply,
            left: ah,
            right: bh,
        });
        let p0_hi = self.append_expr(Expression::Binary {
            op: BinaryOperator::ShiftRight,
            left: p0,
            right: shift16,
        });
        let p1_lo = self.append_expr(Expression::Binary {
            op: BinaryOperator::And,
            left: p1,
            right: mask16,
        });
        let p2_lo = self.append_expr(Expression::Binary {
            op: BinaryOperator::And,
            left: p2,
            right: mask16,
        });
        let mid_a = self.append_expr(Expression::Binary {
            op: BinaryOperator::Add,
            left: p0_hi,
            right: p1_lo,
        });
        let mid_b = self.append_expr(Expression::Binary {
            op: BinaryOperator::Add,
            left: mid_a,
            right: p2_lo,
        });
        let carry = self.append_expr(Expression::Binary {
            op: BinaryOperator::ShiftRight,
            left: mid_b,
            right: shift16,
        });
        let p1_hi = self.append_expr(Expression::Binary {
            op: BinaryOperator::ShiftRight,
            left: p1,
            right: shift16,
        });
        let p2_hi = self.append_expr(Expression::Binary {
            op: BinaryOperator::ShiftRight,
            left: p2,
            right: shift16,
        });
        let high_a = self.append_expr(Expression::Binary {
            op: BinaryOperator::Add,
            left: p3,
            right: p1_hi,
        });
        let high_b = self.append_expr(Expression::Binary {
            op: BinaryOperator::Add,
            left: high_a,
            right: p2_hi,
        });
        self.append_expr(Expression::Binary {
            op: BinaryOperator::Add,
            left: high_b,
            right: carry,
        })
    }

    pub(super) fn global_invocation_axis(&mut self, axis: u32) -> naga::Handle<Expression> {
        let base = self.append_expr(Expression::FunctionArgument(self.builtins.global));
        self.append_expr(Expression::AccessIndex { base, index: axis })
    }

    pub(super) fn emit_opaque_expr(
        &mut self,
        op: &KernelOp,
        extension_id: u32,
        extension_kind: &str,
        payload: &[u8],
    ) -> Result<(), EmitError> {
        if matches!(
            extension_kind,
            "vyre.literal.u64" | "vyre.literal.i64" | "vyre.literal.f64"
        ) {
            let bytes: [u8; 8] = payload.try_into().map_err(|_| {
                EmitError::InvalidDescriptor(wide_literal_payload_message(
                    extension_kind,
                    payload.len(),
                ))
            })?;
            let (literal, ty) = match extension_kind {
                // Emit the full 64-bit literal directly. Naga's IR supports
                // Literal::U64 and the type handle u64_ty is already
                // registered in TypeHandles. Previously this narrowed to u32,
                // which silently produced the wrong type (and hard-errored for
                // values above u32::MAX), diverging from f64 which already
                // used Literal::F64. Callers that ask for vyre.literal.u64
                // always want a u64 result.
                "vyre.literal.u64" => {
                    let value = u64::from_le_bytes(bytes);
                    (Literal::U64(value), self.types.u64_ty)
                }
                // Emit the full 64-bit signed literal directly, matching the
                // u64 fix above. Previously narrowed to i32 and hard-errored
                // for values outside i32 range.
                "vyre.literal.i64" => {
                    let value = i64::from_le_bytes(bytes);
                    (Literal::I64(value), self.types.i64_ty)
                }
                "vyre.literal.f64" => (Literal::F64(f64::from_le_bytes(bytes)), self.types.f64_ty),
                other => {
                    return Err(EmitError::InvalidDescriptor(
                        wide_literal_kind_gate_message(other),
                    ));
                }
            };
            let value = self.append_expr(Expression::Literal(literal));
            return self.bind_result_typed(op, value, ty);
        }
        Err(EmitError::InvalidDescriptor(opaque_expression_message(
            extension_kind,
            extension_id,
        )))
    }

    /// Emit a Load on a byte-element binding (DataType::U8 / DataType::I8).
    ///
    /// Reference-eval treats U8/I8 buffers as byte-addressed; the WGSL
    /// backend has no native byte storage, so the underlying buffer is
    /// `array<u32>` (per `setup::scalar_type`). To honor the IR-level
    /// byte semantics, the emitter computes
    ///
    /// ```text
    /// word_index = index >> 2
    /// shift      = (index & 3) << 3
    /// byte       = (buffer[word_index] >> shift) & 0xff
    /// ```
    ///
    /// For `I8`, the extracted byte is sign-extended via the
    /// `(byte << 24) >> 24` bitcast pattern (arithmetic shift on i32
    /// preserves the sign bit).
    fn emit_byte_element_load(
        &mut self,
        op: &KernelOp,
        slot: u32,
        data_type: DataType,
    ) -> Result<(), EmitError> {
        // The IR-level index is a byte address. Translate it to a word
        // index for naga's `array<u32>` Access expression.
        let raw_index = self.value_operand(op, 1)?;
        let byte_index = self.coerce_value_to_type(raw_index, self.types.u32_ty);
        let two = self.literal_u32(2);
        let three = self.literal_u32(3);
        let eight = self.literal_u32(8);
        let mask_ff = self.literal_u32(0xff);
        let word_index = self.append_expr(Expression::Binary {
            op: BinaryOperator::ShiftRight,
            left: byte_index,
            right: two,
        });
        let lane_in_word = self.append_expr(Expression::Binary {
            op: BinaryOperator::And,
            left: byte_index,
            right: three,
        });
        let shift_bits = self.append_expr(Expression::Binary {
            op: BinaryOperator::Multiply,
            left: lane_in_word,
            right: eight,
        });
        let pointer = self.binding_element_pointer_by_slot(slot, word_index)?;
        let word_bits = self.append_expr(Expression::Load { pointer });
        // The byte-extract shift+mask is UNSIGNED bit manipulation. An `I8`
        // buffer is backed by `array<i32>` (scalar_type maps I8 -> i32_ty), so
        // the loaded word is Sint; masking it with the u32 `0xff`/shift literals
        // would emit `And(i32, u32)` / `ShiftRight(i32, u32)` which naga rejects
        // (InvalidBinaryOperandTypes) — the I8 byte-extract emitted invalid WGSL.
        // Reinterpret the word's bits to u32 so the whole extraction is u32; the
        // I8 case re-signs only at the final `(byte << 24) as i32 >> 24` step.
        // `U8` is already `array<u32>`, so its word needs no reinterpret.
        let word = if matches!(data_type, DataType::I8) {
            self.append_expr(Expression::As {
                expr: word_bits,
                kind: naga::ScalarKind::Uint,
                convert: None,
            })
        } else {
            word_bits
        };
        let shifted = self.append_expr(Expression::Binary {
            op: BinaryOperator::ShiftRight,
            left: word,
            right: shift_bits,
        });
        let byte_u32 = self.append_expr(Expression::Binary {
            op: BinaryOperator::And,
            left: shifted,
            right: mask_ff,
        });
        match data_type {
            DataType::U8 => {
                // Result type tracked in binding_types is u32_ty (per
                // setup::scalar_type's U8 → u32_ty mapping); the
                // extracted byte is already a u32 in the [0, 255]
                // range so it is wire-correct as-is.
                let ty = self.types.u32_ty;
                self.bind_result_typed(op, byte_u32, ty)
            }
            DataType::I8 => {
                // Sign-extend the [0, 255] u32 byte to a 32-bit signed
                // value via `((byte << 24) as i32) >> 24` (arithmetic
                // shift on i32 propagates the sign bit).
                let twenty_four = self.literal_u32(24);
                let shifted_left = self.append_expr(Expression::Binary {
                    op: BinaryOperator::ShiftLeft,
                    left: byte_u32,
                    right: twenty_four,
                });
                let as_i32 = self.append_expr(Expression::As {
                    expr: shifted_left,
                    kind: naga::ScalarKind::Sint,
                    convert: None,
                });
                let signed = self.append_expr(Expression::Binary {
                    op: BinaryOperator::ShiftRight,
                    left: as_i32,
                    right: twenty_four,
                });
                let ty = self.types.i32_ty;
                self.bind_result_typed(op, signed, ty)
            }
            other => Err(EmitError::InvalidBinding {
                slot,
                reason: non_byte_load_route_message(other),
            }),
        }
    }

    /// Emit a Store on a byte-element binding (DataType::U8 / DataType::I8).
    ///
    /// WGSL has no native byte storage; the underlying buffer is
    /// `array<u32>`. To store one byte at byte address `index` without
    /// clobbering the three adjacent bytes packed in the same u32, the
    /// emitter computes:
    ///
    /// ```text
    /// word_index = index >> 2
    /// shift      = (index & 3) << 3
    /// word       = buffer[word_index]
    /// cleared    = word & ~(0xff << shift)
    /// buffer[word_index] = cleared | ((value & 0xff) << shift)
    /// ```
    ///
    /// **Concurrency:** the read-modify-write is non-atomic. Two
    /// invocations writing different bytes of the same u32 word can race
    /// and lose one byte. This matches the existing convention for
    /// non-atomic word stores; callers needing safe concurrent byte
    /// stores should keep one invocation per word (the common pattern
    /// for output buffers indexed by `GlobalInvocationId`) or migrate to
    /// `Expr::Atomic` ops on a U32 buffer with explicit byte packing.
    fn emit_byte_element_store(&mut self, op: &KernelOp, slot: u32) -> Result<(), EmitError> {
        let raw_index = self.value_operand(op, 1)?;
        let raw_value = self.value_operand(op, 2)?;
        let byte_index = self.coerce_value_to_type(raw_index, self.types.u32_ty);
        let value_u32 = self.coerce_value_to_type(raw_value, self.types.u32_ty);
        let two = self.literal_u32(2);
        let three = self.literal_u32(3);
        let eight = self.literal_u32(8);
        let mask_ff = self.literal_u32(0xff);
        let word_index = self.append_expr(Expression::Binary {
            op: BinaryOperator::ShiftRight,
            left: byte_index,
            right: two,
        });
        let lane_in_word = self.append_expr(Expression::Binary {
            op: BinaryOperator::And,
            left: byte_index,
            right: three,
        });
        let shift_bits = self.append_expr(Expression::Binary {
            op: BinaryOperator::Multiply,
            left: lane_in_word,
            right: eight,
        });
        // (0xff << shift)  -  the byte mask in u32-word position.
        let lane_mask = self.append_expr(Expression::Binary {
            op: BinaryOperator::ShiftLeft,
            left: mask_ff,
            right: shift_bits,
        });
        // ~(0xff << shift)  -  invert to clear the target byte.
        let cleared_mask = self.append_expr(Expression::Unary {
            op: naga::UnaryOperator::BitwiseNot,
            expr: lane_mask,
        });
        // (value & 0xff) << shift  -  value byte in u32-word position.
        let value_byte = self.append_expr(Expression::Binary {
            op: BinaryOperator::And,
            left: value_u32,
            right: mask_ff,
        });
        let value_in_word = self.append_expr(Expression::Binary {
            op: BinaryOperator::ShiftLeft,
            left: value_byte,
            right: shift_bits,
        });
        // An `I8` buffer is backed by `array<i32>` (scalar_type maps I8 ->
        // i32_ty). The read-modify-write below is UNSIGNED bit manipulation
        // (mask/clear/merge with u32 literals), so on a Sint word it would emit
        // `And(i32, u32)` which naga rejects. Reinterpret the loaded word to u32
        // for the RMW, then reinterpret the merged result back to i32 before the
        // Store (whose value must match the array<i32> element type). `U8` is
        // already `array<u32>`, so it needs neither reinterpret.
        let is_signed_byte = matches!(self.binding_data_types.get(&slot), Some(DataType::I8));
        // Read-modify-write the u32 word.
        let pointer = self.binding_element_pointer_by_slot(slot, word_index)?;
        let word_bits = self.append_expr(Expression::Load { pointer });
        let word = if is_signed_byte {
            self.append_expr(Expression::As {
                expr: word_bits,
                kind: naga::ScalarKind::Uint,
                convert: None,
            })
        } else {
            word_bits
        };
        let cleared = self.append_expr(Expression::Binary {
            op: BinaryOperator::And,
            left: word,
            right: cleared_mask,
        });
        let merged = self.append_expr(Expression::Binary {
            op: BinaryOperator::InclusiveOr,
            left: cleared,
            right: value_in_word,
        });
        // Re-sign the merged u32 word to the buffer's i32 element type for I8.
        let store_value = if is_signed_byte {
            self.append_expr(Expression::As {
                expr: merged,
                kind: naga::ScalarKind::Sint,
                convert: None,
            })
        } else {
            merged
        };
        // Re-emit the pointer Access expression: naga's Statement::Store
        // requires a pointer that is in scope at the store site, and
        // the earlier `pointer` handle was consumed by the `Load`
        // we emitted above.
        let store_pointer = self.binding_element_pointer_by_slot(slot, word_index)?;
        self.function.body.push(
            Statement::Store {
                pointer: store_pointer,
                value: store_value,
            },
            Span::UNDEFINED,
        );
        Ok(())
    }
}
