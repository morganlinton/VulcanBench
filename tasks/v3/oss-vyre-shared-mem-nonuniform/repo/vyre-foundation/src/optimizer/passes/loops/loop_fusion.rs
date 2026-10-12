//! ROADMAP A26  -  fuse adjacent `Node::Loop` siblings whose bounds
//! match and whose bodies touch disjoint buffer sets.
//!
//! Op id: `vyre-foundation::optimizer::passes::loop_fusion`.
//! Soundness: `Exact` under the conservative buffer-disjointness
//! check. Two loops with identical literal `from..to` ranges, distinct
//! loop variables, and disjoint touched-buffer sets cannot have any
//! cross-loop dependency through memory; fusing them lets the runtime
//! amortise the loop overhead and may unlock further fusion / scratch
//! reuse downstream. Cost direction: monotone-down on `node_count`
//! (one fewer Loop wrapper) and on per-iteration loop overhead.
//! Preserves: every analysis. Invalidates: nothing.
//!
//! ## Rule
//!
//! ```text
//! Node::Loop { var: i, from: LitU32(a), to: LitU32(b), body: [body_a] }
//! Node::Loop { var: j, from: LitU32(a), to: LitU32(b), body: [body_b] }
//!     where buffers_touched(body_a) ∩ buffers_touched(body_b) == ∅
//!     AND body_b uses no name bound inside body_a (other than j itself)
//! →
//! Node::Loop {
//!     var: i,
//!     from: LitU32(a),
//!     to: LitU32(b),
//!     body: [
//!         body_a...,
//!         body_b... (with `j` rewritten to `i`),
//!     ],
//! }
//! ```
//!
//! ## Conservatism
//!
//! - Bounds must be `Expr::LitU32` and structurally equal.
//! - Only adjacent siblings inside the same container body.
//! - Buffer sets must be disjoint  -  any shared buffer would need an
//!   alias / cross-iteration-dependency proof we do not have without
//!   the downstream dataflow analysis.
//! - The second loop's body is rewritten so every `Expr::Var(j)`
//!   becomes `Expr::Var(i)`. A Let in body_a whose name shadows `j`
//!   (or vice versa) blocks the fusion to keep the rewrite local.
//! - The two bodies must not bind a common local name, and body_b must not
//!   bind the fused loop variable `i`: fusion merges both bodies into ONE
//!   scope, so a shared `let` name becomes a duplicate sibling binding (V032)
//!   and a body_b binding of `i` shadows the loop var (V008). The block-scoped
//!   IR pops each loop body's bindings at loop exit, so two sibling loops
//!   binding the same local are legal pre-fusion but collide once merged.
//! - Neither body may `Assign` a scalar the other body reads or assigns: fusion
//!   interleaves the bodies (`A(0); B(0); A(1); B(1); ...`), so a scalar one
//!   loop writes and the other touches is a cross-loop dependency the original
//!   ordering (all of loop_a, then all of loop_b) does not have, and the
//!   interleaving silently changes the observed values. Assign-free bodies (the
//!   common map/transform loops) take the fast path and are unaffected.

use crate::ir::{Expr, Ident, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::node_map;
use rustc_hash::FxHashSet;

/// Fuse adjacent `Node::Loop` siblings under the buffer-disjoint
/// conservatism rule.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "loop_fusion",
    requires = [],
    invalidates = [],
    phase = "loop",
    boundary_class = "abi_preserving",
    cost_model_family = "loop"
)]
/// ABI-preserving loop fusion pass for adjacent loops with compatible iteration spaces.
pub struct LoopFusion;

impl LoopFusion {
    /// Skip when no body has a fusable pair. Checks both the
    /// top-level entry vec (transform fuses adjacent siblings there
    /// too) and every nested If/Loop/Block/Region body.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // Fusion needs at least two adjacent Loops; absent any Loop
        // at all the recursive walk has nothing to find.
        if !program.stats().has_node_loop() {
            return PassAnalysis::SKIP;
        }
        if body_has_fusable_pair(program.entry())
            || program
                .entry()
                .iter()
                .any(|n| node_map::any_descendant(n, &mut has_fusable_pair))
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the program; fuse every fusable adjacent Loop pair found.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| fuse_in_body(entry, &mut changed));
        PassResult { program, changed }
    }
}

