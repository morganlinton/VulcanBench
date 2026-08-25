use super::UseFacts;
use crate::ir::{Expr, Ident, Node};
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use std::sync::Arc;

#[derive(Default)]
struct UseFactBuilder {
    var_counts: FxHashMap<Ident, usize>,
    buffer_reads: FxHashMap<Ident, usize>,
    buffer_writes: FxHashMap<Ident, usize>,
    buffer_index_axes: FxHashMap<Ident, [usize; 3]>,
    var_buffer_deps: FxHashMap<Ident, FxHashSet<Ident>>,
    buffer_write_deps: FxHashMap<Ident, FxHashSet<Ident>>,
    indirect_dispatch_buffers: FxHashSet<Ident>,
    has_opaque: bool,
}

impl UseFactBuilder {
    fn finish(self) -> UseFacts {
        UseFacts {
            var_counts: Arc::new(self.var_counts),
            buffer_reads: self.buffer_reads,
            buffer_writes: self.buffer_writes,
            buffer_index_axes: self.buffer_index_axes,
            var_buffer_deps: self.var_buffer_deps,
            buffer_write_deps: self.buffer_write_deps,
            indirect_dispatch_buffers: self.indirect_dispatch_buffers,
            has_opaque: self.has_opaque,
        }
    }
}

pub(super) fn derive_use_facts(program: &crate::ir::Program) -> UseFacts {
    let mut facts = UseFactBuilder::default();
    derive_nodes_uses(program.entry(), &mut facts, &FxHashSet::default());
    facts.finish()
}

fn derive_nodes_uses(nodes: &[Node], facts: &mut UseFactBuilder, control_deps: &FxHashSet<Ident>) {
    for node in nodes {
        match node {
            Node::Let { name, value } | Node::Assign { name, value } => {
                let mut deps = record_expr_uses_and_buffer_deps(value, facts);
                deps.extend(control_deps.iter().cloned());
                facts
                    .var_buffer_deps
                    .entry(name.clone())
                    .or_default()
                    .extend(deps);
            }
            Node::Store {
                buffer,
                index,
                value,
            } => {
                *facts.buffer_writes.entry(buffer.clone()).or_insert(0) += 1;
                let mut deps = FxHashSet::default();
                record_expr_uses_and_buffer_deps_into(&mut deps, index, facts);
                count_index_axes(index, buffer, facts);
                record_expr_uses_and_buffer_deps_into(&mut deps, value, facts);
                deps.extend(control_deps.iter().cloned());
                add_buffer_write_deps(facts, buffer, deps);
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                let cond_deps = record_expr_uses_and_buffer_deps(cond, facts);
                // Fast-path: most predicate expressions are buffer-free
                // comparisons / arithmetic on Vars without recorded buffer
                // deps, so cond_deps is empty. Skip the union allocation
                // and reuse the parent control_deps directly.
                if cond_deps.is_empty() {
                    derive_nodes_uses(then, facts, control_deps);
                    derive_nodes_uses(otherwise, facts, control_deps);
                } else {
                    let branch_control = union_deps(control_deps, &cond_deps);
                    derive_nodes_uses(then, facts, &branch_control);
                    derive_nodes_uses(otherwise, facts, &branch_control);
                }
            }
            Node::Loop { from, to, body, .. } => {
                let mut loop_deps = FxHashSet::default();
                record_expr_uses_and_buffer_deps_into(&mut loop_deps, from, facts);
                record_expr_uses_and_buffer_deps_into(&mut loop_deps, to, facts);
                if loop_deps.is_empty() {
                    derive_nodes_uses(body, facts, control_deps);
                } else {
                    let loop_control = union_deps(control_deps, &loop_deps);
                    derive_nodes_uses(body, facts, &loop_control);
                }
            }
            Node::Block(nodes) => {
                derive_nodes_uses(nodes, facts, control_deps);
            }
            Node::Region { body, .. } => {
                derive_nodes_uses(body, facts, control_deps);
            }
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
                *facts.buffer_reads.entry(source.clone()).or_insert(0) += 1;
                *facts.buffer_writes.entry(destination.clone()).or_insert(0) += 1;
                let mut deps = FxHashSet::default();
                record_expr_uses_and_buffer_deps_into(&mut deps, offset, facts);
                record_expr_uses_and_buffer_deps_into(&mut deps, size, facts);
                deps.extend(control_deps.iter().cloned());
                deps.insert(source.clone());
                add_buffer_write_deps(facts, destination, deps);
            }
            Node::Trap { address, .. } => {
                record_expr_uses_and_buffer_deps(address, facts);
            }
            Node::IndirectDispatch { count_buffer, .. } => {
                facts.indirect_dispatch_buffers.insert(count_buffer.clone());
                *facts.buffer_reads.entry(count_buffer.clone()).or_insert(0) += 1;
            }
            Node::AllReduce { buffer, .. } | Node::Broadcast { buffer, .. } => {
                *facts.buffer_reads.entry(buffer.clone()).or_insert(0) += 1;
                *facts.buffer_writes.entry(buffer.clone()).or_insert(0) += 1;
                let mut deps = FxHashSet::default();
                deps.extend(control_deps.iter().cloned());
                deps.insert(buffer.clone());
                add_buffer_write_deps(facts, buffer, deps);
            }
            Node::AllGather { input, output, .. } | Node::ReduceScatter { input, output, .. } => {
                *facts.buffer_reads.entry(input.clone()).or_insert(0) += 1;
                *facts.buffer_writes.entry(output.clone()).or_insert(0) += 1;
                let mut deps = FxHashSet::default();
                deps.extend(control_deps.iter().cloned());
                deps.insert(input.clone());
                add_buffer_write_deps(facts, output, deps);
            }
            Node::Opaque(_) => {
                facts.has_opaque = true;
            }
            Node::Return | Node::Barrier { .. } | Node::AsyncWait { .. } | Node::Resume { .. } => {}
        }
    }
}

