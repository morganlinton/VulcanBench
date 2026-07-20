//! BinOp emit-time helpers split out of `emitter/mod.rs`:
//!
//! - [`BodyBuilder::is_bool_expression`]  -  local heuristic for whether
//!   a Naga `Expression` produces a bool-typed value (used to widen
//!   mixed bool+u32 bitwise operands so naga's type checker accepts).
//! - [`BodyBuilder::coerce_to_u32`]  -  `Expression::Select` (bool→u32),
//!   `Expression::As` (i32→u32), or identity for already-u32 values.
//! - [`BodyBuilder::fold_literal_binop`]  -  host-side evaluator for
//!   literal-vs-literal U32/I32 BinOps so naga's compile-time
//!   evaluator never sees underflow / overflow it would reject.
//!
//! These three helpers all run during the `KernelOpKind::BinOpKind`
//! emit arm. They live in their own file to keep `mod.rs` from
//! ballooning.

use naga::{BinaryOperator, Expression, Literal, ScalarKind, Type, UnaryOperator};
use vyre_foundation::ir::BinOp;

use super::BodyBuilder;

impl<'a> BodyBuilder<'a> {
    /// Heuristic: does this expression handle name a bool-typed value
    /// from a shape we can infer locally? Mirrors common bool-producing
    /// expression shapes naga rejects when bitwise-anded with u32.
    pub(super) fn is_bool_expression(&self, value: naga::Handle<Expression>) -> bool {
        match self.scalar_kind_of_expression(value, 0) {
            Some(ScalarKind::Bool) => true,
            Some(_) => false,
            // Unknown shape  -  fall back to the conservative shape
            // heuristic. Better than treating it as non-bool when it
            // might actually be bool (one direction of the And mixed-
            // type problem).
            None => self.is_bool_expression_inner(value, 0),
        }
    }

    /// Robust scalar-kind resolver for a Naga expression handle.
    /// Walks the expression tree using only the BodyBuilder's local
    /// state (function expressions arena + local_variables + globals),
    /// without needing the parent Module's typifier. Returns `None`
    /// when the shape is something we don't handle (e.g. compose,
    /// matrix, image ops); callers can fall back to a heuristic.
    ///
    /// Bounded recursion at depth 8.
    pub(super) fn scalar_kind_of_expression(
        &self,
        value: naga::Handle<Expression>,
        depth: u32,
    ) -> Option<naga::ScalarKind> {
        if depth > 8 {
            return None;
        }
        match &self.function.expressions[value] {
            Expression::Literal(Literal::Bool(_)) => Some(ScalarKind::Bool),
            Expression::Literal(Literal::U32(_)) => Some(ScalarKind::Uint),
            Expression::Literal(Literal::I32(_)) => Some(ScalarKind::Sint),
            Expression::Literal(Literal::F32(_)) => Some(ScalarKind::Float),
            Expression::ZeroValue(ty_handle) => match self.binding_types_lookup(*ty_handle) {
                Some(scalar) => Some(scalar),
                None => None,
            },
            Expression::Binary { op, left, .. } => match op {
                BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterEqual
                | BinaryOperator::LogicalAnd
                | BinaryOperator::LogicalOr => Some(ScalarKind::Bool),
                _ => self.scalar_kind_of_expression(*left, depth + 1),
            },
            Expression::Unary { op, expr } => match op {
                UnaryOperator::LogicalNot => Some(ScalarKind::Bool),
                _ => self.scalar_kind_of_expression(*expr, depth + 1),
            },
            Expression::Select { accept, .. } => self.scalar_kind_of_expression(*accept, depth + 1),
            Expression::As { kind, .. } => Some(*kind),
            Expression::LocalVariable(handle) => {
                let local = self.function.local_variables.try_get(*handle).ok()?;
                self.binding_types_lookup(local.ty)
            }
            Expression::Load { pointer } => self.scalar_kind_of_expression(*pointer, depth + 1),
            Expression::Math { arg, .. } => self.scalar_kind_of_expression(*arg, depth + 1),
            Expression::AccessIndex { base, .. } => {
                self.scalar_kind_of_expression(*base, depth + 1)
            }
            // A dynamic `array[index]` access into a buffer: resolve through the
            // base, exactly like the constant-index `AccessIndex`. Without this,
            // a load from a buffer indexed by a runtime value (every real scan)
            // could not resolve its scalar kind.
            Expression::Access { base, .. } => self.scalar_kind_of_expression(*base, depth + 1),
            // A storage/uniform buffer global. Resolve its ELEMENT scalar kind
            // via the slot's recorded element type. This is what lets a value
            // loaded from a SIGNED (i32) buffer resolve to `Sint`, so
            // `unify_binary_operand_types` can match `i32_load & u32_literal`
            // instead of emitting `And(i32, u32)` — invalid WGSL naga rejects.
            // (The `globals` map is slot -> global handle; reverse it.)
            Expression::GlobalVariable(handle) => self
                .globals
                .iter()
                .find(|(_, g)| **g == *handle)
                .and_then(|(slot, _)| self.binding_types.get(slot).copied())
                .and_then(|ty| self.binding_types_lookup(ty)),
            Expression::FunctionArgument(_) => Some(ScalarKind::Uint),
            _ => None,
        }
        .or_else(|| {
            // Pointer/Array bases  -  when we can't dereference we fall
            // back to None.
            None::<ScalarKind>
        })
        .map(|s| match s {
            // Naga sometimes carries a vec3<u32> whose access yields
            // u32  -  the recursive map above already covers that.
            other => other,
        })
        .and_then(|s| {
            // We resolved SOME scalar kind; prefer it.
            Some(s)
        })
        .or_else(|| {
            // Fallback: try the original handle through the Type arena
            // if it's a LocalVariable / GlobalVariable shape.
            None
        })
        .map(|s| s)
        .or(self.fallback_scalar_kind(value))
    }