fn fuse_in_body(body: Vec<Node>, changed: &mut bool) -> Vec<Node> {
    let body: Vec<Node> = body.into_iter().map(|n| recurse(n, changed)).collect();
    let mut out: Vec<Node> = Vec::with_capacity(body.len());
    let mut iter = body.into_iter().peekable();
    while let Some(node) = iter.next() {
        let Node::Loop {
            var: var_a,
            from: from_a,
            to: to_a,
            body: body_a,
        } = node
        else {
            out.push(node);
            continue;
        };
        let next_is_fusable = matches!(iter.peek(), Some(Node::Loop { .. }));
        if !next_is_fusable {
            out.push(Node::Loop {
                var: var_a,
                from: from_a,
                to: to_a,
                body: body_a,
            });
            continue;
        }
        let Some(Node::Loop {
            var: var_b,
            from: from_b,
            to: to_b,
            body: body_b,
        }) = iter.next()
        else {
            unreachable!("peek confirmed Loop above");
        };
        if !bounds_match(&from_a, &to_a, &from_b, &to_b)
            || var_a == var_b
            || !buffers_disjoint(&body_a, &body_b)
            || body_a_let_names_collide_with_b(&body_a, &body_b, &var_b)
            || fusion_collides_bindings(&body_a, &body_b, &var_a, &var_b)
            || fusion_has_scalar_dependency(&body_a, &body_b)
        {
            // Cannot fuse  -  emit the first loop, push the second back
            // for the next iteration to consider against its successor.
            out.push(Node::Loop {
                var: var_a,
                from: from_a,
                to: to_a,
                body: body_a,
            });
            // We can't actually push back into a Peekable<vec::IntoIter>;
            // emit body_b as-is. Re-fusion across the missed pair will
            // happen on the next pass-scheduler iteration if applicable.
            out.push(Node::Loop {
                var: var_b,
                from: from_b,
                to: to_b,
                body: body_b,
            });
            continue;
        }
        let mut fused = body_a;
        let renamed_body_b: Vec<Node> = body_b
            .into_iter()
            .map(|n| rename_var_in_node(n, &var_b, &var_a))
            .collect();
        fused.extend(renamed_body_b);
        *changed = true;
        out.push(Node::Loop {
            var: var_a,
            from: from_a,
            to: to_a,
            body: fused,
        });
    }
    out
}

fn recurse(node: Node, changed: &mut bool) -> Node {
    let recursed = node_map::map_children(node, &mut |child| recurse(child, changed));
    node_map::map_body(recursed, &mut |body| fuse_in_body(body, changed))
}

fn bounds_match(from_a: &Expr, to_a: &Expr, from_b: &Expr, to_b: &Expr) -> bool {
    matches!(
        (from_a, to_a, from_b, to_b),
        (
            Expr::LitU32(_),
            Expr::LitU32(_),
            Expr::LitU32(_),
            Expr::LitU32(_)
        )
    ) && from_a == from_b
        && to_a == to_b
}

fn buffers_disjoint(body_a: &[Node], body_b: &[Node]) -> bool {
    let mut a_buffers: FxHashSet<Ident> = FxHashSet::default();
    let mut b_buffers: FxHashSet<Ident> = FxHashSet::default();
    collect_touched_buffers(body_a, &mut a_buffers);
    collect_touched_buffers(body_b, &mut b_buffers);
    a_buffers.is_disjoint(&b_buffers)
}

