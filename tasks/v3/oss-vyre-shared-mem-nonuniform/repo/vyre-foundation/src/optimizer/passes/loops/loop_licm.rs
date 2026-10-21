//! ROADMAP A17  -  Loop-invariant code motion.
//!
//! Op id: `vyre-foundation::optimizer::passes::loop_licm`.
//! Soundness: `Exact`  -  a `Node::Let` inside a loop body whose value
//! expression depends on neither the loop induction variable nor any
//! variable mutated inside the loop, AND whose evaluation has no
//! observable side effect, computes the same value on every iteration
//! and can be hoisted out of the loop without changing observable
//! semantics. The hoisted bind retains its original name; references
//! inside the loop body resolve to the now-outer Let by lexical
//! scoping. Cost direction: monotone-down on `node_count` per loop
//! iteration (one fewer Let evaluation per trip); the outer body
//! grows by one Let, so cumulative `node_count` may increase by 1
//! while per-iteration cost decreases by the loop trip count. For
//! any non-trivial trip count this is a win. Preserves: every
//! analysis. Invalidates: nothing.
//!
//! ## Rule
//!
//! ```text
//! Node::Loop { var, from, to, body: [
//!     ...siblings_a...,
//!     Node::Let { name: x, value: v },     // v references neither `var`
//!     ...siblings_b...,                    // nor any name mutated by `body`
//! ] }
//! →
//! Node::Let { name: x, value: v },         // hoisted before the loop
//! Node::Loop { var, from, to, body: [
//!     ...siblings_a...,
//!     ...siblings_b...,
//! ] }
//! ```
//!
//! ## Conservatism
//!
//! - Hoists only `Node::Let` (single-assignment binding). `Node::Assign`
//!   inside a loop is by definition loop-carrying and cannot be hoisted.
//! - Skips Lets whose value expression contains `Expr::Load`, `Expr::Atomic`,
//!   `Expr::Call`, `Expr::Opaque`, or `Expr::BufLen`  -  these can be
//!   side-effecting or order-sensitive. `expr_is_pure_constant_in_loop`
//!   below names every variant explicitly.
//! - Skips Lets whose value references the loop var, any other Let
//!   shadowed inside the body that is itself loop-carrying, or any
//!   `Node::Assign` target in the body.
//! - Skips Lets whose name is bound elsewhere in the enclosing body's
//!   subtree (`count_bound_names != 1`). Hoisting binds the name in the
//!   enclosing scope, so a name shared with a sibling loop / sibling Let /
//!   loop var would become a duplicate sibling binding (V032) or shadow
//!   (V008). The block-scoped IR pops loop-body bindings at loop exit, so
//!   two sibling loops binding the same `t` are legal pre-hoist but collide
//!   once both are flat-spliced into the shared body.
//! - Walks one container body at a time. Nested loops get their own
//!   pass invocation through the recursion.

use crate::ir::{Expr, Ident, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::bound_names::count_bound_names;
use rustc_hash::{FxHashMap, FxHashSet};

/// Hoist loop-invariant `Node::Let` bindings out of `Node::Loop`
/// bodies whenever they have no observable side effect and reference
/// no name mutated inside the loop.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "loop_licm",
    requires = [],
    invalidates = [],
    phase = "loop",
    boundary_class = "abi_preserving",
    cost_model_family = "loop"
)]
pub struct LoopLicm;

impl LoopLicm {
    /// Skip the pass when no body in the program contains a Loop
    /// whose first nested Let could be hoisted.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // Hoisting requires both a Loop AND a Let inside it. Either
        // missing → recursive walk would find nothing.
        use crate::ir::stats::{NODE_KIND_LET, NODE_KIND_LOOP};
        let stats = program.stats();
        if !stats.has_any_node_kind(NODE_KIND_LOOP) || !stats.has_any_node_kind(NODE_KIND_LET) {
            return PassAnalysis::SKIP;
        }
        if program.entry().iter().any(has_hoistable_let_in_any_loop) {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the program; rewrite every container body that owns a
    /// `Node::Loop` whose interior has at least one hoistable Let.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| hoist_in_body(entry, &mut changed));
        PassResult { program, changed }
    }
}