    /// Last-ditch: if the expression has a registered handle in
    /// `binding_types` (for buffer Loads via slot id) or names a
    /// local variable through a one-step Load chain, return its
    /// scalar kind. Returns None when no signal is available.
    fn fallback_scalar_kind(&self, _value: naga::Handle<Expression>) -> Option<naga::ScalarKind> {
        None
    }

    /// Coerce a value to the binding's target type when it differs
    /// from the value's actual scalar kind. Used at every Store
    /// site so source builders that pass a bool to a u32 binding
    /// (or vice versa) don't trip naga's InvalidStoreTypes gate.
    pub(super) fn coerce_value_to_type(
        &mut self,
        value: naga::Handle<Expression>,
        target: naga::Handle<naga::Type>,
    ) -> naga::Handle<Expression> {
        let target_kind = if target == self.types.bool_ty {
            ScalarKind::Bool
        } else if target == self.types.u32_ty || target == self.types.u64_ty {
            ScalarKind::Uint
        } else if target == self.types.i32_ty || target == self.types.i64_ty {
            ScalarKind::Sint
        } else if target == self.types.f32_ty || target == self.types.f64_ty {
            ScalarKind::Float
        } else {
            // A non-scalar target (atomic<T>, vector, pointer, …) has no scalar
            // kind to coerce toward, so the value is emitted unchanged. This is
            // the correct identity — e.g. an atomic result feeding a later
            // descriptor op (atomic_result_can_feed_later_descriptor_ops) — NOT a
            // silent miscompile: scalar coercion simply does not apply to these
            // handles. The bool/u32/u64/i32/i64/f32/f64 arms above cover every
            // scalar kind vyre coerces between.
            return value;
        };
        if std::env::var("VYRE_COERCE_TRACE").is_ok() {
            tracing::debug!(
                "[coerce_value_to_type] value={value:?} target_kind={target_kind:?} scalar_kind={:?}",
                self.scalar_kind_of_expression(value, 0)
            );
        }
        let actual = match self.scalar_kind_of_expression(value, 0) {
            Some(k) => k,
            None => {
                // Heuristic infer didn't decide. Follow Load chains and
                // resolve the underlying LocalVariable's type via
                // `function.local_variables[h].ty` directly against the
                // module's `types` arena (not just the canonical handle
                // cache). Carrier locals allocated by allocate_carrier_local
                // use canonical handles so this hits, but Stores into
                // generated locals (loop indices, scratch) sometimes
                // carry one-off types that don't match the cache.
                self.resolve_underlying_local_kind(value)
                    .unwrap_or(target_kind)
            }
        };
        if actual == target_kind {
            return value;
        }
        match (actual, target_kind) {
            (ScalarKind::Bool, ScalarKind::Uint) => {
                self.coerce_to_u32(value, Some(self.types.bool_ty))
            }
            (ScalarKind::Bool, ScalarKind::Sint) => {
                let one = self.append_expr(Expression::Literal(Literal::I32(1)));
                let zero = self.append_expr(Expression::Literal(Literal::I32(0)));
                self.append_expr(Expression::Select {
                    condition: value,
                    accept: one,
                    reject: zero,
                })
            }
            (ScalarKind::Uint | ScalarKind::Sint, ScalarKind::Bool) => {
                self.ensure_bool_condition(value)
            }
            (_, ScalarKind::Uint) => self.append_expr(Expression::As {
                expr: value,
                kind: ScalarKind::Uint,
                convert: Some(4),
            }),
            (_, ScalarKind::Sint) => self.append_expr(Expression::As {
                expr: value,
                kind: ScalarKind::Sint,
                convert: Some(4),
            }),
            (_, ScalarKind::Float) => self.append_expr(Expression::As {
                expr: value,
                kind: ScalarKind::Float,
                convert: Some(4),
            }),
            _ => value,
        }
    }

