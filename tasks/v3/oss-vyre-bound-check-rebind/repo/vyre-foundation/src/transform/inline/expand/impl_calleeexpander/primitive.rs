use super::super::CalleeExpander;
use crate::error::Result;
use crate::ir::{AtomicOp, BinOp, Expr, Ident, Node, UnOp};

impl CalleeExpander<'_> {
    #[inline]
    pub(crate) fn expr(&mut self, expr: &Expr) -> Result<(Vec<Node>, Expr)> {
        match expr {
            Expr::Var(name) => Ok((Vec::new(), Expr::var(self.rename_use(name)))),
            Expr::Load { buffer, index } => self.load(buffer, index),
            Expr::BufLen { buffer } if self.output_name == *buffer => {
                Ok((Vec::new(), Expr::u32(1)))
            }
            Expr::BufLen { buffer } if self.input_args.contains_key(buffer) => {
                Ok((Vec::new(), Expr::u32(1)))
            }
            Expr::Call { .. } => {
                let renamed = self.rename_expr_vars(expr)?;
                self.ctx.inline_expr(&renamed)
            }
            Expr::InvocationId { .. }
            | Expr::WorkgroupId { .. }
            | Expr::LocalId { .. }
            | Expr::SubgroupLocalId
            | Expr::SubgroupSize => {
                Err(crate::error::Error::lowering(
                    "inliner cannot inline a callee that references \
                     InvocationId / WorkgroupId / LocalId / SubgroupLocalId / SubgroupSize: \
                     these built-ins are per-invocation and cannot be passed as callee arguments. \
                     Fix: hoist the built-in read to the call site and pass it as an explicit \
                     argument before inlining.".to_string(),
                ))
            }
            Expr::LitU32(_)
            | Expr::LitI32(_)
            | Expr::LitF32(_)
            | Expr::LitBool(_)
            | Expr::BufLen { .. } => Ok((Vec::new(), expr.clone())),
            Expr::BinOp { op, left, right } => self.binop(*op, left, right),
            Expr::UnOp { op, operand } => self.unop(op.clone(), operand),
            Expr::Fma { a, b, c } => self.fma(a, b, c),
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => self.select(cond, true_val, false_val),
            Expr::Cast { target, value } => {
                let (prefix, value) = self.expr(value)?;
                Ok((
                    prefix,
                    Expr::Cast {
                        target: target.clone(),
                        value: Box::new(value),
                    },
                ))
            }
            Expr::Atomic {
                op,
                buffer,
                index,
                expected,
                value,
                ordering,
            } => self.atomic(*op, buffer, index, expected.as_deref(), value, *ordering),
            &Expr::SubgroupBallot { .. } | &Expr::SubgroupShuffle { .. } | &Expr::SubgroupReduce { .. } => {
                Err(crate::error::Error::lowering(
                    "inliner cannot expand subgroup intrinsics; RFC 0004 gates this on target builder 25+. Fix: avoid inlining across subgroup-op boundaries.".to_string(),
                ))
            }
        Expr::Opaque(extension) => Err(crate::error::Error::lowering(format!(
                "inliner cannot expand opaque expression extension `{}`/`{}`. Fix: lower the extension to core Expr variants before inlining.",
                extension.extension_kind(),
                extension.debug_identity()
            ))),
        }
    }

    #[inline]
    pub(crate) fn load(&mut self, buffer: &Ident, index: &Expr) -> Result<(Vec<Node>, Expr)> {
        if let Some(arg) = self.input_args.get(buffer) {
            return Ok((Vec::new(), arg.clone()));
        }
        let (prefix, index) = self.expr(index)?;
        Ok((
            prefix,
            Expr::Load {
                buffer: buffer.into(),
                index: Box::new(index),
            },
        ))
    }

    #[inline]
    pub(crate) fn binop(
        &mut self,
        op: BinOp,
        left: &Expr,
        right: &Expr,
    ) -> Result<(Vec<Node>, Expr)> {
        let (mut prefix, left) = self.expr(left)?;
        let (right_prefix, right) = self.expr(right)?;
        prefix.extend(right_prefix);
        Ok((
            prefix,
            Expr::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
        ))
    }

    #[inline]
    pub(crate) fn unop(&mut self, op: UnOp, operand: &Expr) -> Result<(Vec<Node>, Expr)> {
        let (prefix, operand) = self.expr(operand)?;
        Ok((
            prefix,
            Expr::UnOp {
                op,
                operand: Box::new(operand),
            },
        ))
    }

    #[inline]
    pub(crate) fn fma(&mut self, a: &Expr, b: &Expr, c: &Expr) -> Result<(Vec<Node>, Expr)> {
        let (mut prefix, a) = self.expr(a)?;
        let (b_prefix, b) = self.expr(b)?;
        let (c_prefix, c) = self.expr(c)?;
        prefix.extend(b_prefix);
        prefix.extend(c_prefix);
        Ok((
            prefix,
            Expr::Fma {
                a: Box::new(a),
                b: Box::new(b),
                c: Box::new(c),
            },
        ))
    }

    #[inline]
    pub(crate) fn select(
        &mut self,
        cond: &Expr,
        true_val: &Expr,
        false_val: &Expr,
    ) -> Result<(Vec<Node>, Expr)> {
        let (mut prefix, cond) = self.expr(cond)?;
        let (true_prefix, true_val) = self.expr(true_val)?;
        let (false_prefix, false_val) = self.expr(false_val)?;
        prefix.extend(true_prefix);
        prefix.extend(false_prefix);
        Ok((
            prefix,
            Expr::Select {
                cond: Box::new(cond),
                true_val: Box::new(true_val),
                false_val: Box::new(false_val),
            },
        ))
    }

    #[inline]
    pub(crate) fn atomic(
        &mut self,
        op: AtomicOp,
        buffer: &Ident,
        index: &Expr,
        expected: Option<&Expr>,
        value: &Expr,
        ordering: crate::memory_model::MemoryOrdering,
    ) -> Result<(Vec<Node>, Expr)> {
        let (mut prefix, index) = self.expr(index)?;
        let (expected_prefix, expected) = match expected {
            Some(expected) => {
                let (prefix, expected) = self.expr(expected)?;
                (prefix, Some(Box::new(expected)))
            }
            None => (Vec::new(), None),
        };
        let (value_prefix, value) = self.expr(value)?;
        prefix.extend(expected_prefix);
        prefix.extend(value_prefix);
        Ok((
            prefix,
            Expr::Atomic {
                op,
                buffer: buffer.into(),
                index: Box::new(index),
                expected,
                value: Box::new(value),
                ordering,
            },
        ))
    }
}
