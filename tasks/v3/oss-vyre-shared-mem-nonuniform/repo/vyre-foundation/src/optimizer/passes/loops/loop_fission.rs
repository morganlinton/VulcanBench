//! ROADMAP A27  -  fission a `Node::Loop` whose body partitions cleanly
//! into two consecutive halves that touch disjoint buffer sets.
//!
//! Op id: `vyre-foundation::optimizer::passes::loop_fission`.
//! Soundness: `Exact` under the conservative buffer-disjoint partition
//! check. If `body = a; b` and `buffers_touched(a) ∩ buffers_touched(b)
//! == ∅` and `b` does not depend on any name bound in `a`, the loop
//! `for i in from..to { a; b }` is observably equivalent to
//! `for i in from..to { a }; for i in from..to { b }` because no
//! cross-iteration or cross-arm dependency exists. Cost direction:
//! `node_count` rises by one Loop wrapper, but the per-arm body
//! becomes vectorizable / tilable / strip-minable in isolation; this
//! is an enabler pass for A29 strip-mine and the SIMD-fan rewrites.
//! Preserves: every analysis. Invalidates: nothing (the loops cover
//! the same iteration space and emit the same observable side
//! effects in the same order).
//!
//! ## Pattern
//!
//! ```text
//! Loop(i, a, b, [s_1, ..., s_k, s_{k+1}, ..., s_n])
//!   where buffers_touched(s_1..s_k) ∩ buffers_touched(s_{k+1}..s_n) == ∅
//!   AND no name bound in s_1..s_k is read by s_{k+1}..s_n
//!   AND no Barrier / IndirectDispatch / AsyncWait sits at the split point
//! → Loop(i, a, b, [s_1, ..., s_k]); Loop(j, a, b, [s_{k+1}, ..., s_n])
//! ```
//!
//! ## Conservatism
//!
//! - `from`/`to` must be `Expr::LitU32` with the same values in both
//!   resulting loops; we copy the original bounds verbatim and freshen
//!   the second loop's induction variable.
//! - The split point is the first index where the prefix and suffix
//!   touch disjoint buffer sets and no name-flow crosses the boundary.
//!   This is a single split  -  multi-way fission falls out by repeated
//!   application of the pass.
//! - Barrier-bearing loops are rejected: a Barrier inside the body
//!   sequences memory across iterations, and splitting it across two
//!   loops changes the observed ordering at the device.
//! - IndirectDispatch / AsyncWait carry queue-level effects whose
//!   relative ordering with the surrounding work cannot be split, so
//!   any presence in the body blocks fission.

use crate::ir::{Expr, Ident, Node, Program};
use crate::optimizer::{vyre_pass, PassAnalysis, PassResult};
use crate::visit::node_map;
use rustc_hash::FxHashSet;

/// Fission a `Node::Loop` with a buffer-disjoint partitionable body
/// into two sibling loops covering the same iteration space.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "loop_fission",
    requires = [],
    invalidates = []
)]
/// ABI-preserving loop fission pass for buffer-disjoint partitionable loop bodies.
pub struct LoopFission;

impl LoopFission {
    /// Skip programs without a fissionable Loop.
    #[must_use]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        if !program.stats().has_node_loop() {
            return PassAnalysis::SKIP;
        }
        if program
            .entry()
            .iter()
            .any(|n| node_map::any_descendant(n, &mut is_fissionable_loop))
        {
            PassAnalysis::RUN
        } else {
            PassAnalysis::SKIP
        }
    }

    /// Walk the entry tree and split fissionable Loops.
    #[must_use]
    pub fn transform(program: Program) -> PassResult {
        let mut changed = false;
        let program = program.map_entry(|entry| fission_in_body(entry, &mut changed));
        PassResult { program, changed }
    }
}

fn fission_in_body(body: Vec<Node>, changed: &mut bool) -> Vec<Node> {
    let body: Vec<Node> = body.into_iter().map(|n| recurse(n, changed)).collect();
    let mut out: Vec<Node> = Vec::with_capacity(body.len());
    for node in body {
        match node {
            Node::Loop {
                var,
                from,
                to,
                body: loop_body,
            } => {
                let bounds_ok = matches!(from, Expr::LitU32(_)) && matches!(to, Expr::LitU32(_));
                if !bounds_ok {
                    out.push(Node::Loop {
                        var,
                        from,
                        to,
                        body: loop_body,
                    });
                    continue;
                }
                if let Some((prefix, suffix)) = try_partition(&loop_body, &var) {
                    *changed = true;
                    let fresh_var = freshen(&var, &loop_body);
                    let renamed_suffix: Vec<Node> = suffix
                        .into_iter()
                        .map(|n| rename_var_in_node(n, &var, &fresh_var))
                        .collect();
                    out.push(Node::Loop {
                        var: var.clone(),
                        from: from.clone(),
                        to: to.clone(),
                        body: prefix,
                    });
                    out.push(Node::Loop {
                        var: fresh_var,
                        from,
                        to,
                        body: renamed_suffix,
                    });
                } else {
                    out.push(Node::Loop {
                        var,
                        from,
                        to,
                        body: loop_body,
                    });
                }
            }
            other => out.push(other),
        }
    }
    out
}