fn collect_touched_buffers(nodes: &[Node], out: &mut FxHashSet<Ident>) {
    for node in nodes {
        match node {
            Node::Store {
                buffer,
                index,
                value,
            } => {
                out.insert(buffer.clone());
                collect_buffers_in_expr(index, out);
                collect_buffers_in_expr(value, out);
            }
            Node::Let { value, .. } | Node::Assign { value, .. } => {
                collect_buffers_in_expr(value, out);
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                collect_buffers_in_expr(cond, out);
                collect_touched_buffers(then, out);
                collect_touched_buffers(otherwise, out);
            }
            Node::Loop { from, to, body, .. } => {
                collect_buffers_in_expr(from, out);
                collect_buffers_in_expr(to, out);
                collect_touched_buffers(body, out);
            }
            Node::Block(body) => collect_touched_buffers(body, out),
            Node::Region { body, .. } => collect_touched_buffers(body, out),
            Node::AsyncLoad {
                source,
                destination,
                offset,
                size,
                ..
            }
            | Node::AsyncStore {
                source,
                destination,
                offset,
                size,
                ..
            } => {
                out.insert(source.clone());
                out.insert(destination.clone());
                collect_buffers_in_expr(offset, out);
                collect_buffers_in_expr(size, out);
            }
            Node::IndirectDispatch { count_buffer, .. } => {
                out.insert(count_buffer.clone());
            }
            Node::Trap { address, .. } => collect_buffers_in_expr(address, out),
            Node::AllReduce { buffer, .. } | Node::Broadcast { buffer, .. } => {
                out.insert(buffer.clone());
            }
            Node::AllGather { input, output, .. } | Node::ReduceScatter { input, output, .. } => {
                out.insert(input.clone());
                out.insert(output.clone());
            }
            Node::Barrier { .. }
            | Node::Return
            | Node::Resume { .. }
            | Node::Opaque(_)
            | Node::AsyncWait { .. } => {}
        }
    }
}

fn collect_buffers_in_expr(expr: &Expr, out: &mut FxHashSet<Ident>) {
    match expr {
        Expr::Load { buffer, index } => {
            out.insert(buffer.clone());
            collect_buffers_in_expr(index, out);
        }
        Expr::BufLen { buffer } => {
            out.insert(buffer.clone());
        }
        Expr::Atomic {
            buffer,
            index,
            expected,
            value,
            ..
        } => {
            out.insert(buffer.clone());
            collect_buffers_in_expr(index, out);
            if let Some(e) = expected.as_deref() {
                collect_buffers_in_expr(e, out);
            }
            collect_buffers_in_expr(value, out);
        }
        Expr::BinOp { left, right, .. } => {
            collect_buffers_in_expr(left, out);
            collect_buffers_in_expr(right, out);
        }
        Expr::UnOp { operand, .. } | Expr::Cast { value: operand, .. } => {
            collect_buffers_in_expr(operand, out);
        }
        Expr::Fma { a, b, c } => {
            collect_buffers_in_expr(a, out);
            collect_buffers_in_expr(b, out);
            collect_buffers_in_expr(c, out);
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            collect_buffers_in_expr(cond, out);
            collect_buffers_in_expr(true_val, out);
            collect_buffers_in_expr(false_val, out);
        }
        Expr::Call { args, .. } => {
            for a in args {
                collect_buffers_in_expr(a, out);
            }
        }
        Expr::SubgroupShuffle { value, .. } | Expr::SubgroupReduce { value, .. } => {
            collect_buffers_in_expr(value, out);
        }
        Expr::SubgroupBallot { cond } => collect_buffers_in_expr(cond, out),
        Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::Opaque(_) => {}
    }
}

fn body_a_let_names_collide_with_b(body_a: &[Node], body_b: &[Node], var_b: &Ident) -> bool {
    // If body_a binds a name that body_b reads (other than var_b),
    // fusing would change resolution. Conservative: refuse to fuse.
    let mut a_lets: FxHashSet<Ident> = FxHashSet::default();
    collect_let_names(body_a, &mut a_lets);
    let mut b_reads: FxHashSet<Ident> = FxHashSet::default();
    collect_var_reads(body_b, &mut b_reads);
    b_reads.remove(var_b);
    !a_lets.is_disjoint(&b_reads)
}

/// True iff fusing the two bodies would introduce a duplicate or shadowing
/// binding the validator rejects. Fusion concatenates the bodies into ONE loop
/// scope (`fused = body_a ++ rename(body_b, var_b -> var_a)`), so a name bound
/// by BOTH bodies becomes a duplicate sibling binding (V032), and a body_b
/// binding of the fused loop variable `var_a` shadows it (V008). The block-scoped
/// IR pops each loop body's bindings at loop exit, so two sibling loops binding
/// the same local are legal pre-fusion but collide once merged.
///
/// `collect_let_names` recurses into nested scopes (and counts `Assign`
/// targets), so this is conservative: it may refuse a fusion whose shared name
/// actually sits in disjoint nested scopes, but it never permits a real
/// collision. This is disjoint from the *capture* hazard guarded by
/// [`body_a_let_names_collide_with_b`] (body_b READING a body_a binding); this
/// is the duplicate-BINDING hazard.
fn fusion_collides_bindings(
    body_a: &[Node],
    body_b: &[Node],
    var_a: &Ident,
    var_b: &Ident,
) -> bool {
    let mut a_lets: FxHashSet<Ident> = FxHashSet::default();
    collect_let_names(body_a, &mut a_lets);
    let mut b_lets: FxHashSet<Ident> = FxHashSet::default();
    collect_let_names(body_b, &mut b_lets);
    b_lets.iter().any(|name| {
        // body_b is rewritten var_b -> var_a before splicing, so a body_b
        // binding of var_b lands as var_a in the fused scope.
        let fused_name = if name == var_b { var_a } else { name };
        // Collides with the fused loop variable (shadow, V008) or with a name
        // body_a already binds (duplicate sibling, V032).
        fused_name == var_a || a_lets.contains(fused_name)
    })
}

