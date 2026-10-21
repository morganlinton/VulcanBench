//! P4.2  -  Canonical-form pass.
//!
//! Rewrites `Program` IR into a canonical shape so semantically-equal
//! Programs have byte-equal wire output. This is the foundation the
//! content-addressed pipeline cache (P4.3) pins against: two Cat-A
//! authors who write the same computation via different spellings
//! get the same fingerprint after `canonicalize(program)`.
//!
//! Rules applied:
//!
//! 1. **Hoist literal operands to the right** on commutative BinOps
//!    so `1 + a` canonicalizes to `a + 1`. Non-literal operand order
//!    is preserved because IEEE-754 NaN payload propagation makes
//!    `a + b` and `b + a` observably different for float operations.
//!    Commutativity is looked up in `vyre-spec::BinOp` (Add/Mul,
//!    And/Or/Xor, Eq/Ne). Min/Max are intentionally excluded.
//! 2. **Hoist literal-left**: when only one operand of a commutative
//!    op is a literal, order is literal-on-right. Canonical form
//!    for constant folding.
//!
//! Rules owned by CSE / DCE / rewrite passes:
//!
//! - Expression-CSE (lives in `cse.rs`).
//! - Identity-fold (`x + 0 → x`)  -  requires `AlgebraicLaw::Identity`
//!   registration lookup; next pass.
//! - Associativity-rearranging  -  left-fold stays left, no right-
//!   rotation.
//! - **Reflexive-comparison value folding (`x == x → true`,
//!   `x != x → false`) is deliberately NOT done here.** canonicalize is
//!   type-blind (it has no `DataType` for a bare `Var`), and reflexive
//!   comparison is *unsound* for float operands: a float `x` that is
//!   `NaN` at runtime makes `x == x` false and `x != x` true under
//!   IEEE-754, which the reference oracle (`vyre-reference`
//!   `binop_f32`) and the SPIR-V emitter (`OpFOrdEqual`) both honor.
//!   Folding it to a bool literal here would (a) miscompile NaN checks
//!   and (b) corrupt the content-addressed cache by giving the
//!   genuinely-distinct programs `x == x` and `true` the same
//!   fingerprint. Type-aware value folding lives in `const_fold`, where
//!   the operand type can be proven before the fold fires.
//!
//! The pass is idempotent: `canonicalize(canonicalize(p)) ==
//! canonicalize(p)` on every valid `Program`.

use crate::ir_inner::model::expr::Expr;
use crate::ir_inner::model::node::Node;
use crate::ir_inner::model::program::Program;
use vyre_spec::BinOp;

/// Run the canonical-form pass on `program`, returning the
/// canonicalized form with `validated=false` so downstream passes
/// re-check the rewritten shape.
#[must_use]
pub fn run(program: Program) -> Program {
    // VYRE_IR_HOTSPOTS CRIT: `program.entry().to_vec()` cloned the
    // whole `Vec<Node>` unconditionally. `Program::map_entry` moves the
    // entry out of its Arc when uniquely owned (the common case  -
    // canonicalize runs on programs the pass pipeline has exclusive
    // access to) and rebuilds on the remaining fields without an
    // intermediate scaffold allocation.
    program.map_entry(canonicalize_nodes)
}

/// Run the canonical-form pass from a borrowed `Program`.
///
/// This is for hot paths such as pipeline fingerprinting where the caller owns
/// a shared `Program` and only needs a temporary canonical serialization view.
/// It preserves the program scaffold by reference and clones only the entry
/// nodes that are actually rewritten.
#[must_use]
pub fn run_borrowed(program: &Program) -> Program {
    program.with_rewritten_entry(canonicalize_nodes_borrowed(program.entry()))
}

fn canonicalize_nodes_borrowed(nodes: &[Node]) -> Vec<Node> {
    nodes.iter().cloned().map(canonicalize_node).collect()
}

fn canonicalize_nodes(nodes: Vec<Node>) -> Vec<Node> {
    nodes.into_iter().map(canonicalize_node).collect()
}