fn recurse(node: Node, changed: &mut bool) -> Node {
    let recursed = node_map::map_children(node, &mut |child| recurse(child, changed));
    node_map::map_body(recursed, &mut |body| fission_in_body(body, changed))
}

/// True iff `nodes` contains an op whose ordering or cross-thread
/// synchronization semantics fission's intra-thread buffer-disjointness
/// analysis cannot safely reorder: a Barrier, `IndirectDispatch`, async op,
/// cross-thread collective, trap, or opaque node. We only check direct
/// siblings because the partition itself only splits direct siblings.
fn has_barrier_like(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| {
        matches!(
            n,
            Node::Barrier { .. }
                | Node::IndirectDispatch { .. }
                | Node::AsyncWait { .. }
                | Node::AsyncLoad { .. }
                | Node::AsyncStore { .. }
                | Node::Trap { .. }
                | Node::Resume { .. }
                | Node::Opaque(_)
                // Cross-thread collectives synchronize a CommGroup; splitting a
                // loop reorders them relative to surrounding code, which the
                // intra-thread buffer-disjointness check cannot prove safe.
                // loop_unroll / loop_strip_mine already block on these.
                | Node::AllReduce { .. }
                | Node::AllGather { .. }
                | Node::ReduceScatter { .. }
                | Node::Broadcast { .. }
        )
    })
}

/// Partition the body into the largest prefix + non-empty suffix
/// whose touched-buffer sets are disjoint AND whose name-flow does
/// not cross the split. Returns `(prefix, suffix)` if such a split
/// exists with both halves non-empty; `None` otherwise.
fn try_partition(body: &[Node], loop_var: &Ident) -> Option<(Vec<Node>, Vec<Node>)> {
    if body.len() < 2 {
        return None;
    }
    if has_barrier_like(body) {
        return None;
    }
    for split in 1..body.len() {
        let prefix = &body[..split];
        let suffix = &body[split..];
        if buffers_disjoint(prefix, suffix) && !suffix_reads_prefix_names(prefix, suffix, loop_var)
        {
            return Some((prefix.to_vec(), suffix.to_vec()));
        }
    }
    None
}

fn buffers_disjoint(a: &[Node], b: &[Node]) -> bool {
    let mut a_buffers: FxHashSet<Ident> = FxHashSet::default();
    let mut b_buffers: FxHashSet<Ident> = FxHashSet::default();
    collect_touched_buffers(a, &mut a_buffers);
    collect_touched_buffers(b, &mut b_buffers);
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
            | Node::IndirectDispatch { .. }
            | Node::AsyncLoad { .. }
            | Node::AsyncStore { .. }
            | Node::AsyncWait { .. }
            | Node::Resume { .. }
            | Node::Opaque(_) => {}
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
        Expr::UnOp { operand, .. } => collect_buffers_in_expr(operand, out),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            collect_buffers_in_expr(cond, out);
            collect_buffers_in_expr(true_val, out);
            collect_buffers_in_expr(false_val, out);
        }
        Expr::Cast { value, .. } | Expr::SubgroupReduce { value, .. } => {
            collect_buffers_in_expr(value, out);
        }
        Expr::Fma { a, b, c } => {
            collect_buffers_in_expr(a, out);
            collect_buffers_in_expr(b, out);
            collect_buffers_in_expr(c, out);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_buffers_in_expr(arg, out);
            }
        }
        Expr::SubgroupBallot { cond } => collect_buffers_in_expr(cond, out),
        Expr::SubgroupShuffle { value, lane } => {
            collect_buffers_in_expr(value, out);
            collect_buffers_in_expr(lane, out);
        }
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