/// Collect every `Node::Assign` target name in `nodes` (recursively). These are
/// the scalars a body MUTATES (as opposed to merely binds via `Let` or reads).
fn collect_assign_targets(nodes: &[Node], out: &mut FxHashSet<Ident>) {
    for node in nodes {
        match node {
            Node::Assign { name, .. } => {
                out.insert(name.clone());
            }
            Node::If {
                then, otherwise, ..
            } => {
                collect_assign_targets(then, out);
                collect_assign_targets(otherwise, out);
            }
            Node::Loop { body, .. } | Node::Block(body) => collect_assign_targets(body, out),
            Node::Region { body, .. } => collect_assign_targets(body, out),
            _ => {}
        }
    }
}

/// True iff fusing the two bodies would reorder a cross-loop dependency through
/// a shared mutable scalar. `buffers_disjoint` rules out memory dependencies and
/// `body_a_let_names_collide_with_b` rules out body_b capturing a body_a
/// binding, but NEITHER covers a scalar that one body WRITES (via `Node::Assign`)
/// and the other body reads or writes. The original program runs loop_a entirely
/// before loop_b; fusing interleaves them, so any such scalar dependency changes
/// the observed values (a silent value miscompile, e.g. body_a reading an outer
/// `s` that body_b overwrites). Conservative and name-based (recurses into
/// nested scopes): if either body assigns a name the other body references,
/// refuse. The early return keeps the common assign-free map/transform loops on
/// the fast path.
fn fusion_has_scalar_dependency(body_a: &[Node], body_b: &[Node]) -> bool {
    let mut writes_a: FxHashSet<Ident> = FxHashSet::default();
    collect_assign_targets(body_a, &mut writes_a);
    let mut writes_b: FxHashSet<Ident> = FxHashSet::default();
    collect_assign_targets(body_b, &mut writes_b);
    if writes_a.is_empty() && writes_b.is_empty() {
        return false; // neither body mutates a scalar -> no scalar dependency
    }
    // refs_x = every name body_x reads OR writes.
    let mut refs_a: FxHashSet<Ident> = FxHashSet::default();
    collect_var_reads(body_a, &mut refs_a);
    refs_a.extend(writes_a.iter().cloned());
    let mut refs_b: FxHashSet<Ident> = FxHashSet::default();
    collect_var_reads(body_b, &mut refs_b);
    refs_b.extend(writes_b.iter().cloned());
    // A scalar written by one body and touched by the other is a cross-loop
    // dependency the interleaving would violate.
    !writes_a.is_disjoint(&refs_b) || !writes_b.is_disjoint(&refs_a)
}

fn collect_let_names(nodes: &[Node], out: &mut FxHashSet<Ident>) {
    for node in nodes {
        match node {
            Node::Let { name, .. } | Node::Assign { name, .. } => {
                out.insert(name.clone());
            }
            Node::If {
                then, otherwise, ..
            } => {
                collect_let_names(then, out);
                collect_let_names(otherwise, out);
            }
            Node::Loop { body, .. } | Node::Block(body) => collect_let_names(body, out),
            Node::Region { body, .. } => collect_let_names(body, out),
            _ => {}
        }
    }
}