    /// Ensure a Naga expression handle names a bool-typed value;
    /// coerce non-bool to bool via `value != 0`. Used at every
    /// emit site that produces a Naga construct requiring a bool
    /// condition (Select, structured-if branches, loop break_if).
    pub(super) fn ensure_bool_condition(
        &mut self,
        value: naga::Handle<Expression>,
    ) -> naga::Handle<Expression> {
        let kind = self.scalar_kind_of_expression(value, 0);
        match kind {
            Some(naga::ScalarKind::Bool) => value,
            Some(naga::ScalarKind::Uint) => {
                let zero = self.append_expr(Expression::Literal(Literal::U32(0)));
                self.append_expr(Expression::Binary {
                    op: BinaryOperator::NotEqual,
                    left: value,
                    right: zero,
                })
            }
            Some(naga::ScalarKind::Sint) => {
                let zero = self.append_expr(Expression::Literal(Literal::I32(0)));
                self.append_expr(Expression::Binary {
                    op: BinaryOperator::NotEqual,
                    left: value,
                    right: zero,
                })
            }
            Some(naga::ScalarKind::Float) => {
                let zero = self.append_expr(Expression::Literal(Literal::F32(0.0)));
                self.append_expr(Expression::Binary {
                    op: BinaryOperator::NotEqual,
                    left: value,
                    right: zero,
                })
            }
            _ => value,
        }
    }

    /// Look up a Naga type handle and return its scalar kind if it
    /// is a scalar / vector / pointer-to-scalar.
    /// Walk a `Load { LocalVariable }` chain to the underlying local
    /// and ask for its scalar kind even when the local's type handle
    /// isn't one of the four canonical scalar handles cached in
    /// `self.types`. Reads naga's type arena directly via the
    /// `function.local_variables` and `module.types` lookup that
    /// `binding_types_lookup` skips.
    pub(super) fn resolve_underlying_local_kind(
        &self,
        value: naga::Handle<Expression>,
    ) -> Option<ScalarKind> {
        let mut current = value;
        for _ in 0..6 {
            match self.function.expressions[current] {
                Expression::Load { pointer } => current = pointer,
                Expression::LocalVariable(handle) => {
                    let local = self.function.local_variables.try_get(handle).ok()?;
                    if local.ty == self.types.bool_ty {
                        return Some(ScalarKind::Bool);
                    } else if local.ty == self.types.u32_ty || local.ty == self.types.u64_ty {
                        return Some(ScalarKind::Uint);
                    } else if local.ty == self.types.i32_ty || local.ty == self.types.i64_ty {
                        return Some(ScalarKind::Sint);
                    } else if local.ty == self.types.f32_ty || local.ty == self.types.f64_ty {
                        return Some(ScalarKind::Float);
                    }
                    return None;
                }
                _ => return None,
            }
        }
        None
    }