/// Walk one container body, recursing into every nested container,
/// and hoist invariant Lets out of every `Node::Loop` we find.
fn hoist_in_body(body: Vec<Node>, changed: &mut bool) -> Vec<Node> {
    // Names bound anywhere in THIS enclosing body's subtree, with multiplicity.
    // A Let hoisted out of a loop lands in this body's scope, so it is only
    // safe to flat-splice when its name is bound nowhere else here (count == 1);
    // otherwise two sibling loops binding the same name would each hoist it and
    // produce a duplicate sibling binding the validator rejects (V032).
    // Hoisting only moves a binding WITHIN this subtree, so these counts stay
    // valid for every hoist that lands in `body`.
    let mut bound_counts: FxHashMap<Ident, usize> = FxHashMap::default();
    count_bound_names(&body, &mut bound_counts);
    let mut out: Vec<Node> = Vec::with_capacity(body.len());
    for node in body {
        match node {
            Node::Loop {
                var,
                from,
                to,
                body: loop_body,
            } => {
                let inner = hoist_in_body(loop_body, changed);
                let (hoisted, kept) = split_invariant_lets(&var, inner, &bound_counts, changed);
                for h in hoisted {
                    out.push(h);
                }
                out.push(Node::Loop {
                    var,
                    from,
                    to,
                    body: kept,
                });
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                let then = hoist_in_body(then, changed);
                let otherwise = hoist_in_body(otherwise, changed);
                out.push(Node::If {
                    cond,
                    then,
                    otherwise,
                });
            }
            Node::Block(inner) => {
                out.push(Node::Block(hoist_in_body(inner, changed)));
            }
            Node::Region {
                generator,
                source_region,
                body,
            } => {
                let body_vec =
                    std::sync::Arc::try_unwrap(body).unwrap_or_else(|arc| (*arc).clone());
                let body_vec = hoist_in_body(body_vec, changed);
                out.push(Node::Region {
                    generator,
                    source_region,
                    body: std::sync::Arc::new(body_vec),
                });
            }
            other => out.push(other),
        }
    }
    out
}

/// Split a loop body into (hoistable Lets, retained body). A Let is
/// hoistable when its value expression depends on no name in
/// `mutated_names` (which always contains the loop var) and has no
/// observable side effect. The hoisted Lets land above the Loop in
/// the order they originally appeared.
fn split_invariant_lets(
    loop_var: &Ident,
    body: Vec<Node>,
    enclosing_bound_counts: &FxHashMap<Ident, usize>,
    changed: &mut bool,
) -> (Vec<Node>, Vec<Node>) {
    let mut mutated: FxHashSet<Ident> = FxHashSet::default();
    mutated.insert(loop_var.clone());
    collect_assigned_and_let_bound_names(&body, &mut mutated);
    let mut assigned: FxHashSet<Ident> = FxHashSet::default();
    collect_assigned_names(&body, &mut assigned);

    // Names that have been hoisted so far in this pass; references to
    // them from later Lets in the body are still safe to hoist.
    let mut hoisted: Vec<Node> = Vec::new();
    let mut kept: Vec<Node> = Vec::with_capacity(body.len());
    for node in body {
        match node {
            Node::Let { name, value } => {
                let any_dependency_mutated = expr_references_any(&value, &mutated);
                let name_reassigned_in_loop = assigned.contains(&name);
                // SCOPE-EXTENSION guard. Hoisting binds `name` in the enclosing
                // body. That is sound only if `name` is bound nowhere else in
                // that body's subtree (count == 1, i.e. this Let is its sole
                // binding); otherwise the hoist would duplicate a sibling
                // binding (V032) or shadow a loop var / disjoint child binding
                // (V008). Two sibling loops each binding `t` count 2 and are
                // both held back -- conservative but always well-scoped.
                let name_unique_in_enclosing = enclosing_bound_counts.get(&name) == Some(&1);
                if !name_reassigned_in_loop
                    && !any_dependency_mutated
                    && name_unique_in_enclosing
                    && expr_is_observably_free(&value)
                {
                    *changed = true;
                    // The hoisted Let no longer counts as
                    // loop-mutated; the in-body references to `name`
                    // resolve to the outer-scope Let we just produced.
                    mutated.remove(&name);
                    hoisted.push(Node::let_bind(name.as_str(), *Box::new(value)));
                } else {
                    kept.push(Node::Let { name, value });
                }
            }
            other => kept.push(other),
        }
    }
    (hoisted, kept)
}

