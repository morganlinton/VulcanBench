//! Constant-propagation rewrite over the post-fold/post-CSE Program.
//!
//! Walks the program once, building a per-scope `name → LitU32` map
//! for every `Node::Let { name, value: Expr::LitU32(v) }`; then walks
//! again, replacing every `Expr::Var(name)` reference with the
//! literal value when the name is in the map. The two walks fuse
//! into one DFS that processes Lets first (forward-only data flow,
//! no use-before-def issues since the IR is structured).
//!
//! Per-scope semantics: a Let in an If's `then` branch is NOT visible
//! after the If  -  the constant-prop map for the parent scope is
//! re-used after the branch, but the branch's own additions are
//! popped on exit. This matches the IR's lexical scoping.
//!
//! This is the rewrite that catches cascading folds: after CSE
//! let-dedupe rewrites `let b = Var(a)`, const-prop turns `b`'s uses
//! into the same literal `a` was bound to, which then cascades
//! through later const-fold passes (or dead-store elimination if the
//! Var-bound let becomes unused).

use std::sync::Arc;

use rustc_hash::FxHashMap;
use vyre_foundation::ir::model::types::{BinOp, UnOp};
use vyre_foundation::ir::{Expr, Ident, Node, Program};

/// The post-substitution folder splits between u32-result ops and
/// bool-result ops. The caller picks the right folder based on the
/// expected return type.
enum FoldResult {
    U32(u32),
    I32(i32),
    Bool(bool),
}

fn fold_u32_binop(op: BinOp, l: u32, r: u32) -> Option<FoldResult> {
    match op {
        BinOp::Add => Some(FoldResult::U32(l.wrapping_add(r))),
        BinOp::Sub => Some(FoldResult::U32(l.wrapping_sub(r))),
        BinOp::Mul => Some(FoldResult::U32(l.wrapping_mul(r))),
        BinOp::Div if r != 0 => Some(FoldResult::U32(l / r)),
        BinOp::Mod if r != 0 => Some(FoldResult::U32(l % r)),
        BinOp::BitAnd => Some(FoldResult::U32(l & r)),
        BinOp::BitOr => Some(FoldResult::U32(l | r)),
        BinOp::BitXor => Some(FoldResult::U32(l ^ r)),
        BinOp::Shl => Some(FoldResult::U32(l.wrapping_shl(r))),
        BinOp::Shr => Some(FoldResult::U32(l.wrapping_shr(r))),
        // Comparison ops produce LitBool. Critical for dead-branch
        // elimination to fire on `if (Var(x) == 0) { … }` patterns
        // after const-prop has substituted `x`'s literal value.
        BinOp::Eq => Some(FoldResult::Bool(l == r)),
        BinOp::Ne => Some(FoldResult::Bool(l != r)),
        BinOp::Lt => Some(FoldResult::Bool(l < r)),
        BinOp::Le => Some(FoldResult::Bool(l <= r)),
        BinOp::Gt => Some(FoldResult::Bool(l > r)),
        BinOp::Ge => Some(FoldResult::Bool(l >= r)),
        BinOp::Min => Some(FoldResult::U32(l.min(r))),
        BinOp::Max => Some(FoldResult::U32(l.max(r))),
        BinOp::AbsDiff => Some(FoldResult::U32(l.abs_diff(r))),
        BinOp::SaturatingAdd => Some(FoldResult::U32(l.saturating_add(r))),
        BinOp::SaturatingSub => Some(FoldResult::U32(l.saturating_sub(r))),
        BinOp::SaturatingMul => Some(FoldResult::U32(l.saturating_mul(r))),
        BinOp::WrappingAdd => Some(FoldResult::U32(l.wrapping_add(r))),
        BinOp::WrappingSub => Some(FoldResult::U32(l.wrapping_sub(r))),
        BinOp::RotateLeft => Some(FoldResult::U32(l.rotate_left(r))),
        BinOp::RotateRight => Some(FoldResult::U32(l.rotate_right(r))),
        _ => None,
    }
}