    fn binding_types_lookup(&self, ty: naga::Handle<naga::Type>) -> Option<naga::ScalarKind> {
        // BodyBuilder doesn't own the type arena directly  -  we read
        // from `self.types` which holds all canonical scalar-type handles
        // vyre creates upfront. Match against all seven.
        if ty == self.types.bool_ty {
            Some(ScalarKind::Bool)
        } else if ty == self.types.u32_ty || ty == self.types.u64_ty {
            Some(ScalarKind::Uint)
        } else if ty == self.types.i32_ty || ty == self.types.i64_ty {
            Some(ScalarKind::Sint)
        } else if ty == self.types.f32_ty || ty == self.types.f64_ty {
            Some(ScalarKind::Float)
        } else {
            None
        }
    }

    /// Bounded recursion: a Select / Load chain whose terminal is a
    /// bool comparison still names a bool value. Cap depth at 6 so a
    /// pathological IR doesn't burn the stack.
    fn is_bool_expression_inner(&self, value: naga::Handle<Expression>, depth: u32) -> bool {
        if depth > 6 {
            return false;
        }
        match self.function.expressions[value] {
            Expression::Literal(Literal::Bool(_)) => true,
            Expression::Binary { op, .. } => matches!(
                op,
                BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterEqual
                    | BinaryOperator::LogicalAnd
                    | BinaryOperator::LogicalOr
            ),
            Expression::Unary {
                op: UnaryOperator::LogicalNot,
                ..
            } => true,
            // Select is bool-typed when both branches are bool.
            Expression::Select { accept, reject, .. } => {
                self.is_bool_expression_inner(accept, depth + 1)
                    && self.is_bool_expression_inner(reject, depth + 1)
            }
            // Load can be bool-typed; we can't always introspect the
            // pointer's element type from the expression alone, so
            // conservatively report `false` and rely on
            // `value_type_operand` for those.
            _ => false,
        }
    }

    /// Coerce a value to u32 at emit time when its source type is
    /// either bool (via `select(b, 1u, 0u)`) or i32 (via `As`). For
    /// already-u32 values this is the identity. Used to widen a
    /// mixed-type bitwise BinOp's operands to a common u32 form so
    /// naga's type checker accepts the resulting expression.
    pub(super) fn coerce_to_u32(
        &mut self,
        value: naga::Handle<Expression>,
        ty_hint: Option<naga::Handle<Type>>,
    ) -> naga::Handle<Expression> {
        // Trust the naga expression resolver FIRST. The `ty_hint` from
        // value_types[id] is sometimes wrong (vyre IR's tracked type
        // doesn't always survive emit-naga's inserted comparisons /
        // coercions). If we coerced based on the stale hint we'd emit
        // `select(u32_value, 1u, 0u)` whose condition is u32  -  which
        // naga then rejects with `SelectConditionNotABool`.
        let actual = self.scalar_kind_of_expression(value, 0);
        let kind = match actual {
            Some(k) => k,
            None => {
                // Resolver couldn't decide; fall back to the hint.
                if ty_hint == Some(self.types.bool_ty) {
                    ScalarKind::Bool
                } else if ty_hint == Some(self.types.i32_ty) {
                    ScalarKind::Sint
                } else if ty_hint == Some(self.types.f32_ty) {
                    ScalarKind::Float
                } else {
                    return value;
                }
            }
        };
        match kind {
            ScalarKind::Uint => value,
            ScalarKind::Bool => {
                let one = self.append_expr(Expression::Literal(Literal::U32(1)));
                let zero = self.append_expr(Expression::Literal(Literal::U32(0)));
                self.append_expr(Expression::Select {
                    condition: value,
                    accept: one,
                    reject: zero,
                })
            }
            ScalarKind::Sint => self.append_expr(Expression::As {
                expr: value,
                kind: ScalarKind::Uint,
                convert: Some(4),
            }),
            ScalarKind::Float => self.append_expr(Expression::As {
                expr: value,
                kind: ScalarKind::Uint,
                convert: Some(4),
            }),
            _ => value,
        }
    }