/// True iff any name introduced by `prefix` (via `Let`) is read in
/// `suffix`. The loop induction `loop_var` is excluded  -  both halves
/// see it bound by their own loop header after the split, so the
/// suffix's reference to `loop_var` is not a cross-half name flow.
fn suffix_reads_prefix_names(prefix: &[Node], suffix: &[Node], loop_var: &Ident) -> bool {
    let mut prefix_names: FxHashSet<Ident> = FxHashSet::default();
    collect_let_names(prefix, &mut prefix_names);
    prefix_names.remove(loop_var);
    if prefix_names.is_empty() {
        return false;
    }
    let mut suffix_reads: FxHashSet<Ident> = FxHashSet::default();
    collect_var_reads(suffix, &mut suffix_reads);
    !prefix_names.is_disjoint(&suffix_reads)
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
            Node::Loop { var, body, .. } => {
                out.insert(var.clone());
                collect_let_names(body, out);
            }
            Node::Block(body) => collect_let_names(body, out),
            Node::Region { body, .. } => collect_let_names(body, out),
            _ => {}
        }
    }
}

fn collect_var_reads(nodes: &[Node], out: &mut FxHashSet<Ident>) {
    for node in nodes {
        match node {
            Node::Let { value, .. } | Node::Assign { value, .. } => {
                collect_var_reads_in_expr(value, out);
            }
            Node::Store { index, value, .. } => {
                collect_var_reads_in_expr(index, out);
                collect_var_reads_in_expr(value, out);
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                collect_var_reads_in_expr(cond, out);
                collect_var_reads(then, out);
                collect_var_reads(otherwise, out);
            }
            Node::Loop { from, to, body, .. } => {
                collect_var_reads_in_expr(from, out);
                collect_var_reads_in_expr(to, out);
                collect_var_reads(body, out);
            }
            Node::Block(body) => collect_var_reads(body, out),
            Node::Region { body, .. } => collect_var_reads(body, out),
            _ => {}
        }
    }
}

fn collect_var_reads_in_expr(expr: &Expr, out: &mut FxHashSet<Ident>) {
    match expr {
        Expr::Var(name) => {
            out.insert(name.clone());
        }
        Expr::Load { index, .. } => collect_var_reads_in_expr(index, out),
        Expr::BinOp { left, right, .. } => {
            collect_var_reads_in_expr(left, out);
            collect_var_reads_in_expr(right, out);
        }
        Expr::UnOp { operand, .. } => collect_var_reads_in_expr(operand, out),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            collect_var_reads_in_expr(cond, out);
            collect_var_reads_in_expr(true_val, out);
            collect_var_reads_in_expr(false_val, out);
        }
        Expr::Cast { value, .. } | Expr::SubgroupReduce { value, .. } => {
            collect_var_reads_in_expr(value, out);
        }
        Expr::Fma { a, b, c } => {
            collect_var_reads_in_expr(a, out);
            collect_var_reads_in_expr(b, out);
            collect_var_reads_in_expr(c, out);
        }
        Expr::Call { args, .. } => {
            for arg in args {
                collect_var_reads_in_expr(arg, out);
            }
        }
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            collect_var_reads_in_expr(index, out);
            if let Some(e) = expected.as_deref() {
                collect_var_reads_in_expr(e, out);
            }
            collect_var_reads_in_expr(value, out);
        }
        Expr::SubgroupBallot { cond } => collect_var_reads_in_expr(cond, out),
        Expr::SubgroupShuffle { value, lane } => {
            collect_var_reads_in_expr(value, out);
            collect_var_reads_in_expr(lane, out);
        }
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
        | Expr::Opaque(_) => {}
    }
}

fn rename_var_in_node(node: Node, from: &Ident, to: &Ident) -> Node {
    match node {
        Node::Let { name, value } => Node::Let {
            name,
            value: rename_var_in_expr(value, from, to),
        },
        Node::Assign { name, value } => Node::Assign {
            name,
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
            let body_vec: Vec<Node> = match std::sync::Arc::try_unwrap(body) {
                Ok(v) => v,
                Err(arc) => (*arc).clone(),
            };
            Node::Region {
                generator,
                source_region,
                body: std::sync::Arc::new(
                    body_vec
                        .into_iter()
                        .map(|n| rename_var_in_node(n, from, to))
                        .collect(),
                ),
            }
        }
        other => other,
    }
}

