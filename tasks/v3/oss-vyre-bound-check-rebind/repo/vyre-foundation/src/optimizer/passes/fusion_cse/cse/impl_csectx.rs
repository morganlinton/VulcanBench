#![allow(clippy::expect_used)]
use super::expr_key::ExprId;
use super::{expr_has_effect, CseCtx, ScopeFrame, ScopedBinding};
use crate::ir::{Expr, Ident, Node};
use std::borrow::Cow;

impl CseCtx {
    /// Reset all accumulated state while preserving allocated capacity.
    #[inline]
    pub fn clear(&mut self) {
        self.values.clear();
        self.undo_log.clear();
        self.scope_stack.clear();
        self.current_epoch = 0;
        self.arena.clear();
        self.deduplication.clear();
        self.subgroup_counter = 0;
        self.intern_calls
            .store(0, std::sync::atomic::Ordering::Relaxed);
    }

    #[inline]
    pub(crate) fn enter_scope(&mut self) {
        self.scope_stack.push(ScopeFrame {
            undo_len: self.undo_log.len(),
            epoch: self.current_epoch,
        });
    }

    #[inline]
    pub(crate) fn leave_scope(&mut self) {
        let Some(frame) = self.scope_stack.pop() else {
            return;
        };
        while self.undo_log.len() > frame.undo_len {
            let Some((key, old)) = self.undo_log.pop() else {
                break;
            };
            match old {
                Some(value) => {
                    self.values.insert(key, value);
                }
                None => {
                    self.values.remove(&key);
                }
            }
        }
        self.current_epoch = frame.epoch;
    }

    #[inline]
    pub(crate) fn clear_observed_state(&mut self) {
        if self.scope_stack.is_empty() {
            self.values.clear();
        } else {
            self.current_epoch = self.current_epoch.wrapping_add(1);
        }
    }

    #[inline]
    fn record_insert(&mut self, key: ExprId, value: Ident) {
        let old = self.values.insert(
            key,
            ScopedBinding {
                name: value,
                epoch: self.current_epoch,
            },
        );
        if !self.scope_stack.is_empty() {
            self.undo_log.push((key, old));
        }
    }

    #[inline]
    fn visible_value(&self, key: ExprId) -> Option<&Ident> {
        let value = self.values.get(&key)?;
        (value.epoch == self.current_epoch).then_some(&value.name)
    }

    #[inline]
    pub(crate) fn nodes(&mut self, nodes: &[Node]) -> Vec<Node> {
        // VYRE_IR_HOTSPOTS CRIT (cse/impl_csectx.rs:64-65): pre-allocate the
        // result vector with the exact capacity needed so the collector
        // doesn't reallocate mid-way for large node lists.
        let mut out = Vec::with_capacity(nodes.len());
        out.extend(nodes.iter().map(|node| self.node(node)));
        out
    }