/// Walk `nodes` collecting every name that appears as the target of
/// `Node::Assign` or `Node::Let` (i.e. anything potentially mutated
/// inside the loop body, including freshly-bound names that we then
/// reassign elsewhere).
fn collect_assigned_and_let_bound_names(nodes: &[Node], out: &mut FxHashSet<Ident>) {
    for node in nodes {
        match node {
            Node::Let { name, .. } | Node::Assign { name, .. } => {
                out.insert(name.clone());
            }
            Node::If {
                then, otherwise, ..
            } => {
                collect_assigned_and_let_bound_names(then, out);
                collect_assigned_and_let_bound_names(otherwise, out);
            }
            Node::Loop { body, .. } | Node::Block(body) => {
                collect_assigned_and_let_bound_names(body, out);
            }
            Node::Region { body, .. } => {
                collect_assigned_and_let_bound_names(body, out);
            }
            _ => {}
        }
    }
}

/// Walk `nodes` collecting only true mutation targets. A `Let` inside a loop
/// can be hoisted only when the name is never assigned in that loop body:
/// `let emit = 0; if (...) { emit = 1; }` is a per-iteration reset, not an
/// invariant outer binding.
fn collect_assigned_names(nodes: &[Node], out: &mut FxHashSet<Ident>) {
    for node in nodes {
        match node {
            Node::Assign { name, .. } => {
                out.insert(name.clone());
            }
            Node::If {
                then, otherwise, ..
            } => {
                collect_assigned_names(then, out);
                collect_assigned_names(otherwise, out);
            }
            Node::Loop { body, .. } | Node::Block(body) => {
                collect_assigned_names(body, out);
            }
            Node::Region { body, .. } => {
                collect_assigned_names(body, out);
            }
            _ => {}
        }
    }
}

/// True iff `expr` references at least one name in `mutated`.
/// Like `expr_references_any` but pretends `ignore` is not in `mutated`.
/// Used by `has_hoistable_let_in_any_loop` so we can ask "does this Let
/// depend on any in-loop name OTHER than its own bound name" without
/// cloning `mutated` per Let.
fn expr_references_any_except(expr: &Expr, mutated: &FxHashSet<Ident>, ignore: &Ident) -> bool {
    match expr {
        Expr::Var(name) => name != ignore && mutated.contains(name),
        Expr::Load { index, .. } => expr_references_any_except(index, mutated, ignore),
        Expr::BinOp { left, right, .. } => {
            expr_references_any_except(left, mutated, ignore)
                || expr_references_any_except(right, mutated, ignore)
        }
        Expr::UnOp { operand, .. } | Expr::Cast { value: operand, .. } => {
            expr_references_any_except(operand, mutated, ignore)
        }
        Expr::Fma { a, b, c } => {
            expr_references_any_except(a, mutated, ignore)
                || expr_references_any_except(b, mutated, ignore)
                || expr_references_any_except(c, mutated, ignore)
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            expr_references_any_except(cond, mutated, ignore)
                || expr_references_any_except(true_val, mutated, ignore)
                || expr_references_any_except(false_val, mutated, ignore)
        }
        Expr::Call { args, .. } => args
            .iter()
            .any(|a| expr_references_any_except(a, mutated, ignore)),
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            expr_references_any_except(index, mutated, ignore)
                || expected
                    .as_deref()
                    .is_some_and(|e| expr_references_any_except(e, mutated, ignore))
                || expr_references_any_except(value, mutated, ignore)
        }
        Expr::SubgroupShuffle { value, .. } | Expr::SubgroupReduce { value, .. } => {
            expr_references_any_except(value, mutated, ignore)
        }
        Expr::SubgroupBallot { cond } => expr_references_any_except(cond, mutated, ignore),
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::Opaque(_) => false,
    }
}