fn collect_var_reads(nodes: &[Node], out: &mut FxHashSet<Ident>) {
    for node in nodes {
        match node {
            Node::Let { value, .. } | Node::Assign { value, .. } => {
                collect_vars_in_expr(value, out);
            }
            Node::Store { index, value, .. } => {
                collect_vars_in_expr(index, out);
                collect_vars_in_expr(value, out);
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                collect_vars_in_expr(cond, out);
                collect_var_reads(then, out);
                collect_var_reads(otherwise, out);
            }
            Node::Loop { from, to, body, .. } => {
                collect_vars_in_expr(from, out);
                collect_vars_in_expr(to, out);
                collect_var_reads(body, out);
            }
            Node::Block(body) => collect_var_reads(body, out),
            Node::Region { body, .. } => collect_var_reads(body, out),
            _ => {}
        }
    }
}

fn collect_vars_in_expr(expr: &Expr, out: &mut FxHashSet<Ident>) {
    match expr {
        Expr::Var(name) => {
            out.insert(name.clone());
        }
        Expr::Load { index, .. } => collect_vars_in_expr(index, out),
        Expr::BinOp { left, right, .. } => {
            collect_vars_in_expr(left, out);
            collect_vars_in_expr(right, out);
        }
        Expr::UnOp { operand, .. } | Expr::Cast { value: operand, .. } => {
            collect_vars_in_expr(operand, out);
        }
        Expr::Fma { a, b, c } => {
            collect_vars_in_expr(a, out);
            collect_vars_in_expr(b, out);
            collect_vars_in_expr(c, out);
        }
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            collect_vars_in_expr(cond, out);
            collect_vars_in_expr(true_val, out);
            collect_vars_in_expr(false_val, out);
        }
        Expr::Atomic { index, value, .. } => {
            collect_vars_in_expr(index, out);
            collect_vars_in_expr(value, out);
        }
        Expr::Call { args, .. } => {
            for a in args {
                collect_vars_in_expr(a, out);
            }
        }
        Expr::SubgroupShuffle { value, .. } | Expr::SubgroupReduce { value, .. } => {
            collect_vars_in_expr(value, out);
        }
        Expr::SubgroupBallot { cond } => collect_vars_in_expr(cond, out),
        _ => {}
    }
}

fn rename_var_in_node(node: Node, from: &Ident, to: &Ident) -> Node {
    match node {
        Node::Let { name, value } => Node::Let {
            name: if name == *from { to.clone() } else { name },
            value: rename_var_in_expr(value, from, to),
        },
        Node::Assign { name, value } => Node::Assign {
            name: if name == *from { to.clone() } else { name },
            value: rename_var_in_expr(value, from, to),
        },
        Node::Store {
            buffer,
            index,
            value,
        } => Node::Store {
            buffer,
            index: rename_var_in_expr(index, from, to),
            value: rename_var_in_expr(value, from, to),
        },
        Node::If {
            cond,
            then,
            otherwise,
        } => Node::If {
            cond: rename_var_in_expr(cond, from, to),
            then: then
                .into_iter()
                .map(|n| rename_var_in_node(n, from, to))
                .collect(),
            otherwise: otherwise
                .into_iter()
                .map(|n| rename_var_in_node(n, from, to))
                .collect(),
        },
        Node::Loop {
            var,
            from: lo,
            to: hi,
            body,
        } => Node::Loop {
            var,
            from: rename_var_in_expr(lo, from, to),
            to: rename_var_in_expr(hi, from, to),
            body: body
                .into_iter()
                .map(|n| rename_var_in_node(n, from, to))
                .collect(),
        },
        Node::Block(body) => Node::Block(
            body.into_iter()
                .map(|n| rename_var_in_node(n, from, to))
                .collect(),
        ),
        Node::Region {
            generator,
            source_region,
            body,
        } => {
            let body_vec = std::sync::Arc::try_unwrap(body).unwrap_or_else(|arc| (*arc).clone());
            let renamed: Vec<Node> = body_vec
                .into_iter()
                .map(|n| rename_var_in_node(n, from, to))
                .collect();
            Node::Region {
                generator,
                source_region,
                body: std::sync::Arc::new(renamed),
            }
        }
        other => other,
    }
}