fn canonicalize_node(node: Node) -> Node {
    match node {
        Node::Let { name, value } => Node::Let {
            name,
            value: canonicalize_expr(value),
        },
        Node::Assign { name, value } => Node::Assign {
            name,
            value: canonicalize_expr(value),
        },
        Node::Store {
            buffer,
            index,
            value,
        } => Node::Store {
            buffer,
            index: canonicalize_expr(index),
            value: canonicalize_expr(value),
        },
        Node::If {
            cond,
            then,
            otherwise,
        } => Node::If {
            cond: canonicalize_expr(cond),
            then: canonicalize_nodes(then),
            otherwise: canonicalize_nodes(otherwise),
        },
        Node::Loop {
            var,
            from,
            to,
            body,
        } => Node::Loop {
            var,
            from: canonicalize_expr(from),
            to: canonicalize_expr(to),
            body: canonicalize_nodes(body),
        },
        Node::Block(children) => Node::Block(canonicalize_nodes(children)),
        Node::Region {
            generator,
            source_region,
            body,
        } => {
            // VYRE_IR_HOTSPOTS CRIT: avoid cloning the inner Vec<Node>
            // when the Arc is uniquely owned  -  try_unwrap hands the
            // Vec back directly.
            let body_vec = match std::sync::Arc::try_unwrap(body) {
                Ok(v) => v,
                Err(arc) => (*arc).clone(),
            };
            Node::Region {
                generator,
                source_region,
                body: std::sync::Arc::new(canonicalize_nodes(body_vec)),
            }
        }
        other => other,
    }
}

fn canonicalize_expr(expr: Expr) -> Expr {
    match expr {
        Expr::BinOp { op, left, right } => {
            let mut l = canonicalize_expr(*left);
            let mut r = canonicalize_expr(*right);
            if is_commutative_binop(op) {
                // Rule: literal-on-right for all commutative ops.
                // Non-literals are sorted ONLY for bitwise/boolean ops
                // because IEEE-754 float Add/Mul NaN payload propagation
                // is not commutative at the bit level.
                let l_is_lit = is_literal(&l);
                let r_is_lit = is_literal(&r);
                let should_swap = match (l_is_lit, r_is_lit) {
                    (true, false) => true,
                    (false, true) => false,
                    _ => is_safe_to_sort_nonliterals(op) && expr_sort_key(&l) > expr_sort_key(&r),
                };
                if should_swap {
                    std::mem::swap(&mut l, &mut r);
                }
            }
            // NOTE: reflexive comparison (`x == x`, `x != x`) is NOT folded
            // here. canonicalize is type-blind and the fold is unsound for
            // float NaN — see the module-level doc. Operand-order
            // canonicalization (above) is the only rewrite Eq/Ne gets; that
            // is sound for every type because `a == b` ≡ `b == a`.
            Expr::BinOp {
                op,
                left: Box::new(l),
                right: Box::new(r),
            }
        }
        Expr::UnOp { op, operand } => Expr::UnOp {
            op,
            operand: Box::new(canonicalize_expr(*operand)),
        },
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => Expr::Select {
            cond: Box::new(canonicalize_expr(*cond)),
            true_val: Box::new(canonicalize_expr(*true_val)),
            false_val: Box::new(canonicalize_expr(*false_val)),
        },
        Expr::Fma { a, b, c } => Expr::Fma {
            a: Box::new(canonicalize_expr(*a)),
            b: Box::new(canonicalize_expr(*b)),
            c: Box::new(canonicalize_expr(*c)),
        },
        Expr::Cast { target, value } => Expr::Cast {
            target,
            value: Box::new(canonicalize_expr(*value)),
        },
        Expr::Load { buffer, index } => Expr::Load {
            buffer,
            index: Box::new(canonicalize_expr(*index)),
        },
        Expr::Atomic {
            op,
            buffer,
            index,
            expected,
            value,
            ordering,
        } => Expr::Atomic {
            op,
            buffer,
            index: Box::new(canonicalize_expr(*index)),
            expected: expected.map(|e| Box::new(canonicalize_expr(*e))),
            value: Box::new(canonicalize_expr(*value)),
            ordering,
        },
        // Compound operand-bearing exprs must canonicalize their children too,
        // or a commutative BinOp nested inside them (e.g. `subgroup_add(1 + a)`,
        // `f(1 + a)`) stays un-normalized and two semantically-equal Programs
        // produce different wire bytes — fragmenting the content-addressed
        // pipeline cache the canonical form is the foundation for. Fields that
        // are not operands (`Call.op_id`, `SubgroupReduce.op`) are preserved.
        Expr::Call { op_id, args } => Expr::Call {
            op_id,
            args: args.into_iter().map(canonicalize_expr).collect(),
        },
        Expr::SubgroupBallot { cond } => Expr::SubgroupBallot {
            cond: Box::new(canonicalize_expr(*cond)),
        },
        Expr::SubgroupShuffle { value, lane } => Expr::SubgroupShuffle {
            value: Box::new(canonicalize_expr(*value)),
            lane: Box::new(canonicalize_expr(*lane)),
        },
        Expr::SubgroupReduce { op, value } => Expr::SubgroupReduce {
            op,
            value: Box::new(canonicalize_expr(*value)),
        },
        other => other,
    }
}