fn record_expr_uses_and_buffer_deps(expr: &Expr, facts: &mut UseFactBuilder) -> FxHashSet<Ident> {
    let mut deps = FxHashSet::default();
    record_expr_uses_and_buffer_deps_into(&mut deps, expr, facts);
    deps
}

/// In-place variant of [`record_expr_uses_and_buffer_deps`]. Use when
/// merging dep sets across two sibling expressions (Store index+value,
/// AsyncLoad/Store offset+size, Loop from+to)  -  avoids the second
/// allocation that the return-value form would otherwise produce on
/// `deps.extend(record_expr_uses_and_buffer_deps(...))`.
fn record_expr_uses_and_buffer_deps_into(
    deps: &mut FxHashSet<Ident>,
    expr: &Expr,
    facts: &mut UseFactBuilder,
) {
    let mut stack: SmallVec<[&Expr; 16]> = SmallVec::new();
    stack.push(expr);
    while let Some(expr) = stack.pop() {
        match expr {
            Expr::Var(name) => {
                *facts.var_counts.entry(name.clone()).or_insert(0) += 1;
                if let Some(var_deps) = facts.var_buffer_deps.get(name) {
                    deps.extend(var_deps.iter().cloned());
                }
            }
            Expr::Load { buffer, index } => {
                *facts.buffer_reads.entry(buffer.clone()).or_insert(0) += 1;
                count_index_axes(index, buffer, facts);
                deps.insert(buffer.clone());
            }
            Expr::BufLen { buffer } => {
                *facts.buffer_reads.entry(buffer.clone()).or_insert(0) += 1;
                deps.insert(buffer.clone());
            }
            Expr::Atomic { buffer, index, .. } => {
                *facts.buffer_reads.entry(buffer.clone()).or_insert(0) += 1;
                *facts.buffer_writes.entry(buffer.clone()).or_insert(0) += 1;
                count_index_axes(index, buffer, facts);
                deps.insert(buffer.clone());
            }
            Expr::Opaque(_) => {
                facts.has_opaque = true;
            }
            _ => {}
        }
        push_expr_children(expr, &mut stack);
    }
}

fn union_deps(a: &FxHashSet<Ident>, b: &FxHashSet<Ident>) -> FxHashSet<Ident> {
    if a.is_empty() {
        return b.clone();
    }
    if b.is_empty() {
        return a.clone();
    }
    let mut out = FxHashSet::default();
    out.reserve(a.len().saturating_add(b.len()));
    out.extend(a.iter().cloned());
    out.extend(b.iter().cloned());
    out
}

fn add_buffer_write_deps(facts: &mut UseFactBuilder, buffer: &Ident, deps: FxHashSet<Ident>) {
    if deps.is_empty() {
        return;
    }
    facts
        .buffer_write_deps
        .entry(buffer.clone())
        .or_default()
        .extend(deps);
}

fn count_index_axes(index: &Expr, buffer: &Ident, facts: &mut UseFactBuilder) {
    let mut stack: SmallVec<[&Expr; 16]> = SmallVec::new();
    stack.push(index);
    while let Some(expr) = stack.pop() {
        if let Expr::InvocationId { axis } | Expr::LocalId { axis } = expr {
            if let Some(slot) = facts
                .buffer_index_axes
                .entry(buffer.clone())
                .or_insert([0; 3])
                .get_mut(usize::from(*axis).min(2))
            {
                *slot += 1;
            }
        }
        push_expr_children(expr, &mut stack);
    }
}

fn push_expr_children<'a>(expr: &'a Expr, stack: &mut SmallVec<[&'a Expr; 16]>) {
    match expr {
        Expr::Load { index, .. } | Expr::UnOp { operand: index, .. } => stack.push(index),
        Expr::BinOp { left, right, .. } => {
            stack.push(left);
            stack.push(right);
        }
        Expr::Call { args, .. } => stack.extend(args),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => {
            stack.push(cond);
            stack.push(true_val);
            stack.push(false_val);
        }
        Expr::Cast { value, .. } | Expr::SubgroupReduce { value, .. } => stack.push(value),
        Expr::Fma { a, b, c } => {
            stack.push(a);
            stack.push(b);
            stack.push(c);
        }
        Expr::Atomic {
            index,
            expected,
            value,
            ..
        } => {
            stack.push(index);
            if let Some(expected) = expected {
                stack.push(expected);
            }
            stack.push(value);
        }
        Expr::SubgroupBallot { cond } => stack.push(cond),
        Expr::SubgroupShuffle { value, lane } => {
            stack.push(value);
            stack.push(lane);
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
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize
        | Expr::Opaque(_) => {}
    }
}