    #[inline]
    #[expect(
        clippy::too_many_lines,
        reason = "CSE node rewrite keeps visible-value state transitions adjacent to Node variant handling"
    )]
    pub(crate) fn node(&mut self, node: &Node) -> Node {
        match node {
            Node::Let { name, value } => {
                let value = self.expr(value);
                if expr_has_effect(value.as_ref()) {
                    self.clear_observed_state();
                    return Node::let_bind(name, value.into_owned());
                }

                // Do not CSE-alias literals through variables: `let state = 0u`
                // must not record `LitU32(0) → "state"` because `state` may be
                // reassigned later while the literal stays constant.
                if matches!(
                    value.as_ref(),
                    Expr::Var(_)
                        | Expr::LitU32(_)
                        | Expr::LitI32(_)
                        | Expr::LitF32(_)
                        | Expr::LitBool(_)
                ) {
                    return Node::let_bind(name, value.into_owned());
                }
                let key = self.intern_expr(value.as_ref());
                let canonical = if let Some(existing) = self.visible_value(key) {
                    Expr::var(existing.clone())
                } else {
                    self.record_insert(key, name.clone());
                    value.into_owned()
                };
                Node::let_bind(name, canonical)
            }
            Node::Assign { name, value } => {
                let value = self.expr(value).into_owned();
                self.clear_observed_state();
                Node::assign(name, value)
            }
            Node::Store {
                buffer,
                index,
                value,
            } => {
                let index = self.expr(index).into_owned();
                let value = self.expr(value).into_owned();
                self.clear_observed_state();
                Node::store(buffer, index, value)
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                let cond = self.expr(cond).into_owned();
                self.enter_scope();
                let then = self.nodes(then);
                self.leave_scope();
                self.enter_scope();
                let otherwise = self.nodes(otherwise);
                self.leave_scope();
                self.clear_observed_state();
                Node::if_then_else(cond, then, otherwise)
            }
            Node::Loop {
                var,
                from,
                to,
                body,
            } => {
                let from = self.expr(from).into_owned();
                let to = self.expr(to).into_owned();
                let mut body_ctx = CseCtx::default();
                let body = body_ctx.nodes(body);
                self.clear_observed_state();
                Node::loop_for(var, from, to, body)
            }
            Node::Return => Node::Return,
            Node::Block(nodes) => {
                self.enter_scope();
                let nodes = self.nodes(nodes);
                self.leave_scope();
                self.clear_observed_state();
                Node::block(nodes)
            }
            Node::Barrier { ordering } => {
                self.clear_observed_state();
                Node::barrier_with_ordering(*ordering)
            }
            Node::IndirectDispatch {
                count_buffer,
                count_offset,
            } => {
                self.clear_observed_state();
                Node::IndirectDispatch {
                    count_buffer: count_buffer.clone(),
                    count_offset: *count_offset,
                }
            }
            Node::AsyncLoad {
                source,
                destination,
                offset,
                size,
                tag,
            } => {
                self.clear_observed_state();
                Node::async_load_ext(
                    source.clone(),
                    destination.clone(),
                    (**offset).clone(),
                    (**size).clone(),
                    tag.clone(),
                )
            }
            Node::AsyncStore {
                source,
                destination,
                offset,
                size,
                tag,
            } => {
                self.clear_observed_state();
                Node::async_store(
                    source.clone(),
                    destination.clone(),
                    (**offset).clone(),
                    (**size).clone(),
                    tag.clone(),
                )
            }
            Node::AsyncWait { tag } => {
                self.clear_observed_state();
                Node::AsyncWait { tag: tag.clone() }
            }
            Node::Trap { .. } | Node::Resume { .. } => {
                self.clear_observed_state();
                node.clone()
            }
            Node::AllReduce { .. }
            | Node::AllGather { .. }
            | Node::ReduceScatter { .. }
            | Node::Broadcast { .. } => {
                self.clear_observed_state();
                node.clone()
            }
            Node::Region {
                generator,
                source_region,
                body,
            } => {
                // Regions create a new variable scope
                self.enter_scope();
                let nodes = self.nodes(body);
                self.leave_scope();
                // Observed state (loads/stores) does not leak out of region
                self.clear_observed_state();
                Node::Region {
                    generator: generator.clone(),
                    source_region: source_region.clone(),
                    body: std::sync::Arc::new(nodes),
                }
            }
            Node::Opaque(extension) => {
                self.clear_observed_state();
                Node::Opaque(extension.clone())
            }
        }
    }

    #[inline]
    pub(crate) fn expr<'a>(&mut self, expr: &'a Expr) -> Cow<'a, Expr> {
        let rewritten = match expr {
            Expr::Load { buffer, index } => match self.expr(index) {
                Cow::Borrowed(_) => Cow::Borrowed(expr),
                Cow::Owned(index) => Cow::Owned(Expr::Load {
                    buffer: buffer.clone(),
                    index: Box::new(index),
                }),
            },
            Expr::BinOp { op, left, right } => {
                rewrite_binary(expr, *op, self.expr(left), self.expr(right))
            }
            Expr::UnOp { op, operand } => match self.expr(operand) {
                Cow::Borrowed(_) => Cow::Borrowed(expr),
                Cow::Owned(operand) => Cow::Owned(Expr::UnOp {
                    op: op.clone(),
                    operand: Box::new(operand),
                }),
            },
            Expr::Fma { a, b, c } => rewrite_fma(expr, self.expr(a), self.expr(b), self.expr(c)),
            Expr::Call { op_id, args } => match rewrite_args(self, args) {
                Cow::Borrowed(_) => Cow::Borrowed(expr),
                Cow::Owned(args) => Cow::Owned(Expr::Call {
                    op_id: op_id.clone(),
                    args,
                }),
            },
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => rewrite_select(
                expr,
                self.expr(cond),
                self.expr(true_val),
                self.expr(false_val),
            ),
            Expr::Cast { target, value } => match self.expr(value) {
                Cow::Borrowed(_) => Cow::Borrowed(expr),
                Cow::Owned(value) => Cow::Owned(Expr::Cast {
                    target: target.clone(),
                    value: Box::new(value),
                }),
            },
            Expr::Atomic {
                op,
                buffer,
                index,
                expected,
                value,
                ordering,
            } => {
                let index = self.expr(index).into_owned();
                let expected = expected
                    .as_deref()
                    .map(|expr| Box::new(self.expr(expr).into_owned()));
                let value = self.expr(value).into_owned();
                self.clear_observed_state();
                Cow::Owned(Expr::Atomic {
                    op: *op,
                    buffer: buffer.clone(),
                    index: Box::new(index),
                    expected,
                    value: Box::new(value),
                    ordering: *ordering,
                })
            }
            Expr::LitU32(_)
            | Expr::LitI32(_)
            | Expr::LitF32(_)
            | Expr::LitBool(_)
            | Expr::Var(_)
            | Expr::BufLen { .. }
            | Expr::InvocationId { .. }
            | Expr::WorkgroupId { .. }
            | Expr::LocalId { .. }
            | Expr::SubgroupBallot { .. }
            | Expr::SubgroupShuffle { .. }
            | Expr::SubgroupReduce { .. }
            | Expr::SubgroupLocalId
            | Expr::SubgroupSize
            | Expr::Opaque(_) => Cow::Borrowed(expr),
        };

        if matches!(rewritten.as_ref(), Expr::Var(_)) || expr_has_effect(rewritten.as_ref()) {
            return rewritten;
        }

        // Soundness fix (S19): the previous pointer cache mapped
        // `*const Expr → ExprId`, claiming the IR is immutable during
        // a single CSE pass. That isn't safe  -  Box<Expr> sub-trees
        // freed by `Cow::Owned` rewrites can be reallocated at the
        // same address by later sub-trees, so a stale ExprId from a
        // prior expression flows through `values.get(stale_id)` and
        // CSE merges semantically distinct expressions (caught by
        // `full_optimize_is_idempotent_on_canonical_wire` regression
        //  -  `BitAnd(46, Mul(BitAnd(888, X), 0))` collapsed into the
        // unrelated `BitOr(invocation, 1)`). Drop the cache; the
        // underlying `deduplication` map already gives O(1) intern
        // dedup by structural key.
        let key = self.intern_expr(rewritten.as_ref());
        // Never replace a literal with a variable reference  -  literals are
        // already minimal, and the variable may be mutated later (e.g. loop
        // counters or state accumulators). Substituting `0u` with `var state`
        // when `state` was initially `0u` is unsound if `state` is reassigned.
        if matches!(
            rewritten.as_ref(),
            Expr::LitU32(_) | Expr::LitI32(_) | Expr::LitBool(_)
        ) {
            return rewritten;
        }
        self.visible_value(key).map_or(rewritten, |existing| {
            Cow::Owned(Expr::var(existing.clone()))
        })
    }
}

