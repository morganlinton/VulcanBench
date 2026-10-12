//! Workgroup-uniformity analysis for `Expr` nodes.
//!
//! An expression is *uniform* iff every invocation in the same
//! workgroup, evaluating it at the same source position, produces
//! the same value. Uniform `Loop` bounds and `If` conditions keep
//! every invocation in lockstep, so a `Node::Barrier { ordering: vyre::memory_model::MemoryOrdering::SeqCst }` placed in
//! such a body is well-defined under backend barrier semantics.
//!
//! The analyzer is intentionally conservative: anything we cannot
//! statically prove uniform is reported as divergent. False
//! negatives are safe (the validator continues to reject barriers
//! that *would* in fact be uniform); false positives are not (a
//! divergent barrier reaches only some lanes and deadlocks the
//! workgroup or produces undefined results on real hardware).

use crate::ir_inner::model::expr::Expr;
use crate::validate::binding::Binding;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

/// Return `true` when `expr` is statically uniform across the
/// workgroup given the live `scope` of `Var` bindings.
///
/// Uniform: literal scalars, `BufLen { .. }`, `WorkgroupId { .. }`,
/// `Var` bindings flagged uniform, and arithmetic/cast/select trees
/// whose every leaf is uniform.
///
/// Divergent: `InvocationId`, `LocalId`, `SubgroupLocalId`,
/// `SubgroupSize`, `Load`, `Atomic`, every `Subgroup*` op, `Call`,
/// and `Opaque`. A `Var` for which no binding is known is also
/// treated as divergent.
pub(crate) fn is_uniform(expr: &Expr, scope: &FxHashMap<crate::ir::Ident, Binding>) -> bool {
    let mut stack: SmallVec<[&Expr; 32]> = SmallVec::new();
    stack.push(expr);
    while let Some(expr) = stack.pop() {
        match expr {
            Expr::LitU32(_)
            | Expr::LitI32(_)
            | Expr::LitF32(_)
            | Expr::LitBool(_)
            | Expr::BufLen { .. }
            | Expr::WorkgroupId { .. } => {}
            Expr::Var(name) if scope.get(name.as_str()).is_some_and(|b| b.uniform) => {}
            Expr::BinOp { left, right, .. } => {
                stack.push(right);
                stack.push(left);
            }
            Expr::UnOp { operand, .. } => stack.push(operand),
            Expr::Cast { value, .. } => stack.push(value),
            Expr::Select {
                cond,
                true_val,
                false_val,
            } => {
                stack.push(false_val);
                stack.push(true_val);
                stack.push(cond);
            }
            Expr::Fma { a, b, c } => {
                stack.push(c);
                stack.push(b);
                stack.push(a);
            }
            Expr::InvocationId { .. }
            | Expr::LocalId { .. }
            | Expr::SubgroupLocalId
            | Expr::SubgroupSize
            | Expr::Var(_)
            | Expr::Load { .. }
            | Expr::Call { .. }
            | Expr::Atomic { .. }
            | Expr::SubgroupBallot { .. }
            | Expr::SubgroupShuffle { .. }
            | Expr::SubgroupReduce { .. }
            | Expr::Opaque(_) => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{DataType, Expr, Ident};

    fn empty_scope() -> FxHashMap<crate::ir::Ident, Binding> {
        FxHashMap::default()
    }

    fn scope_with_uniform(name: &str) -> FxHashMap<crate::ir::Ident, Binding> {
        let mut scope = FxHashMap::default();
        scope.insert(
            crate::ir::Ident::from(name),
            Binding {
                ty: DataType::U32,
                ty_known: true,
                mutable: true,
                uniform: true,
            },
        );
        scope
    }

    fn scope_with_divergent(name: &str) -> FxHashMap<crate::ir::Ident, Binding> {
        let mut scope = FxHashMap::default();
        scope.insert(
            crate::ir::Ident::from(name),
            Binding {
                ty: DataType::U32,
                ty_known: true,
                mutable: true,
                uniform: false,
            },
        );
        scope
    }

    #[test]
    fn literals_are_uniform() {
        let scope = empty_scope();
        assert!(is_uniform(&Expr::u32(42), &scope));
        assert!(is_uniform(&Expr::f32(std::f32::consts::PI), &scope));
        assert!(is_uniform(&Expr::LitBool(true), &scope));
        assert!(is_uniform(&Expr::i32(-1), &scope));
    }

    #[test]
    fn invocation_id_is_divergent() {
        let scope = empty_scope();
        assert!(!is_uniform(&Expr::InvocationId { axis: 0 }, &scope));
    }

    #[test]
    fn workgroup_id_is_uniform() {
        let scope = empty_scope();
        assert!(is_uniform(&Expr::WorkgroupId { axis: 0 }, &scope));
    }

    #[test]
    fn uniform_var_is_uniform() {
        let scope = scope_with_uniform("x");
        assert!(is_uniform(&Expr::Var(Ident::from("x")), &scope));
    }

    #[test]
    fn divergent_var_is_divergent() {
        let scope = scope_with_divergent("x");
        assert!(!is_uniform(&Expr::Var(Ident::from("x")), &scope));
    }

    #[test]
    fn unknown_var_is_divergent() {
        let scope = empty_scope();
        assert!(!is_uniform(&Expr::Var(Ident::from("unknown")), &scope));
    }

    #[test]
    fn binop_of_uniform_is_uniform() {
        let scope = empty_scope();
        assert!(is_uniform(&Expr::add(Expr::u32(1), Expr::u32(2)), &scope));
    }

    #[test]
    fn binop_with_divergent_is_divergent() {
        let scope = empty_scope();
        let expr = Expr::add(Expr::u32(1), Expr::InvocationId { axis: 0 });
        assert!(!is_uniform(&expr, &scope));
    }

    #[test]
    fn load_is_always_divergent() {
        let scope = empty_scope();
        assert!(!is_uniform(&Expr::load("buf", Expr::u32(0)), &scope));
    }

    #[test]
    fn fma_uniform_when_all_uniform() {
        let scope = empty_scope();
        let fma = Expr::Fma {
            a: Box::new(Expr::f32(1.0)),
            b: Box::new(Expr::f32(2.0)),
            c: Box::new(Expr::f32(3.0)),
        };
        assert!(is_uniform(&fma, &scope));
    }
}