    /// Pre-fold literal-vs-literal BinOps at emit time so naga's
    /// static-evaluator never sees a wrap/underflow that would be
    /// valid at WGSL runtime but reject at shader-parse time.
    /// Specifically: `0u - 1u` is legal WGSL u32 wrap-around at runtime,
    /// but naga refuses the literal form with "subtraction operation
    /// overflowed". This commonly fires in guarded
    /// `select(cond, x - 1, 0)` shapes after const-fold resolves x to
    /// literal 0.
    pub(super) fn fold_literal_binop(
        &mut self,
        left: naga::Handle<Expression>,
        right: naga::Handle<Expression>,
        binop: BinOp,
    ) -> Option<naga::Handle<Expression>> {
        let left_lit = match self.function.expressions[left] {
            Expression::Literal(l) => l,
            _ => return None,
        };
        let right_lit = match self.function.expressions[right] {
            Expression::Literal(l) => l,
            _ => return None,
        };
        let folded = match (left_lit, right_lit) {
            (Literal::U32(a), Literal::U32(b)) => match binop {
                BinOp::Add | BinOp::WrappingAdd => Literal::U32(a.wrapping_add(b)),
                // WGSL u32 subtraction wraps at runtime (two's complement).
                // Both BinOp::Sub and BinOp::WrappingSub must produce the
                // same bit-for-bit result as the GPU would: e.g. 0u32 - 1u32
                // == 0xFFFF_FFFF. The previous saturating_sub was a
                // mis-fold: it produced 0 for underflow, diverging from
                // the runtime value. WrappingSub in particular has an
                // explicit wrapping contract that saturating_sub violates.
                BinOp::Sub | BinOp::WrappingSub => Literal::U32(a.wrapping_sub(b)),
                BinOp::Mul => Literal::U32(a.wrapping_mul(b)),
                BinOp::Div if b != 0 => Literal::U32(a / b),
                BinOp::Mod if b != 0 => Literal::U32(a % b),
                BinOp::BitAnd => Literal::U32(a & b),
                BinOp::BitOr => Literal::U32(a | b),
                BinOp::BitXor => Literal::U32(a ^ b),
                BinOp::Shl if b < 32 => Literal::U32(a.wrapping_shl(b)),
                BinOp::Shr if b < 32 => Literal::U32(a.wrapping_shr(b)),
                _ => return None,
            },
            (Literal::I32(a), Literal::I32(b)) => match binop {
                BinOp::Add | BinOp::WrappingAdd => Literal::I32(a.wrapping_add(b)),
                BinOp::Sub | BinOp::WrappingSub => Literal::I32(a.wrapping_sub(b)),
                BinOp::Mul => Literal::I32(a.wrapping_mul(b)),
                BinOp::Div if b != 0 => Literal::I32(a.wrapping_div(b)),
                BinOp::Mod if b != 0 => Literal::I32(a.wrapping_rem(b)),
                BinOp::BitAnd => Literal::I32(a & b),
                BinOp::BitOr => Literal::I32(a | b),
                BinOp::BitXor => Literal::I32(a ^ b),
                _ => return None,
            },
            _ => return None,
        };
        Some(self.append_expr(Expression::Literal(folded)))
    }
}