fn expr_references_any(expr: &Expr, mutated: &FxHashSet<Ident>) -> bool {
    match expr {
        Expr::Var(name) => mutated.contains(name),
        Expr::Load { index, .. } => expr_references_any(index, mutated),
        Expr::BinOp { left, right, .. } => {
            expr_references_any(left, mutated) || expr_references_any(right, mutated)
        }
        Expr::UnOp { operand, .. } | Expr::Cast { value: operand, .. } => {
            expr_references_any(operand, mutated)
        }
        Expr::Fma { a, b, c } => {
            expr_references_any(a, mutated)
                || expr_references_any(b, mutated)
                || expr_references_any(c, mutated)
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            expr_references_any(cond, mutated)
                || expr_references_any(true_val, mutated)
                || expr_references_any(false_val, mutated)
        }
        Expr::Call { args, .. } => args.iter().any(|a| expr_references_any(a, mutated)),
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            expr_references_any(index, mutated)
                || expected
                    .as_deref()
                    .is_some_and(|e| expr_references_any(e, mutated))
                || expr_references_any(value, mutated)
        }
        Expr::SubgroupShuffle { value, .. } | Expr::SubgroupReduce { value, .. } => {
            expr_references_any(value, mutated)
        }
        Expr::SubgroupBallot { cond } => expr_references_any(cond, mutated),
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::Opaque(_) => false,
    }
}

/// True iff `expr` evaluates to the same value on every iteration AND
/// produces no observable side effect when evaluated more or fewer
/// times. Loads, Atomics, Calls, Opaque, `BufLen`, and Subgroup ops are
/// rejected  -  relaxed memory ordering or per-invocation lane state
/// could make repeated evaluation observably different from single
/// evaluation when the loop is hoisted to outer scope.
fn expr_is_observably_free(expr: &Expr) -> bool {
    match expr {
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. } => true,
        Expr::BinOp { left, right, .. } => {
            expr_is_observably_free(left) && expr_is_observably_free(right)
        }
        Expr::UnOp { operand, .. } | Expr::Cast { value: operand, .. } => {
            expr_is_observably_free(operand)
        }
        Expr::Fma { a, b, c } => {
            expr_is_observably_free(a) && expr_is_observably_free(b) && expr_is_observably_free(c)
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            expr_is_observably_free(cond)
                && expr_is_observably_free(true_val)
                && expr_is_observably_free(false_val)
        }
        // Anything that could carry a side effect or depend on lane
        // state must stay inside the loop where its execution count
        // is known.
        Expr::Load { .. }
        | Expr::BufLen { .. }
        | Expr::Atomic { .. }
        | Expr::Call { .. }
        | Expr::Opaque(_)
        | Expr::SubgroupShuffle { .. }
        | Expr::SubgroupReduce { .. }
        | Expr::SubgroupBallot { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize => false,
    }
}

/// Cheap matcher used by `analyze`: walks `node` looking for any
/// `Node::Loop` whose body contains at least one Let whose value
/// references neither the loop var nor any mutated-in-body name and
/// is observably free.
fn has_hoistable_let_in_any_loop(node: &Node) -> bool {
    match node {
        Node::Loop { var, body, .. } => {
            let mut mutated: FxHashSet<Ident> = FxHashSet::default();
            mutated.insert(var.clone());
            collect_assigned_and_let_bound_names(body, &mut mutated);
            let mut assigned: FxHashSet<Ident> = FxHashSet::default();
            collect_assigned_names(body, &mut assigned);
            for n in body {
                if let Node::Let { name, value } = n {
                    // Previously: clone `mutated` and `remove(name)` per Let,
                    // which was O(|mutated|) clone per Let just to mask the
                    // current Let's own name. Pass `name` through to the
                    // reference check directly so it can skip the masked id
                    // without rebuilding the set.
                    if !assigned.contains(name)
                        && !expr_references_any_except(value, &mutated, name)
                        && expr_is_observably_free(value)
                    {
                        return true;
                    }
                }
                if has_hoistable_let_in_any_loop(n) {
                    return true;
                }
            }
            false
        }
        Node::If {
            then, otherwise, ..
        } => {
            then.iter().any(has_hoistable_let_in_any_loop)
                || otherwise.iter().any(has_hoistable_let_in_any_loop)
        }
        Node::Block(body) => body.iter().any(has_hoistable_let_in_any_loop),
        Node::Region { body, .. } => body.iter().any(has_hoistable_let_in_any_loop),
        _ => false,
    }
}