fn is_commutative_binop(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Add
            | BinOp::Mul
            | BinOp::BitAnd
            | BinOp::BitOr
            | BinOp::BitXor
            | BinOp::Eq
            | BinOp::Ne
            | BinOp::And
            | BinOp::Or
    )
}

/// Operations where sorting non-literal operands is semantics-preserving
/// for all Vyre types (integers and booleans). Add/Mul are excluded
/// because IEEE-754 NaN payload propagation makes them non-commutative
/// at the bit level for float operands.
fn is_safe_to_sort_nonliterals(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::BitAnd
            | BinOp::BitOr
            | BinOp::BitXor
            | BinOp::Eq
            | BinOp::Ne
            | BinOp::And
            | BinOp::Or
    )
}

fn is_literal(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::LitU32(_) | Expr::LitI32(_) | Expr::LitF32(_) | Expr::LitBool(_)
    )
}

/// Total order used to break ties on commutative-operand sort.
/// Stable across runs: only depends on the Expr's structural key.
/// Goal is determinism, not perceived "smaller is simpler" meaning.
fn expr_sort_key(expr: &Expr) -> u64 {
    match expr {
        Expr::LitU32(v) => u64::from(*v),
        Expr::LitI32(v) => u64::from(u32::from_ne_bytes(v.to_ne_bytes())),
        Expr::LitF32(v) => u64::from(v.to_bits()),
        Expr::LitBool(v) => u64::from(*v),
        // VYRE_IR_HOTSPOTS LOW: `Ident` carries a precomputed hash
        // (see ident.rs::cached_hash). Using it here replaces the
        // per-comparison FNV walk with a single u64 field read, so
        // the commutative-sort is O(n log n) in comparisons instead
        // of O(n * |name| * log n).
        Expr::Var(name) => name.cached_hash() & 0xFFFF_FFFF,
        Expr::Load { buffer, .. } => 0x1_0000_0000 | buffer.cached_hash(),
        Expr::BufLen { buffer } => 0x2_0000_0000 | buffer.cached_hash(),
        Expr::InvocationId { axis } => 0x3_0000_0000 | u64::from(*axis),
        Expr::WorkgroupId { axis } => 0x4_0000_0000 | u64::from(*axis),
        Expr::LocalId { axis } => 0x5_0000_0000 | u64::from(*axis),
        Expr::BinOp { .. } => 0x6_0000_0000,
        Expr::UnOp { .. } => 0x7_0000_0000,
        Expr::Call { .. } => 0x8_0000_0000,
        Expr::Fma { .. } => 0x9_0000_0000,
        Expr::Select { .. } => 0xa_0000_0000,
        Expr::Cast { .. } => 0xb_0000_0000,
        Expr::Atomic { .. } => 0xc_0000_0000,
        Expr::SubgroupBallot { .. } => 0xd_0000_0000,
        Expr::SubgroupShuffle { .. } => 0xe_0000_0000,
        Expr::SubgroupReduce { .. } => 0xf_0000_0000,
        Expr::SubgroupLocalId | Expr::SubgroupSize => 0x20_0000_0000,
        Expr::Opaque(_) => 0x10_0000_0000,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferDecl, DataType, Expr as E, Ident, Program};

    fn scalar_out_prog(body: Vec<Node>) -> Program {
        Program::wrapped(
            vec![BufferDecl::read_write("out", 0, DataType::U32).with_count(1)],
            [1, 1, 1],
            body,
        )
    }

    #[test]
    fn expr_sort_key_uses_cached_ident_hash_for_name_bearing_exprs() {
        let name = Ident::from("very_long_identifier_that_must_not_be_rehashed_per_compare");

        assert_eq!(
            expr_sort_key(&Expr::Var(name.clone())),
            name.cached_hash() & 0xFFFF_FFFF
        );
        assert_eq!(
            expr_sort_key(&Expr::load(name.clone(), Expr::u32(0))),
            0x1_0000_0000 | name.cached_hash()
        );
        assert_eq!(
            expr_sort_key(&Expr::buf_len(name.clone())),
            0x2_0000_0000 | name.cached_hash()
        );
    }

    #[test]
    fn commutative_add_operand_order_canonicalized() {
        // `a + 1` and `1 + a` must canonicalize to the same IR.
        let p1 = scalar_out_prog(vec![Node::store(
            "out",
            E::u32(0),
            E::add(E::var("a"), E::u32(1)),
        )]);
        let p2 = scalar_out_prog(vec![Node::store(
            "out",
            E::u32(0),
            E::add(E::u32(1), E::var("a")),
        )]);
        let c1 = run(p1).to_wire().unwrap();
        let c2 = run(p2).to_wire().unwrap();
        assert_eq!(c1, c2, "canonicalize(a+1) == canonicalize(1+a)");
    }

    #[test]
    fn noncommutative_sub_order_preserved() {
        // `a - b` must NOT canonicalize to `b - a`; sub is non-commutative.
        let p1 = scalar_out_prog(vec![Node::store(
            "out",
            E::u32(0),
            E::sub(E::var("a"), E::var("b")),
        )]);
        let p2 = scalar_out_prog(vec![Node::store(
            "out",
            E::u32(0),
            E::sub(E::var("b"), E::var("a")),
        )]);
        let c1 = run(p1).to_wire().unwrap();
        let c2 = run(p2).to_wire().unwrap();
        assert_ne!(c1, c2, "sub must preserve operand order");
    }

    #[test]
    fn canonicalize_is_idempotent() {
        let body = vec![Node::store(
            "out",
            E::u32(0),
            E::add(E::u32(5), E::mul(E::var("b"), E::var("a"))),
        )];
        let p = scalar_out_prog(body);
        let once = run(p).to_wire().unwrap();
        let twice = run(Program::from_wire(&once).unwrap()).to_wire().unwrap();
        assert_eq!(once, twice, "canonicalize must be idempotent");
    }

    #[test]
    fn literal_hoists_to_right_on_commutative_ops() {
        // `3 + a` canonicalizes to `a + 3`.
        let p = scalar_out_prog(vec![Node::store(
            "out",
            E::u32(0),
            E::add(E::u32(3), E::var("a")),
        )]);
        let canonical = run(p);
        let body = crate::test_util::region_body(&canonical);
        match &body[0] {
            Node::Store { value, .. } => match value {
                Expr::BinOp { left, right, .. } => {
                    assert!(
                        !is_literal(left),
                        "literal must not be on the left after canonicalize"
                    );
                    assert!(
                        is_literal(right),
                        "literal must be on the right after canonicalize"
                    );
                }
                _ => panic!("expected BinOp"),
            },
            other => panic!("expected Store, got {other:?}"),
        }
    }

    #[test]
    fn inner_binop_also_canonicalized() {
        // Nested commutative BinOps canonicalize recursively. The
        // inner mul's literal (2) must end up on the right of the
        // mul regardless of the outer add's layout. We find the
        // mul wherever canonicalize places it and assert that.
        let body = vec![Node::store(
            "out",
            E::u32(0),
            E::add(E::var("outer"), E::mul(E::u32(2), E::var("inner"))),
        )];
        let canonical = run(scalar_out_prog(body));

        // Walk the value tree and find the Mul; assert its literal
        // operand is on the right.
        fn find_mul_and_check(e: &Expr) {
            match e {
                Expr::BinOp {
                    op: BinOp::Mul,
                    left,
                    right,
                } => {
                    // Exactly one side is the literal; it must be
                    // the right after canonicalize.
                    let l_lit = is_literal(left);
                    let r_lit = is_literal(right);
                    assert!(l_lit ^ r_lit, "test expects exactly one literal operand");
                    assert!(
                        r_lit && !l_lit,
                        "literal must canonicalize to the right of Mul"
                    );
                }
                Expr::BinOp { left, right, .. } => {
                    find_mul_and_check(left);
                    find_mul_and_check(right);
                }
                _ => {}
            }
        }
        let entry_body = crate::test_util::region_body(&canonical);
        match &entry_body[0] {
            Node::Store { value, .. } => find_mul_and_check(value),
            other => panic!("expected Store, got {other:?}"),
        }
    }

    #[test]
    fn eq_same_var_not_folded_by_canonicalize() {
        // canonicalize must NOT fold `a == a` to `true`. The pass is
        // type-blind, and for a float `a` that is NaN at runtime
        // `a == a` is *false* under IEEE-754 (the reference oracle's
        // `binop_f32` and the SPIR-V `OpFOrdEqual` emitter both agree).
        // Folding it to a bool literal would miscompile NaN checks and
        // collide distinct programs in the content-addressed cache.
        // The Eq node is preserved verbatim (operand order unchanged
        // because both operands are the same Var).
        let p = scalar_out_prog(vec![Node::let_bind("t", E::eq(E::var("a"), E::var("a")))]);
        let canonical = run(p);
        let entry_body = crate::test_util::region_body(&canonical);
        match &entry_body[0] {
            Node::Let { value, .. } => match value {
                Expr::BinOp {
                    op: BinOp::Eq,
                    left,
                    right,
                } => {
                    assert_eq!(left.as_ref(), &Expr::var("a"), "lhs preserved as Var(a)");
                    assert_eq!(right.as_ref(), &Expr::var("a"), "rhs preserved as Var(a)");
                }
                other => panic!("a == a must be preserved as an Eq BinOp, got {other:?}"),
            },
            other => panic!("expected Let, got {other:?}"),
        }
    }

    #[test]
    fn ne_same_var_not_folded_by_canonicalize() {
        // Symmetric to `eq_same_var_not_folded_by_canonicalize`: for a
        // float NaN, `a != a` is *true*, not false, so canonicalize must
        // not fold it to a bool literal. The Ne node is preserved.
        let p = scalar_out_prog(vec![Node::let_bind("t", E::ne(E::var("a"), E::var("a")))]);
        let canonical = run(p);
        let entry_body = crate::test_util::region_body(&canonical);
        match &entry_body[0] {
            Node::Let { value, .. } => match value {
                Expr::BinOp {
                    op: BinOp::Ne,
                    left,
                    right,
                } => {
                    assert_eq!(left.as_ref(), &Expr::var("a"), "lhs preserved as Var(a)");
                    assert_eq!(right.as_ref(), &Expr::var("a"), "rhs preserved as Var(a)");
                }
                other => panic!("a != a must be preserved as an Ne BinOp, got {other:?}"),
            },
            other => panic!("expected Let, got {other:?}"),
        }
    }

    #[test]
    fn canonicalize_descends_into_subgroup_reduce_operand() {
        // The pass's contract is "semantically-equal Programs have byte-equal
        // wire output" (the content-addressed pipeline-cache fingerprint).
        // A commutative BinOp nested inside a subgroup operand must therefore
        // canonicalize just like a top-level one: `subgroup_add(1 + a)` and
        // `subgroup_add(a + 1)` are the same computation and must produce the
        // same wire bytes. If canonicalize_expr's catch-all swallows
        // SubgroupReduce, the inner `1 + a` stays un-normalized and the two
        // fingerprints diverge (cache fragmentation).
        let p1 = scalar_out_prog(vec![Node::store(
            "out",
            E::u32(0),
            E::subgroup_add(E::add(E::u32(1), E::var("a"))),
        )]);
        let p2 = scalar_out_prog(vec![Node::store(
            "out",
            E::u32(0),
            E::subgroup_add(E::add(E::var("a"), E::u32(1))),
        )]);
        let c1 = run(p1).to_wire().unwrap();
        let c2 = run(p2).to_wire().unwrap();
        assert_eq!(
            c1, c2,
            "canonicalize(subgroup_add(1 + a)) must equal canonicalize(subgroup_add(a + 1))"
        );
    }

    #[test]
    fn canonicalize_descends_into_call_args() {
        // Same contract for Call arguments: `f(1 + a)` and `f(a + 1)` are the
        // same computation and must canonicalize to the same wire bytes. A
        // catch-all that skips Call leaves the arg's commutative BinOp
        // un-normalized.
        let p1 = scalar_out_prog(vec![Node::store(
            "out",
            E::u32(0),
            E::call("f", vec![E::add(E::u32(1), E::var("a"))]),
        )]);
        let p2 = scalar_out_prog(vec![Node::store(
            "out",
            E::u32(0),
            E::call("f", vec![E::add(E::var("a"), E::u32(1))]),
        )]);
        let c1 = run(p1).to_wire().unwrap();
        let c2 = run(p2).to_wire().unwrap();
        assert_eq!(
            c1, c2,
            "canonicalize(f(1 + a)) must equal canonicalize(f(a + 1))"
        );
    }

    #[test]
    fn eq_different_vars_unchanged() {
        let p = scalar_out_prog(vec![Node::let_bind("t", E::eq(E::var("a"), E::var("b")))]);
        let canonical = run(p);
        let entry_body = crate::test_util::region_body(&canonical);
        match &entry_body[0] {
            Node::Let { value, .. } => match value {
                Expr::BinOp {
                    op: BinOp::Eq,
                    left,
                    right,
                } => {
                    let l = left.as_ref();
                    let r = right.as_ref();
                    assert!(
                        (l == &Expr::var("a") && r == &Expr::var("b"))
                            || (l == &Expr::var("b") && r == &Expr::var("a"))
                    );
                }
                other => panic!("expected Eq BinOp, got {other:?}"),
            },
            other => panic!("expected Let, got {other:?}"),
        }
    }
}