/// I32 BinOp folder. Mirrors `fold_u32_binop` but evaluates with
/// signed semantics (signed div/mod, signed comparisons). Returns
/// `None` for ops without a well-defined i32 evaluation (signed
/// overflow on Add/Sub/Mul is wrapped as in u32; div/mod by zero
/// is rejected; signed-shift-overflow is wrapped).
fn fold_i32_binop(op: BinOp, l: i32, r: i32) -> Option<FoldResult> {
    match op {
        BinOp::Add => Some(FoldResult::I32(l.wrapping_add(r))),
        BinOp::Sub => Some(FoldResult::I32(l.wrapping_sub(r))),
        BinOp::Mul => Some(FoldResult::I32(l.wrapping_mul(r))),
        // Reject divide-by-zero AND the i32::MIN / -1 overflow case.
        BinOp::Div if r != 0 && !(l == i32::MIN && r == -1) => Some(FoldResult::I32(l / r)),
        BinOp::Mod if r != 0 && !(l == i32::MIN && r == -1) => Some(FoldResult::I32(l % r)),
        BinOp::BitAnd => Some(FoldResult::I32(l & r)),
        BinOp::BitOr => Some(FoldResult::I32(l | r)),
        BinOp::BitXor => Some(FoldResult::I32(l ^ r)),
        BinOp::Shl => Some(FoldResult::I32(l.wrapping_shl(r as u32))),
        BinOp::Shr => Some(FoldResult::I32(l.wrapping_shr(r as u32))),
        BinOp::Eq => Some(FoldResult::Bool(l == r)),
        BinOp::Ne => Some(FoldResult::Bool(l != r)),
        BinOp::Lt => Some(FoldResult::Bool(l < r)),
        BinOp::Le => Some(FoldResult::Bool(l <= r)),
        BinOp::Gt => Some(FoldResult::Bool(l > r)),
        BinOp::Ge => Some(FoldResult::Bool(l >= r)),
        BinOp::Min => Some(FoldResult::I32(l.min(r))),
        BinOp::Max => Some(FoldResult::I32(l.max(r))),
        BinOp::AbsDiff => {
            // i32::abs_diff returns u32; widen to u32 result via the
            // unsigned fold variant. abs_diff is well-defined for the
            // full i32 range (no overflow even at i32::MIN).
            Some(FoldResult::U32(l.abs_diff(r)))
        }
        BinOp::SaturatingAdd => Some(FoldResult::I32(l.saturating_add(r))),
        BinOp::SaturatingSub => Some(FoldResult::I32(l.saturating_sub(r))),
        BinOp::SaturatingMul => Some(FoldResult::I32(l.saturating_mul(r))),
        BinOp::WrappingAdd => Some(FoldResult::I32(l.wrapping_add(r))),
        BinOp::WrappingSub => Some(FoldResult::I32(l.wrapping_sub(r))),
        _ => None,
    }
}

/// Apply constant propagation to `program`. Returns a new Program
/// with `Var(name)` replaced by `LitU32(value)` wherever `name` was
/// let-bound to a literal in an enclosing scope.
pub fn apply_const_prop(program: &Program) -> Program {
    let body: Vec<Node> = match program.entry() {
        [Node::Region { body, .. }] => body.as_ref().clone(),
        entry => entry.to_vec(),
    };
    let mut env = ConstEnv::default();
    let new_body = rewrite_scope(&body, &mut env);

    let new_entry = match program.entry() {
        [Node::Region {
            generator,
            source_region,
            ..
        }] => vec![Node::Region {
            generator: generator.clone(),
            source_region: source_region.clone(),
            body: Arc::new(new_body),
        }],
        _ => new_body,
    };
    program.with_rewritten_entry(new_entry)
}

/// What `Var(name)` resolves to in the current scope.
#[derive(Clone)]
enum ConstVal {
    U32(u32),
    I32(i32),
    Bool(bool),
    /// Var-to-Var alias: `let b = Var(a)`. Every later `Var(b)`
    /// rewrites to `Var(a)` so DCE can drop `let b`.
    Alias(Ident),
}

impl ConstVal {
    fn to_expr(&self) -> Expr {
        match self {
            Self::U32(v) => Expr::LitU32(*v),
            Self::I32(v) => Expr::LitI32(*v),
            Self::Bool(v) => Expr::LitBool(*v),
            Self::Alias(name) => Expr::Var(name.clone()),
        }
    }
}

#[derive(Default)]
struct ConstEnv {
    /// `name → ConstVal` for every Let in scope whose value is a
    /// scalar literal we track (LitU32 / LitBool / Var alias).
    /// Lookup-only; the map shrinks on scope exit.
    bindings: FxHashMap<Ident, ConstVal>,
}