fn rename_var_in_expr(expr: Expr, from: &Ident, to: &Ident) -> Expr {
    match expr {
        Expr::Var(name) if name.as_str() == from.as_str() => Expr::Var(to.clone()),
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
        Expr::Call { op_id, args } => Expr::Call {
            op_id,
            args: args
                .into_iter()
                .map(|a| rename_var_in_expr(a, from, to))
                .collect(),
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
        Expr::Cast { target, value } => Expr::Cast {
            target,
            value: Box::new(rename_var_in_expr(*value, from, to)),
        },
        Expr::Fma { a, b, c } => Expr::Fma {
            a: Box::new(rename_var_in_expr(*a, from, to)),
            b: Box::new(rename_var_in_expr(*b, from, to)),
            c: Box::new(rename_var_in_expr(*c, from, to)),
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
        Expr::SubgroupBallot { cond } => Expr::SubgroupBallot {
            cond: Box::new(rename_var_in_expr(*cond, from, to)),
        },
        Expr::SubgroupShuffle { value, lane } => Expr::SubgroupShuffle {
            value: Box::new(rename_var_in_expr(*value, from, to)),
            lane: Box::new(rename_var_in_expr(*lane, from, to)),
        },
        Expr::SubgroupReduce { op, value } => Expr::SubgroupReduce { op,
            value: Box::new(rename_var_in_expr(*value, from, to)),
        },
        other => other,
    }
}

/// Pick a name not used as a Let/Assign/Loop var anywhere in `body`.
fn freshen(base: &Ident, body: &[Node]) -> Ident {
    let mut used: FxHashSet<Ident> = FxHashSet::default();
    collect_let_names(body, &mut used);
    used.insert(base.clone());
    let mut counter = 0u32;
    loop {
        let candidate = Ident::from(format!("{}__fis_{counter}", base.as_str()));
        if !used.contains(&candidate) {
            return candidate;
        }
        counter += 1;
    }
}

fn is_fissionable_loop(node: &Node) -> bool {
    if let Node::Loop {
        var,
        body,
        from,
        to,
    } = node
    {
        if !matches!(from, Expr::LitU32(_)) || !matches!(to, Expr::LitU32(_)) {
            return false;
        }
        try_partition(body, var).is_some()
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node};

    fn buf(name: &str) -> BufferDecl {
        BufferDecl::storage(name, 0, BufferAccess::ReadWrite, DataType::U32).with_count(8)
    }

    fn program_with_entry(buffers: Vec<BufferDecl>, entry: Vec<Node>) -> Program {
        Program::wrapped(buffers, [1, 1, 1], entry)
    }

    fn count_loops(nodes: &[Node]) -> usize {
        let mut total = 0;
        for n in nodes {
            match n {
                Node::Loop { body, .. } => {
                    total += 1;
                    total += count_loops(body);
                }
                Node::If {
                    then, otherwise, ..
                } => {
                    total += count_loops(then);
                    total += count_loops(otherwise);
                }
                Node::Block(body) => total += count_loops(body),
                Node::Region { body, .. } => total += count_loops(body),
                _ => {}
            }
        }
        total
    }

    /// Positive: a loop body that writes two distinct buffers fissions
    /// into two sibling loops with the same iteration space.
    #[test]
    fn fissions_two_disjoint_stores() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(8),
            body: vec![
                Node::store("a", Expr::var("i"), Expr::u32(1)),
                Node::store("b", Expr::var("i"), Expr::u32(2)),
            ],
        }];
        let program = program_with_entry(vec![buf("a"), buf("b")], entry);
        let result = LoopFission::transform(program);
        assert!(result.changed, "two-buffer-disjoint Loop must fission");
        assert_eq!(
            count_loops(result.program.entry()),
            2,
            "exactly two sibling loops after fission"
        );
    }

    /// Negative: shared buffer between halves blocks the fission.
    #[test]
    fn keeps_when_halves_share_a_buffer() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(8),
            body: vec![
                Node::store("a", Expr::var("i"), Expr::u32(1)),
                Node::store("a", Expr::var("i"), Expr::u32(2)),
            ],
        }];
        let program = program_with_entry(vec![buf("a")], entry);
        let result = LoopFission::transform(program);
        assert!(
            !result.changed,
            "shared buffer must block fission  -  alias proof unavailable"
        );
        assert_eq!(count_loops(result.program.entry()), 1);
    }

    /// Negative: a name flow from prefix to suffix blocks the fission.
    #[test]
    fn keeps_when_suffix_reads_prefix_let_name() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(8),
            body: vec![
                Node::let_bind("v", Expr::u32(7)),
                Node::store("a", Expr::var("i"), Expr::var("v")),
                Node::store("b", Expr::var("i"), Expr::var("v")),
            ],
        }];
        let program = program_with_entry(vec![buf("a"), buf("b")], entry);
        let result = LoopFission::transform(program);
        assert!(
            !result.changed,
            "name flow across split point must block fission"
        );
        assert_eq!(count_loops(result.program.entry()), 1);
    }

    /// Negative: a Barrier inside the loop body sequences memory across
    /// iterations and must not be split.
    #[test]
    fn keeps_when_body_contains_barrier() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(8),
            body: vec![
                Node::store("a", Expr::var("i"), Expr::u32(1)),
                Node::barrier_with_ordering(crate::ir::MemoryOrdering::Relaxed),
                Node::store("b", Expr::var("i"), Expr::u32(2)),
            ],
        }];
        let program = program_with_entry(vec![buf("a"), buf("b")], entry);
        let result = LoopFission::transform(program);
        assert!(!result.changed, "Barrier must block fission");
        assert_eq!(count_loops(result.program.entry()), 1);
    }

    /// Negative: a cross-thread collective (AllReduce) in the loop body
    /// synchronizes a CommGroup across threads. Fission's intra-thread
    /// buffer-disjointness check cannot prove that reordering the collective
    /// into a separate loop preserves cross-thread semantics, so it must block
    /// the split — exactly as a Barrier does, and as loop_unroll /
    /// loop_strip_mine already do. (Before the fix, the disjoint `a`/`b`
    /// buffers let fission split this loop, reordering the collective.)
    #[test]
    fn keeps_when_body_contains_collective() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(8),
            body: vec![
                Node::AllReduce {
                    buffer: Ident::from("a"),
                    op: crate::ir::CollectiveOp::Sum,
                    group: crate::ir::CommGroup::WORLD,
                },
                Node::store("b", Expr::var("i"), Expr::u32(2)),
            ],
        }];
        let program = program_with_entry(vec![buf("a"), buf("b")], entry);
        let result = LoopFission::transform(program);
        assert!(
            !result.changed,
            "a cross-thread collective must block fission like a barrier"
        );
        assert_eq!(count_loops(result.program.entry()), 1);
    }

    /// Negative: a single-statement body cannot be fissioned (needs at
    /// least two siblings to split).
    #[test]
    fn keeps_when_body_has_one_statement() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(8),
            body: vec![Node::store("a", Expr::var("i"), Expr::u32(1))],
        }];
        let program = program_with_entry(vec![buf("a")], entry);
        let result = LoopFission::transform(program);
        assert!(!result.changed);
        assert_eq!(count_loops(result.program.entry()), 1);
    }

    /// Negative: runtime upper bound rejects the fission gate (we keep
    /// the bounds-must-be-literal contract symmetrical with A26 fusion
    /// and A29 strip-mine).
    #[test]
    fn keeps_when_upper_bound_is_runtime() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::var("n"),
            body: vec![
                Node::store("a", Expr::var("i"), Expr::u32(1)),
                Node::store("b", Expr::var("i"), Expr::u32(2)),
            ],
        }];
        let program = program_with_entry(vec![buf("a"), buf("b")], entry);
        let result = LoopFission::transform(program);
        assert!(!result.changed);
    }

    /// Positive: a three-arm body (`a`, `b`, `c` writing distinct
    /// buffers) fissions in repeated applications. One pass picks the
    /// earliest cleavable split  -  here the prefix `[a]` versus suffix
    /// `[b; c]`. The resulting `[b; c]` body remains fissionable for a
    /// second pass invocation.
    #[test]
    fn fissions_at_first_cleavable_split() {
        let entry = vec![Node::Loop {
            var: Ident::from("i"),
            from: Expr::u32(0),
            to: Expr::u32(8),
            body: vec![
                Node::store("a", Expr::var("i"), Expr::u32(1)),
                Node::store("b", Expr::var("i"), Expr::u32(2)),
                Node::store("c", Expr::var("i"), Expr::u32(3)),
            ],
        }];
        let program = program_with_entry(vec![buf("a"), buf("b"), buf("c")], entry);
        let first = LoopFission::transform(program);
        assert!(first.changed, "first pass must fission earliest split");
        assert_eq!(
            count_loops(first.program.entry()),
            2,
            "after one fission, two sibling loops exist"
        );
        let second = LoopFission::transform(first.program);
        assert!(
            second.changed,
            "second pass must fission the remaining two-arm loop"
        );
        assert_eq!(
            count_loops(second.program.entry()),
            3,
            "after second fission, three sibling loops exist"
        );
    }

    /// `analyze` short-circuits when no Loop is fissionable.
    #[test]
    fn analyze_skips_program_with_no_loops() {
        let entry = vec![Node::store("a", Expr::u32(0), Expr::u32(1))];
        let program = program_with_entry(vec![buf("a")], entry);
        assert!(matches!(
            crate::optimizer::ProgramPass::analyze(&LoopFission, &program),
            PassAnalysis::SKIP
        ));
    }
}