fn rename_var_in_expr(expr: Expr, from: &Ident, to: &Ident) -> Expr {
    match expr {
        Expr::Var(name) if name == *from => Expr::Var(to.clone()),
        Expr::Var(name) => Expr::Var(name),
        Expr::Load { buffer, index } => Expr::Load {
            buffer,
            index: Box::new(rename_var_in_expr(*index, from, to)),
        },
        Expr::BinOp { op, left, right } => Expr::BinOp {
            op,
            left: Box::new(rename_var_in_expr(*left, from, to)),
            right: Box::new(rename_var_in_expr(*right, from, to)),
        },
        Expr::UnOp { op, operand } => Expr::UnOp {
            op,
            operand: Box::new(rename_var_in_expr(*operand, from, to)),
        },
        Expr::Cast { target, value } => Expr::Cast {
            target,
            value: Box::new(rename_var_in_expr(*value, from, to)),
        },
        Expr::Fma { a, b, c } => Expr::Fma {
            a: Box::new(rename_var_in_expr(*a, from, to)),
            b: Box::new(rename_var_in_expr(*b, from, to)),
            c: Box::new(rename_var_in_expr(*c, from, to)),
        },
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => Expr::Select {
            cond: Box::new(rename_var_in_expr(*cond, from, to)),
            true_val: Box::new(rename_var_in_expr(*true_val, from, to)),
            false_val: Box::new(rename_var_in_expr(*false_val, from, to)),
        },
        Expr::Call { op_id, args } => Expr::Call {
            op_id,
            args: args
                .into_iter()
                .map(|a| rename_var_in_expr(a, from, to))
                .collect(),
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
            index: Box::new(rename_var_in_expr(*index, from, to)),
            expected: expected.map(|e| Box::new(rename_var_in_expr(*e, from, to))),
            value: Box::new(rename_var_in_expr(*value, from, to)),
            ordering,
        },
        Expr::SubgroupShuffle { value, lane } => Expr::SubgroupShuffle {
            value: Box::new(rename_var_in_expr(*value, from, to)),
            lane: Box::new(rename_var_in_expr(*lane, from, to)),
        },
        Expr::SubgroupReduce { op, value } => Expr::SubgroupReduce { op,
            value: Box::new(rename_var_in_expr(*value, from, to)),
        },
        Expr::SubgroupBallot { cond } => Expr::SubgroupBallot {
            cond: Box::new(rename_var_in_expr(*cond, from, to)),
        },
        other => other,
    }
}

fn has_fusable_pair(node: &Node) -> bool {
    let body: &[Node] = match node {
        Node::If {
            then, otherwise, ..
        } => {
            return body_has_fusable_pair(then) || body_has_fusable_pair(otherwise);
        }
        Node::Loop { body, .. } | Node::Block(body) => body,
        Node::Region { body, .. } => body.as_ref(),
        _ => return false,
    };
    body_has_fusable_pair(body)
}