#[inline]
fn rewrite_binary<'a>(
    original: &'a Expr,
    op: crate::ir::BinOp,
    left: Cow<'a, Expr>,
    right: Cow<'a, Expr>,
) -> Cow<'a, Expr> {
    if matches!((&left, &right), (Cow::Borrowed(_), Cow::Borrowed(_))) {
        return Cow::Borrowed(original);
    }
    Cow::Owned(Expr::BinOp {
        op,
        left: Box::new(left.into_owned()),
        right: Box::new(right.into_owned()),
    })
}

#[inline]
fn rewrite_fma<'a>(
    original: &'a Expr,
    a: Cow<'a, Expr>,
    b: Cow<'a, Expr>,
    c: Cow<'a, Expr>,
) -> Cow<'a, Expr> {
    if matches!(
        (&a, &b, &c),
        (Cow::Borrowed(_), Cow::Borrowed(_), Cow::Borrowed(_))
    ) {
        return Cow::Borrowed(original);
    }
    Cow::Owned(Expr::Fma {
        a: Box::new(a.into_owned()),
        b: Box::new(b.into_owned()),
        c: Box::new(c.into_owned()),
    })
}

#[inline]
fn rewrite_select<'a>(
    original: &'a Expr,
    cond: Cow<'a, Expr>,
    true_val: Cow<'a, Expr>,
    false_val: Cow<'a, Expr>,
) -> Cow<'a, Expr> {
    if matches!(
        (&cond, &true_val, &false_val),
        (Cow::Borrowed(_), Cow::Borrowed(_), Cow::Borrowed(_))
    ) {
        return Cow::Borrowed(original);
    }
    Cow::Owned(Expr::Select {
        cond: Box::new(cond.into_owned()),
        true_val: Box::new(true_val.into_owned()),
        false_val: Box::new(false_val.into_owned()),
    })
}

#[inline]
fn rewrite_args<'a>(ctx: &mut CseCtx, args: &'a [Expr]) -> Cow<'a, [Expr]> {
    let mut rewritten: Option<Vec<Expr>> = None;
    for (index, arg) in args.iter().enumerate() {
        match ctx.expr(arg) {
            Cow::Borrowed(_) if rewritten.is_none() => {}
            Cow::Borrowed(borrowed) => {
                if let Some(out) = rewritten.as_mut() {
                    out.push(Expr::clone(borrowed));
                }
            }
            Cow::Owned(owned) => {
                let out = rewritten.get_or_insert_with(|| {
                    let mut out = Vec::with_capacity(args.len());
                    out.extend_from_slice(&args[..index]);
                    out
                });
                out.push(owned);
            }
        }
    }
    rewritten.map_or(Cow::Borrowed(args), Cow::Owned)
}