impl ConstEnv {
    fn snapshot(&self) -> FxHashMap<Ident, ConstVal> {
        self.bindings.clone()
    }
    fn restore(&mut self, saved: FxHashMap<Ident, ConstVal>) {
        self.bindings = saved;
    }
    fn record(&mut self, name: Ident, value: &Expr) {
        match value {
            Expr::LitU32(v) => {
                self.bindings.insert(name, ConstVal::U32(*v));
            }
            Expr::LitI32(v) => {
                self.bindings.insert(name, ConstVal::I32(*v));
            }
            Expr::LitBool(v) => {
                self.bindings.insert(name, ConstVal::Bool(*v));
            }
            Expr::Var(other) if other != &name => {
                let resolved = self.resolve_alias(other.clone());
                self.bindings.insert(name, ConstVal::Alias(resolved));
            }
            _ => {
                self.bindings.remove(&name);
            }
        }
    }
    /// Walk Alias chains until we hit a non-alias binding (or the
    /// chain dead-ends at an unbound name). Returns the final name.
    fn resolve_alias(&self, mut name: Ident) -> Ident {
        // Bounded by unique names in scope; alias cycles can't form
        // because IR is structured (no forward refs).
        for _ in 0..64 {
            match self.bindings.get(&name) {
                Some(ConstVal::Alias(next)) if next != &name => {
                    name = next.clone();
                }
                _ => return name,
            }
        }
        name
    }
}

fn rewrite_scope(body: &[Node], env: &mut ConstEnv) -> Vec<Node> {
    let prefix_len = super::encode::reachable_prefix_len(body);
    let mut out = Vec::with_capacity(prefix_len);
    for node in &body[..prefix_len] {
        out.push(rewrite_node(node, env));
    }
    out
}

fn rewrite_node(node: &Node, env: &mut ConstEnv) -> Node {
    match node {
        Node::Let { name, value } => {
            let new_value = rewrite_expr(value, env);
            // Record the binding *after* rewriting the RHS so the
            // RHS sees only enclosing-scope lits (no self-reference).
            env.record(name.clone(), &new_value);
            Node::let_bind(name.clone(), new_value)
        }
        Node::Assign { name, value } => {
            let new_value = rewrite_expr(value, env);
            // Assign mutates the binding; record the new value (or
            // drop if non-literal) for downstream uses.
            env.record(name.clone(), &new_value);
            Node::assign(name.clone(), new_value)
        }
        Node::Store {
            buffer,
            index,
            value,
        } => Node::store(
            buffer.clone(),
            rewrite_expr(index, env),
            rewrite_expr(value, env),
        ),
        Node::If {
            cond,
            then,
            otherwise,
        } => {
            let new_cond = rewrite_expr(cond, env);
            // Each branch sees the parent's bindings on entry but its
            // own additions are scoped to that branch only.
            let saved = env.snapshot();
            let new_then = rewrite_scope(then, env);
            env.restore(saved.clone());
            let new_otherwise = rewrite_scope(otherwise, env);
            env.restore(saved);
            Node::if_then_else(new_cond, new_then, new_otherwise)
        }
        Node::Loop {
            var,
            from,
            to,
            body,
        } => {
            let new_from = rewrite_expr(from, env);
            let new_to = rewrite_expr(to, env);
            // Loop-iter var shadows any enclosing constant  -  remove
            // it from the env for the body's duration.
            let saved = env.snapshot();
            env.bindings.remove(var);
            let new_body = rewrite_scope(body, env);
            env.restore(saved);
            Node::loop_for(var.clone(), new_from, new_to, new_body)
        }
        Node::Block(body) => {
            let saved = env.snapshot();
            let new_body = rewrite_scope(body, env);
            env.restore(saved);
            Node::Block(new_body)
        }
        Node::Region {
            generator,
            source_region,
            body,
        } => {
            let saved = env.snapshot();
            let new_body = rewrite_scope(body.as_slice(), env);
            env.restore(saved);
            Node::Region {
                generator: generator.clone(),
                source_region: source_region.clone(),
                body: Arc::new(new_body),
            }
        }
        // Pass-through: no Expr payload.
        Node::Return
        | Node::Barrier { .. }
        | Node::IndirectDispatch { .. }
        | Node::AsyncWait { .. }
        | Node::Resume { .. }
        | Node::Opaque(_) => node.clone(),
        _ => node.clone(),
    }
}