fn body_has_fusable_pair(body: &[Node]) -> bool {
    for window in body.windows(2) {
        if let (
            Node::Loop {
                var: var_a,
                from: from_a,
                to: to_a,
                body: body_a,
            },
            Node::Loop {
                var: var_b,
                from: from_b,
                to: to_b,
                body: body_b,
            },
        ) = (&window[0], &window[1])
        {
            if bounds_match(from_a, to_a, from_b, to_b)
                && var_a != var_b
                && buffers_disjoint(body_a, body_b)
                && !body_a_let_names_collide_with_b(body_a, body_b, var_b)
                && !fusion_collides_bindings(body_a, body_b, var_a, var_b)
                && !fusion_has_scalar_dependency(body_a, body_b)
            {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node};

    fn buf(name: &str) -> BufferDecl {
        BufferDecl::storage(name, 0, BufferAccess::ReadWrite, DataType::U32).with_count(8)
    }

    fn program(entry: Vec<Node>) -> Program {
        Program::wrapped(vec![buf("a"), buf("b")], [1, 1, 1], entry)
    }

    fn region_body(program_entry: &[Node]) -> Vec<Node> {
        for n in program_entry {
            if let Node::Region { body, .. } = n {
                return body.as_ref().clone();
            }
        }
        program_entry.to_vec()
    }

    fn count_loops(nodes: &[Node]) -> usize {
        nodes
            .iter()
            .map(|n| match n {
                Node::Loop { body, .. } => 1 + count_loops(body),
                Node::If {
                    then, otherwise, ..
                } => count_loops(then) + count_loops(otherwise),
                Node::Block(body) => count_loops(body),
                Node::Region { body, .. } => count_loops(body),
                _ => 0,
            })
            .sum()
    }

    #[test]
    fn fuses_two_disjoint_buffer_loops_with_matching_bounds() {
        let entry = vec![
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(8),
                vec![Node::store("a", Expr::var("i"), Expr::u32(1))],
            ),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(8),
                vec![Node::store("b", Expr::var("j"), Expr::u32(2))],
            ),
        ];
        let result = LoopFusion::transform(program(entry));
        assert!(result.changed);
        assert_eq!(
            count_loops(&region_body(result.program.entry())),
            1,
            "two loops fused into one"
        );
    }

    #[test]
    fn does_not_fuse_when_bounds_differ() {
        let entry = vec![
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(8),
                vec![Node::store("a", Expr::var("i"), Expr::u32(1))],
            ),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(16),
                vec![Node::store("b", Expr::var("j"), Expr::u32(2))],
            ),
        ];
        let result = LoopFusion::transform(program(entry));
        assert!(!result.changed);
    }

    #[test]
    fn does_not_fuse_when_buffers_overlap() {
        let entry = vec![
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(8),
                vec![Node::store("a", Expr::var("i"), Expr::u32(1))],
            ),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(8),
                vec![Node::store("a", Expr::var("j"), Expr::u32(2))],
            ),
        ];
        let result = LoopFusion::transform(program(entry));
        assert!(
            !result.changed,
            "shared buffer blocks fusion under disjoint-only conservatism"
        );
    }

    #[test]
    fn does_not_fuse_when_loop_vars_match() {
        // Two loops with the same var name would shadow each other in
        // the fused body; the rename rule rewrites by var name, and a
        // collision could change resolution. Refuse.
        let entry = vec![
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(8),
                vec![Node::store("a", Expr::var("i"), Expr::u32(1))],
            ),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(8),
                vec![Node::store("b", Expr::var("i"), Expr::u32(2))],
            ),
        ];
        let result = LoopFusion::transform(program(entry));
        assert!(!result.changed, "same loop var name blocks fusion");
    }

    #[test]
    fn renames_second_loop_var_in_fused_body() {
        // Fused body: `Store("a", i, 1); Store("b", i_renamed_from_j, 2)`.
        // The j-Var inside body_b becomes i.
        let entry = vec![
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(8),
                vec![Node::store("a", Expr::var("i"), Expr::u32(1))],
            ),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(8),
                vec![Node::store("b", Expr::var("j"), Expr::u32(2))],
            ),
        ];
        let result = LoopFusion::transform(program(entry));
        assert!(result.changed);
        let body = region_body(result.program.entry());
        let Node::Loop { body: fused, .. } = &body[0] else {
            assert!(false, "Fix: must be a Loop");
            return;
        };
        assert_eq!(fused.len(), 2);
        if let Node::Store { index, .. } = &fused[1] {
            assert_eq!(
                index,
                &Expr::var("i"),
                "second store's index must be renamed to outer var"
            );
        } else {
            assert!(false, "Fix: second fused node must be a Store");
        }
    }

    #[test]
    fn does_not_fuse_when_body_b_reads_a_let_bound_in_body_a() {
        // body_a binds "tmp"; body_b reads "tmp"  -  fusing would
        // change resolution because body_b has no access to body_a's
        // scope across iterations.
        let entry = vec![
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(8),
                vec![
                    Node::let_bind("tmp", Expr::u32(7)),
                    Node::store("a", Expr::var("i"), Expr::var("tmp")),
                ],
            ),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(8),
                vec![Node::store("b", Expr::var("j"), Expr::var("tmp"))],
            ),
        ];
        let result = LoopFusion::transform(program(entry));
        assert!(!result.changed, "shared name `tmp` blocks fusion");
    }

    #[test]
    fn does_not_fuse_when_both_bodies_bind_same_local() {
        // Both bodies bind `x` in their own loop scopes (valid pre-fusion).
        // body_b does NOT read `x`, so the capture guard
        // (body_a_let_names_collide_with_b) passes -- but fusing concatenates
        // the two `let x` into one loop scope, a duplicate sibling binding the
        // validator rejects (V032). Refuse. (Oracle-differential proof:
        // tests/loop_fusion_binding_collision.rs.)
        let entry = vec![
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(8),
                vec![
                    Node::let_bind("x", Expr::u32(1)),
                    Node::store("a", Expr::var("i"), Expr::var("x")),
                ],
            ),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(8),
                // binds `x` but never reads it -> capture guard does not fire
                vec![
                    Node::let_bind("x", Expr::u32(2)),
                    Node::store("b", Expr::var("j"), Expr::u32(9)),
                ],
            ),
        ];
        let result = LoopFusion::transform(program(entry));
        assert!(
            !result.changed,
            "a local name bound by both bodies blocks fusion (duplicate sibling)"
        );
        assert_eq!(
            count_loops(&region_body(result.program.entry())),
            2,
            "both loops must survive unfused"
        );
    }

    #[test]
    fn fuses_when_bodies_bind_distinct_locals() {
        // Distinct local names (`x` vs `y`) cannot collide, so the
        // duplicate-binding guard must NOT block this fusion.
        let entry = vec![
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(8),
                vec![
                    Node::let_bind("x", Expr::u32(1)),
                    Node::store("a", Expr::var("i"), Expr::var("x")),
                ],
            ),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(8),
                vec![
                    Node::let_bind("y", Expr::u32(2)),
                    Node::store("b", Expr::var("j"), Expr::var("y")),
                ],
            ),
        ];
        let result = LoopFusion::transform(program(entry));
        assert!(
            result.changed,
            "distinct local names must still allow fusion"
        );
        assert_eq!(
            count_loops(&region_body(result.program.entry())),
            1,
            "the two loops fuse into one"
        );
    }

    #[test]
    fn does_not_fuse_when_body_b_writes_a_scalar_body_a_reads() {
        // body_a reads outer scalar `s`; body_b writes it via Assign. The
        // original runs loop_a fully (observing s's pre-loop value) before
        // loop_b; fusing interleaves the read and write, changing observed
        // values -- a silent value miscompile. Refuse. (Oracle-differential
        // proof: tests/loop_fusion_scalar_dependency.rs.)
        let entry = vec![
            Node::let_bind("s", Expr::u32(0)),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(8),
                vec![Node::store("a", Expr::var("i"), Expr::var("s"))],
            ),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(8),
                vec![Node::assign("s", Expr::var("j"))],
            ),
        ];
        let result = LoopFusion::transform(program(entry));
        assert!(
            !result.changed,
            "a cross-loop scalar read/write dependency blocks fusion"
        );
        assert_eq!(
            count_loops(&region_body(result.program.entry())),
            2,
            "both loops survive unfused"
        );
    }

    #[test]
    fn fuses_when_bodies_mutate_independent_scalars() {
        // Each body mutates its OWN distinct outer scalar (acc1 vs acc2); there
        // is no cross-loop dependency, so the scalar-dependency guard must NOT
        // block this fusion.
        let entry = vec![
            Node::let_bind("acc1", Expr::u32(0)),
            Node::let_bind("acc2", Expr::u32(0)),
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(8),
                vec![
                    Node::assign("acc1", Expr::add(Expr::var("acc1"), Expr::var("i"))),
                    Node::store("a", Expr::var("i"), Expr::var("acc1")),
                ],
            ),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(8),
                vec![
                    Node::assign("acc2", Expr::add(Expr::var("acc2"), Expr::var("j"))),
                    Node::store("b", Expr::var("j"), Expr::var("acc2")),
                ],
            ),
        ];
        let result = LoopFusion::transform(program(entry));
        assert!(
            result.changed,
            "independent scalar accumulators must still allow fusion"
        );
        assert_eq!(
            count_loops(&region_body(result.program.entry())),
            1,
            "the two loops fuse into one"
        );
    }

    #[test]
    fn analyze_skips_when_no_fusable_pair() {
        let entry = vec![Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(8),
            vec![Node::store("a", Expr::var("i"), Expr::u32(1))],
        )];
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&LoopFusion, &program(entry)),
            PassAnalysis::SKIP
        );
    }

    #[test]
    fn analyze_runs_when_fusable_pair_exists() {
        let entry = vec![
            Node::loop_for(
                "i",
                Expr::u32(0),
                Expr::u32(8),
                vec![Node::store("a", Expr::var("i"), Expr::u32(1))],
            ),
            Node::loop_for(
                "j",
                Expr::u32(0),
                Expr::u32(8),
                vec![Node::store("b", Expr::var("j"), Expr::u32(2))],
            ),
        ];
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&LoopFusion, &program(entry)),
            PassAnalysis::RUN
        );
    }
}