#[cfg(test)]

mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node};

    fn buf() -> BufferDecl {
        BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4)
    }

    fn program(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf()], [1, 1, 1], entry)
    }

    fn count_lets(node: &Node) -> usize {
        match node {
            Node::Let { .. } => 1,
            Node::If {
                then, otherwise, ..
            } => {
                then.iter().map(count_lets).sum::<usize>()
                    + otherwise.iter().map(count_lets).sum::<usize>()
            }
            Node::Loop { body, .. } | Node::Block(body) => body.iter().map(count_lets).sum(),
            Node::Region { body, .. } => body.iter().map(count_lets).sum(),
            _ => 0,
        }
    }

    fn count_lets_in_loop_body(entry: &[Node]) -> usize {
        for n in entry {
            if let Node::Loop { body, .. } = n {
                return body.iter().map(count_lets).sum();
            }
        }
        0
    }

    #[test]
    fn hoists_pure_let_above_loop() {
        // Loop body has Let("k", 7)  -  k doesn't depend on the loop var
        // and is observably free. Hoist above the loop.
        let entry = vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(8),
            vec![
                Node::let_bind("k", Expr::u32(7)),
                Node::store("buf", Expr::var("i"), Expr::var("k")),
            ],
        )];
        let result = LoopLicm::transform(program(entry));
        assert!(result.changed);
        let entry = result.program.entry();
        // Outer-scope: hoisted Let, then Loop. Loop body has only the Store.
        assert_eq!(count_lets(&entry[0]), 1, "hoisted Let lives at outer scope");
        assert_eq!(
            count_lets_in_loop_body(entry),
            0,
            "loop body no longer holds the Let"
        );
    }

    #[test]
    fn does_not_hoist_let_that_references_loop_var() {
        let entry = vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(8),
            vec![
                Node::let_bind("idx_plus_one", Expr::add(Expr::var("i"), Expr::u32(1))),
                Node::store("buf", Expr::var("idx_plus_one"), Expr::u32(0)),
            ],
        )];
        let result = LoopLicm::transform(program(entry));
        assert!(
            !result.changed,
            "Let depends on loop var; must stay in loop body"
        );
    }

    #[test]
    fn does_not_hoist_let_that_loads_buffer() {
        // Load is observably non-free in a loop  -  repeated reads under
        // relaxed memory ordering may observe distinct values. Conservative:
        // do not hoist.
        let entry = vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(8),
            vec![
                Node::let_bind("snap", Expr::load("buf", Expr::u32(0))),
                Node::store("buf", Expr::var("i"), Expr::var("snap")),
            ],
        )];
        let result = LoopLicm::transform(program(entry));
        assert!(
            !result.changed,
            "Load must not be hoisted; ordering matters"
        );
    }

    #[test]
    fn does_not_hoist_let_whose_dependency_is_assigned_in_loop() {
        // Let depends on `acc`, which is mutated by an Assign inside
        // the loop. Hoisting `tmp` would freeze it to the pre-loop
        // value of acc.
        let entry = vec![
            Node::let_bind("acc", Expr::u32(0)),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(8),
                vec![
                    Node::let_bind("tmp", Expr::add(Expr::var("acc"), Expr::u32(1))),
                    Node::assign("acc", Expr::var("tmp")),
                ],
            ),
        ];
        let result = LoopLicm::transform(program(entry));
        assert!(
            !result.changed,
            "Let depends on a name Assign'd in the loop; cannot hoist"
        );
    }

    #[test]
    fn does_not_hoist_per_iteration_reset_that_is_assigned_later() {
        let entry = vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(8),
            vec![
                Node::let_bind("emit", Expr::u32(0)),
                Node::if_then(
                    Expr::eq(Expr::var("emit"), Expr::u32(0)),
                    vec![Node::assign("emit", Expr::u32(1))],
                ),
                Node::store("buf", Expr::var("i"), Expr::var("emit")),
            ],
        )];
        let result = LoopLicm::transform(program(entry));
        assert!(
            !result.changed,
            "per-iteration reset locals assigned later must stay inside the loop"
        );
    }

    #[test]
    fn hoists_multiple_independent_pure_lets_in_order() {
        let entry = vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(8),
            vec![
                Node::let_bind("a", Expr::u32(1)),
                Node::let_bind("b", Expr::u32(2)),
                Node::store(
                    "buf",
                    Expr::var("i"),
                    Expr::add(Expr::var("a"), Expr::var("b")),
                ),
            ],
        )];
        let result = LoopLicm::transform(program(entry));
        assert!(result.changed);
        let entry = result.program.entry();
        // Two outer-scope Lets, then a Loop containing only the Store.
        let total_outer_lets: usize = entry.iter().take(2).map(count_lets).sum();
        assert_eq!(total_outer_lets, 2, "both invariant Lets hoisted");
        assert_eq!(count_lets_in_loop_body(entry), 0);
    }

    #[test]
    fn analyze_skips_program_with_no_loops() {
        let entry = vec![Node::let_bind("a", Expr::u32(1))];
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&LoopLicm, &program(entry)),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn analyze_runs_when_loop_has_hoistable_let() {
        let entry = vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(8),
            vec![Node::let_bind("k", Expr::u32(7))],
        )];
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&LoopLicm, &program(entry)),
            PassAnalysis::RUN
        );
    }

    #[test]
    fn nested_loop_hoists_inner_invariant_all_the_way_out() {
        // Outer loop body contains an inner loop; the inner loop has a
        // hoistable Let whose value is invariant across BOTH loops.
        // Bottom-up walk hoists into the inner loop's parent body,
        // then the next pass-iteration hoists again into the outer
        // loop's parent body. `Program::wrapped` always wraps the
        // top-level entry in a single Region so `program.entry()`
        // returns `[Region(body: [...])]`; the hoisted Lets and the
        // surviving Loops live inside that Region body.
        let entry = vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(2),
            vec![Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(4),
                vec![
                    Node::let_bind("k", Expr::u32(7)),
                    Node::store("buf", Expr::var("j"), Expr::var("k")),
                ],
            )],
        )];
        let result = LoopLicm::transform(program(entry));
        assert!(result.changed);
        let entry = result.program.entry();
        // Top-level entry is always a single Region wrapper; descend
        // into its body to find the user nodes.
        assert_eq!(
            entry.len(),
            1,
            "Program::wrapped wraps the entry in a Region"
        );
        let Node::Region {
            body: region_body, ..
        } = &entry[0]
        else {
            panic!("Fix: entry must be the Region wrapper");
        };
        assert!(
            region_body.len() >= 2,
            "Region body holds the hoisted Let and the surviving outer Loop"
        );
        assert!(matches!(&region_body[0], Node::Let { name, .. } if name == "k"));
        let Node::Loop {
            body: outer_body, ..
        } = &region_body[1]
        else {
            panic!("Fix: second Region-body node must be the outer Loop");
        };
        assert_eq!(
            outer_body.len(),
            1,
            "outer Loop body holds only the inner Loop"
        );
        let Node::Loop {
            body: inner_body, ..
        } = &outer_body[0]
        else {
            panic!("Fix: outer Loop body's child must be the inner Loop");
        };
        assert_eq!(inner_body.len(), 1, "inner Loop body holds only the Store");
        assert!(matches!(&inner_body[0], Node::Store { .. }));
    }
}