fn rewrite_expr(expr: &Expr, env: &ConstEnv) -> Expr {
    match expr {
        Expr::Var(name) => {
            if let Some(v) = env.bindings.get(name) {
                v.to_expr()
            } else {
                expr.clone()
            }
        }
        Expr::Load { buffer, index } => Expr::Load {
            buffer: buffer.clone(),
            index: Box::new(rewrite_expr(index, env)),
        },
        Expr::BinOp { op, left, right } => {
            let new_left = rewrite_expr(left, env);
            let new_right = rewrite_expr(right, env);
            // Post-substitution fold for both u32 and i32 BinOps:
            // if both operands are now literals, evaluate directly.
            // Crucial for idempotence and for unblocking dead-branch
            // elimination on `if (Var(x) == 0) { … }` patterns.
            if let (Expr::LitU32(l), Expr::LitU32(r)) = (&new_left, &new_right) {
                match fold_u32_binop(*op, *l, *r) {
                    Some(FoldResult::U32(v)) => return Expr::LitU32(v),
                    Some(FoldResult::I32(v)) => return Expr::LitI32(v),
                    Some(FoldResult::Bool(v)) => return Expr::LitBool(v),
                    None => {}
                }
            }
            if let (Expr::LitI32(l), Expr::LitI32(r)) = (&new_left, &new_right) {
                match fold_i32_binop(*op, *l, *r) {
                    Some(FoldResult::U32(v)) => return Expr::LitU32(v),
                    Some(FoldResult::I32(v)) => return Expr::LitI32(v),
                    Some(FoldResult::Bool(v)) => return Expr::LitBool(v),
                    None => {}
                }
            }
            // Bool comparison-with-literal simplifications:
            //   x == true   → x
            //   x == false  → !x
            //   x != false  → x
            //   x != true   → !x
            //   true == x   → x
            //   false != x  → x
            // Only apply when the surviving operand is itself bool-
            // valued. The folder upstream may produce LitBool from
            // literal pairs; this batch handles the case where one
            // operand stays non-literal.
            match (op, &new_left, &new_right) {
                (BinOp::Eq, _, Expr::LitBool(true)) => return new_left,
                (BinOp::Eq, _, Expr::LitBool(false)) => {
                    return Expr::UnOp {
                        op: UnOp::LogicalNot,
                        operand: Box::new(new_left),
                    };
                }
                (BinOp::Ne, _, Expr::LitBool(false)) => return new_left,
                (BinOp::Ne, _, Expr::LitBool(true)) => {
                    return Expr::UnOp {
                        op: UnOp::LogicalNot,
                        operand: Box::new(new_left),
                    };
                }
                (BinOp::Eq, Expr::LitBool(true), _) => return new_right,
                (BinOp::Eq, Expr::LitBool(false), _) => {
                    return Expr::UnOp {
                        op: UnOp::LogicalNot,
                        operand: Box::new(new_right),
                    };
                }
                (BinOp::Ne, Expr::LitBool(false), _) => return new_right,
                (BinOp::Ne, Expr::LitBool(true), _) => {
                    return Expr::UnOp {
                        op: UnOp::LogicalNot,
                        operand: Box::new(new_right),
                    };
                }
                _ => {}
            }
            // Bool BinOps: And / Or / BitXor / Eq / Ne for two bool
            // literals fold to a Bool. Eq/Ne also act as identity-
            // checks. BitAnd / BitOr behave like And / Or on bool.
            if let (Expr::LitBool(l), Expr::LitBool(r)) = (&new_left, &new_right) {
                let res = match op {
                    BinOp::And | BinOp::BitAnd => Some(*l && *r),
                    BinOp::Or | BinOp::BitOr => Some(*l || *r),
                    BinOp::BitXor => Some(*l ^ *r),
                    BinOp::Eq => Some(*l == *r),
                    BinOp::Ne => Some(*l != *r),
                    _ => None,
                };
                if let Some(v) = res {
                    return Expr::LitBool(v);
                }
            }
            Expr::BinOp {
                op: *op,
                left: Box::new(new_left),
                right: Box::new(new_right),
            }
        }
        Expr::UnOp { op, operand } => {
            let new_operand = rewrite_expr(operand, env);
            // Post-substitution UnOp fold: if the substituted operand
            // is a literal, evaluate the unary op at compile time.
            // Covers Negate / BitNot / LogicalNot / Popcount / Clz /
            // Ctz / ReverseBits / Abs across u32 / i32 / bool. Float
            // unary ops (Sin/Cos/Sqrt/etc.) intentionally skipped to
            // avoid host-vs-target rounding divergence.
            match (op, &new_operand) {
                (UnOp::BitNot, Expr::LitU32(v)) => return Expr::LitU32(!*v),
                (UnOp::BitNot, Expr::LitI32(v)) => return Expr::LitI32(!*v),
                (UnOp::Negate, Expr::LitI32(v)) => {
                    return Expr::LitI32(v.wrapping_neg());
                }
                (UnOp::Negate, Expr::LitU32(v)) => {
                    return Expr::LitU32(v.wrapping_neg());
                }
                (UnOp::LogicalNot, Expr::LitBool(b)) => return Expr::LitBool(!*b),
                (UnOp::LogicalNot, Expr::LitU32(0)) => return Expr::LitBool(true),
                (UnOp::LogicalNot, Expr::LitU32(_)) => return Expr::LitBool(false),
                (UnOp::Popcount, Expr::LitU32(v)) => {
                    return Expr::LitU32(v.count_ones());
                }
                (UnOp::Clz, Expr::LitU32(v)) => return Expr::LitU32(v.leading_zeros()),
                (UnOp::Ctz, Expr::LitU32(v)) => return Expr::LitU32(v.trailing_zeros()),
                (UnOp::ReverseBits, Expr::LitU32(v)) => {
                    return Expr::LitU32(v.reverse_bits());
                }
                (UnOp::Abs, Expr::LitI32(v)) => {
                    return Expr::LitI32(v.wrapping_abs());
                }
                (UnOp::Sign, Expr::LitI32(v)) => {
                    return Expr::LitI32(v.signum());
                }
                _ => {}
            }
            Expr::UnOp {
                op: op.clone(),
                operand: Box::new(new_operand),
            }
        }
        Expr::Select {
            cond,

            true_val,
            false_val,
        } => {
            let new_cond = rewrite_expr(cond, env);
            let new_true = rewrite_expr(true_val, env);
            let new_false = rewrite_expr(false_val, env);
            // Select-fold: when the cond is a constant literal,
            // collapse to the surviving arm.
            match &new_cond {
                Expr::LitBool(true) => return new_true,
                Expr::LitBool(false) => return new_false,
                Expr::LitU32(0) | Expr::LitI32(0) => return new_false,
                Expr::LitU32(_) | Expr::LitI32(_) => return new_true,
                _ => {}
            }
            Expr::Select {
                cond: Box::new(new_cond),
                true_val: Box::new(new_true),
                false_val: Box::new(new_false),
            }
        }
        Expr::Fma { a, b, c } => Expr::Fma {
            a: Box::new(rewrite_expr(a, env)),
            b: Box::new(rewrite_expr(b, env)),
            c: Box::new(rewrite_expr(c, env)),
        },
        Expr::Cast { target, value } => Expr::Cast {
            target: target.clone(),
            value: Box::new(rewrite_expr(value, env)),
        },
        Expr::Atomic {
            op,
            buffer,
            index,
            expected,
            value,
            ordering,
        } => Expr::Atomic {
            op: *op,
            buffer: buffer.clone(),
            index: Box::new(rewrite_expr(index, env)),
            expected: expected.as_ref().map(|e| Box::new(rewrite_expr(e, env))),
            value: Box::new(rewrite_expr(value, env)),
            ordering: *ordering,
        },
        Expr::Call { op_id, args } => Expr::Call {
            op_id: op_id.clone(),
            args: args.iter().map(|a| rewrite_expr(a, env)).collect(),
        },
        Expr::SubgroupBallot { cond } => Expr::SubgroupBallot {
            cond: Box::new(rewrite_expr(cond, env)),
        },
        Expr::SubgroupShuffle { value, lane } => Expr::SubgroupShuffle {
            value: Box::new(rewrite_expr(value, env)),
            lane: Box::new(rewrite_expr(lane, env)),
        },
        Expr::SubgroupReduce { op, value } => Expr::SubgroupReduce {
            op: *op,
            value: Box::new(rewrite_expr(value, env)),
        },
        // Literals + builtins pass through.
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize => expr.clone(),
        _ => expr.clone(),
    }
}
