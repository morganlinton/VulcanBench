use crate::ir::{Expr, Ident, Node, Program};
use crate::optimizer::{fingerprint_program, vyre_pass, PassAnalysis, PassResult};
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use std::sync::Arc;

/// Fuse pure single-use scalar pipelines into their consuming expression.
///
/// The pass must preserve the original program's happens-before ordering.
/// Any replacement that depends on a buffer load is flushed before a write to
/// that same buffer so optimized IR cannot observe a newer value than the
/// unfused sequence would have seen.
#[derive(Debug, Default)]
#[vyre_pass(
    name = "fusion",
    requires = [],
    invalidates = ["region_inline", "canonicalize", "const_fold", "cse", "dce"],
    phase = "fusion_cse",
    boundary_class = "abi_preserving",
    cost_model_family = "fusion"
)]
pub struct Fusion;

impl Fusion {
    /// Decide whether this pass should run.
    #[must_use]
    #[inline]
    fn analyze_impl(program: &Program) -> PassAnalysis {
        // Fusion operates on Regions; without any Region the
        // duplicate-scan walk would always be empty.
        if !program
            .stats()
            .has_any_node_kind(crate::ir::stats::NODE_KIND_REGION)
        {
            return PassAnalysis::SKIP;
        }
        // Iterate the pre-computed region column on ProgramFacts instead
        // of recursing through every node: O(regions) vs O(nodes), and
        // facts is already cached on the program by other passes in the
        // pipeline.
        let facts = crate::optimizer::program_soa::ProgramFacts::build_cached(program);
        let mut counts: FxHashMap<&str, u32> = FxHashMap::default();
        for region in facts.regions() {
            if let Some(base) =
                crate::composition::self_exclusive_region_key(region.generator.as_str())
            {
                let entry = counts.entry(base).or_insert(0);
                *entry += 1;
                if *entry > 1 {
                    return PassAnalysis::SKIP;
                }
            }
        }
        PassAnalysis::RUN
    }

    /// Inline single-use pure bindings so load/op/store pipelines lower as one kernel body.
    #[must_use]
    #[expect(
        clippy::needless_pass_by_value,
        reason = "pass transform consumes Program to preserve the ProgramPass ownership contract"
    )]
    pub fn transform(program: Program) -> PassResult {
        let before_fp = fingerprint_program(&program);
        let fused = fuse_nodes(program.entry(), program.buffers(), &program);
        // Reuse the buffer Arc + buffer_index instead of rebuilding via
        // Program::wrapped (which deep-clones buffers and re-interns names).
        // entry_op_id and non_composable_with_self are already preserved by
        // with_rewritten_entry.
        let optimized = program.with_rewritten_wrapped_entry(fused);
        // VYRE_OPTIMIZER LOW-02: `from_programs` runs full `Program` PartialEq
        // (O(N) structural walk). Content-addressed fingerprint already hashes
        // canonical wire bytes; reuse it for the changed bit.
        let changed = fingerprint_program(&optimized) != before_fp;
        PassResult {
            program: optimized,
            changed,
        }
    }
}

#[cfg(test)]
mod analyze_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn analyze_skips_self_exclusive_duplicate_regions() {
        let generator = crate::composition::mark_self_exclusive_region(
            "vyre-libs::parsing::core_delimiter_match",
        );
        let program = Program::wrapped(
            Vec::new(),
            [1, 1, 1],
            vec![
                Node::Region {
                    generator: generator.clone().into(),
                    source_region: None,
                    body: Arc::new(vec![Node::Return]),
                },
                Node::Region {
                    generator: generator.into(),
                    source_region: None,
                    body: Arc::new(vec![Node::Return]),
                },
            ],
        );
        assert_eq!(
            crate::optimizer::ProgramPass::analyze(&Fusion, &program),
            PassAnalysis::SKIP
        );
    }
}

#[derive(Clone, Debug, Default)]
struct ExprDeps {
    // PERF: uses Ident (Arc<str>) instead of String.
    // Each .clone() is an atomic refcount bump (~1ns) vs
    // heap allocation + memcpy (~30-80ns per String).
    vars: FxHashSet<Ident>,
    buffers: FxHashSet<Ident>,
}

#[derive(Clone, Debug)]
struct PendingExpr {
    expr: Expr,
    deps: ExprDeps,
    sequence: usize,
}

#[derive(Debug, Default)]
struct PendingReplacements {
    entries: FxHashMap<Ident, PendingExpr>,
    order: Vec<Ident>,
    deps_by_var: FxHashMap<Ident, FxHashSet<Ident>>,
    deps_by_buffer: FxHashMap<Ident, FxHashSet<Ident>>,
    next_sequence: usize,
}

impl PendingReplacements {
    fn get(&self, name: &Ident) -> Option<&PendingExpr> {
        self.entries.get(name)
    }

    fn insert(&mut self, name: Ident, deps: ExprDeps, expr: Expr) {
        self.remove(&name);
        let sequence = self.next_sequence;
        self.next_sequence += 1;

        for dep in &deps.vars {
            self.deps_by_var
                .entry(dep.clone())
                .or_default()
                .insert(name.clone());
        }
        for dep in &deps.buffers {
            self.deps_by_buffer
                .entry(dep.clone())
                .or_default()
                .insert(name.clone());
        }

        self.order.push(name.clone());
        self.entries.insert(
            name,
            PendingExpr {
                expr,
                deps,
                sequence,
            },
        );
    }

    fn remove(&mut self, name: &Ident) -> Option<PendingExpr> {
        let pending = self.entries.remove(name)?;
        for dep in &pending.deps.vars {
            let remove_dep = if let Some(names) = self.deps_by_var.get_mut(dep) {
                names.remove(name);
                names.is_empty()
            } else {
                false
            };
            if remove_dep {
                self.deps_by_var.remove(dep);
            }
        }
        for dep in &pending.deps.buffers {
            let remove_dep = if let Some(names) = self.deps_by_buffer.get_mut(dep) {
                names.remove(name);
                names.is_empty()
            } else {
                false
            };
            if remove_dep {
                self.deps_by_buffer.remove(dep);
            }
        }
        Some(pending)
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.deps_by_var.clear();
        self.deps_by_buffer.clear();
    }

    fn flush_all(&mut self, fused: &mut Vec<Node>) {
        for name in std::mem::take(&mut self.order) {
            if let Some(pending) = self.remove(&name) {
                fused.push(Node::let_bind(name, pending.expr));
            }
        }
        self.clear();
    }

    fn drop_used(&mut self, used: &FxHashSet<Ident>) {
        for name in used {
            self.remove(name);
        }
    }

    fn flush_for_var(&mut self, name: &Ident, fused: &mut Vec<Node>) {
        let mut names: SmallVec<[Ident; 8]> = self
            .deps_by_var
            .get(name)
            .map(|deps| deps.iter().cloned().collect())
            .unwrap_or_default();
        names.push(name.clone());
        self.flush_selected_names(names, fused);
    }

    fn flush_for_buffer(&mut self, buffer: &Ident, fused: &mut Vec<Node>) {
        let names: SmallVec<[Ident; 8]> = self
            .deps_by_buffer
            .get(buffer)
            .map(|deps| deps.iter().cloned().collect())
            .unwrap_or_default();
        self.flush_selected_names(names, fused);
    }

    fn flush_selected_names(&mut self, names: SmallVec<[Ident; 8]>, fused: &mut Vec<Node>) {
        let mut selected = Vec::with_capacity(names.len());
        for name in names {
            if let Some(pending) = self.remove(&name) {
                selected.push((pending.sequence, name, pending.expr));
            }
        }
        selected.sort_unstable_by_key(|(sequence, _, _)| *sequence);
        for (_, name, expr) in selected {
            fused.push(Node::let_bind(name, expr));
        }
    }
}

fn fuse_nodes(nodes: &[Node], buffers: &[crate::ir::BufferDecl], program: &Program) -> Vec<Node> {
    let use_counts = cached_var_uses(program);
    fuse_nodes_with_counts(nodes, buffers, &use_counts)
}

#[expect(
    clippy::too_many_lines,
    reason = "fusion state machine keeps pending replacements, flush barriers, and Node reconstruction colocated"
)]
fn fuse_nodes_with_counts(
    nodes: &[Node],
    buffers: &[crate::ir::BufferDecl],
    use_counts: &FxHashMap<Ident, usize>,
) -> Vec<Node> {
    let mut replacements = PendingReplacements::default();
    let mut fused = Vec::with_capacity(nodes.len());
    let mut used_vars = FxHashSet::default();

    for node in nodes {
        if is_control_flow_boundary(node) {
            replacements.flush_all(&mut fused);
            let node_to_push = fuse_control_flow_node(node, buffers, use_counts);

            if let Some(prev) = fused.last_mut() {
                if let Some(combined) = try_fuse_regions(prev, &node_to_push, buffers) {
                    *prev = combined;
                    continue;
                }
            }

            fused.push(node_to_push);
            continue;
        }

        match node {
            Node::Let { name, value }
                // SSA single-use criterion: a binding used exactly once can
                // always be inlined at its use site without code duplication.
                if use_counts.get(name).copied().unwrap_or(0) == 1
                    // Purity gate: only inline expressions without side effects
                    // (no atomics, no opaque calls, no subgroup ops).
                    && is_fusable_expr(value) =>
            {
                used_vars.clear();
                collect_used_vars(value, &mut used_vars);
                let value = substitute_expr(value, &replacements);
                replacements.drop_used(&used_vars);
                replacements.insert(name.clone(), expr_deps(&value), value);
            }
            Node::Let { name, value } => {
                used_vars.clear();
                collect_used_vars(value, &mut used_vars);
                let value = substitute_expr(value, &replacements);
                replacements.drop_used(&used_vars);
                replacements.flush_for_var(name, &mut fused);
                fused.push(Node::let_bind(name, value));
            }
            Node::Assign { name, value } => {
                replacements.flush_for_var(name, &mut fused);
                used_vars.clear();
                collect_used_vars(value, &mut used_vars);
                let value = substitute_expr(value, &replacements);
                replacements.drop_used(&used_vars);
                fused.push(Node::assign(name, value));
            }
            Node::Store {
                buffer,
                index,
                value,
            } => {
                replacements.flush_for_buffer(buffer, &mut fused);
                used_vars.clear();
                collect_used_vars(index, &mut used_vars);
                collect_used_vars(value, &mut used_vars);
                fused.push(Node::store(
                    buffer,
                    substitute_expr(index, &replacements),
                    substitute_expr(value, &replacements),
                ));
                replacements.drop_used(&used_vars);
            }
            Node::Return => {
                replacements.clear();
                fused.push(Node::Return);
            }
            Node::Barrier { ordering } => {
                replacements.flush_all(&mut fused);
                fused.push(Node::barrier_with_ordering(*ordering));
            }
            Node::IndirectDispatch {
                count_buffer,
                count_offset,
            } => {
                replacements.flush_all(&mut fused);
                fused.push(Node::IndirectDispatch {
                    count_buffer: count_buffer.clone(),
                    count_offset: *count_offset,
                });
            }
            Node::AsyncLoad {
                source,
                destination,
                offset,
                size,
                tag,
            } => {
                replacements.flush_all(&mut fused);
                fused.push(Node::async_load_ext(
                    source.clone(),
                    destination.clone(),
                    (**offset).clone(),
                    (**size).clone(),
                    tag.clone(),
                ));
            }
            Node::AsyncStore {
                source,
                destination,
                offset,
                size,
                tag,
            } => {
                replacements.flush_all(&mut fused);
                fused.push(Node::async_store(
                    source.clone(),
                    destination.clone(),
                    (**offset).clone(),
                    (**size).clone(),
                    tag.clone(),
                ));
            }
            Node::AsyncWait { tag } => {
                replacements.flush_all(&mut fused);
                fused.push(Node::async_wait(tag));
            }
            Node::Trap { .. }
            | Node::Resume { .. }
            | Node::AllReduce { .. }
            | Node::AllGather { .. }
            | Node::ReduceScatter { .. }
            | Node::Broadcast { .. }
            | Node::Opaque(_) => {
                replacements.flush_all(&mut fused);
                fused.push(node.clone());
            }
            Node::If { .. } | Node::Loop { .. } | Node::Block(_) | Node::Region { .. } => {
                replacements.flush_all(&mut fused);
                fused.push(fuse_control_flow_node(node, buffers, use_counts));
            }
        }
    }

    replacements.flush_all(&mut fused);
    fused
}

fn cached_var_uses(program: &Program) -> Arc<FxHashMap<Ident, usize>> {
    let substrate =
        crate::optimizer::fact_substrate::FactSubstrate::derive_use_only_cached(program);
    substrate.use_counts.clone().unwrap_or_default()
}

fn fuse_control_flow_node(
    node: &Node,
    buffers: &[crate::ir::BufferDecl],
    use_counts: &FxHashMap<Ident, usize>,
) -> Node {
    match node {
        Node::If {
            cond,
            then,
            otherwise,
        } => Node::if_then_else(
            cond.clone(),
            fuse_nodes_with_counts(then, buffers, use_counts),
            fuse_nodes_with_counts(otherwise, buffers, use_counts),
        ),
        Node::Loop {
            var,
            from,
            to,
            body,
        } => Node::loop_for(
            var,
            from.clone(),
            to.clone(),
            fuse_nodes_with_counts(body, buffers, use_counts),
        ),
        Node::Block(nodes) => Node::block(fuse_nodes_with_counts(nodes, buffers, use_counts)),
        Node::Region {
            generator,
            source_region,
            body,
        } => Node::Region {
            generator: generator.clone(),
            source_region: source_region.clone(),
            body: std::sync::Arc::new(fuse_nodes_with_counts(body, buffers, use_counts)),
        },
        _ => node.clone(),
    }
}

fn is_control_flow_boundary(node: &Node) -> bool {
    matches!(
        node,
        Node::If { .. } | Node::Loop { .. } | Node::Block(_) | Node::Region { .. }
    )
}

fn expr_deps(expr: &Expr) -> ExprDeps {
    let mut deps = ExprDeps::default();
    collect_expr_deps(expr, &mut deps);
    deps
}

fn collect_expr_deps(expr: &Expr, deps: &mut ExprDeps) {
    let mut stack: SmallVec<[&Expr; 16]> = SmallVec::new();
    stack.push(expr);
    while let Some(expr) = stack.pop() {
        match expr {
            Expr::Var(name) => {
                deps.vars.insert(name.clone());
            }
            Expr::Load { buffer, .. } | Expr::BufLen { buffer } | Expr::Atomic { buffer, .. } => {
                deps.buffers.insert(buffer.clone());
                push_expr_children(expr, &mut stack);
            }
            _ => push_expr_children(expr, &mut stack),
        }
    }
}

fn collect_used_vars(expr: &Expr, used: &mut FxHashSet<Ident>) {
    let mut stack: SmallVec<[&Expr; 16]> = SmallVec::new();
    stack.push(expr);
    while let Some(expr) = stack.pop() {
        if let Expr::Var(name) = expr {
            used.insert(name.clone());
        }
        push_expr_children(expr, &mut stack);
    }
}

/// Copy-propagation substitution: replace every pending `Var` with its bound
/// expression, leaving all other expressions -- and the replacement's own body
/// -- verbatim.
///
/// This is exactly the canonical `rewrite_expr` contract (a single-shot,
/// post-order transform whose output is inserted without re-rewriting), so it
/// routes through that driver rather than a hand-rolled match. The driver
/// descends into EVERY subexpression -- subgroup operands (`SubgroupBallot`/
/// `SubgroupShuffle`/`SubgroupReduce`), `Call` args, `Fma`, `Atomic`, ... --
/// which structurally precludes the "forgot the subgroup operand" omission
/// class (the bug a hand-rolled descent had to be patched for): adding an
/// `Expr` variant can never silently skip substitution here again.
fn substitute_expr(expr: &Expr, replacements: &PendingReplacements) -> Expr {
    crate::optimizer::rewrite::rewrite_expr(expr, &mut |e| match e {
        Expr::Var(name) => replacements.get(name).map(|pending| pending.expr.clone()),
        _ => None,
    })
    .into_owned()
}

/// An expression is fusable if it is non-trivial (worth inlining because it
/// saves a `let` binding) and pure (no side effects). Trivial leaf
/// expressions like bare literals or `Var` references are not worth a
/// dedicated `let` binding, so they are excluded.
fn is_fusable_expr(expr: &Expr) -> bool {
    match expr {
        // Non-trivial pure expressions  -  these benefit from inlining.
        Expr::Load { index, .. } => is_pure_expr(index),
        Expr::BinOp { left, right, .. } => is_pure_expr(left) && is_pure_expr(right),
        Expr::UnOp { operand, .. } => is_pure_expr(operand),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => is_pure_expr(cond) && is_pure_expr(true_val) && is_pure_expr(false_val),
        Expr::Cast { value, .. } => is_pure_expr(value),
        Expr::Fma { a, b, c } => is_pure_expr(a) && is_pure_expr(b) && is_pure_expr(c),
        // Side-effectful or opaque  -  never fusable.
        Expr::Call { .. }
        | Expr::Atomic { .. }
        | Expr::Opaque(_)
        | Expr::SubgroupBallot { .. }
        | Expr::SubgroupShuffle { .. }
        | Expr::SubgroupReduce { .. }
        // Trivial leaves  -  not worth a dedicated let binding.
        | Expr::LitU32(_)
        | Expr::LitI32(_)
        | Expr::LitF32(_)
        | Expr::LitBool(_)
        | Expr::Var(_)
        | Expr::BufLen { .. }
        | Expr::InvocationId { .. }
        | Expr::WorkgroupId { .. }
        | Expr::LocalId { .. }
        | Expr::SubgroupLocalId
        | Expr::SubgroupSize => false,
    }
}

/// An expression is pure if it has no observable side effects and will
/// always produce the same value when re-evaluated with the same inputs.
/// Side-effectful ops (atomics, opaque calls, subgroup ops) return false.
fn is_pure_expr(expr: &Expr) -> bool {
    let mut stack: SmallVec<[&Expr; 16]> = SmallVec::new();
    stack.push(expr);
    while let Some(expr) = stack.pop() {
        match expr {
            Expr::Atomic { .. }
            | Expr::Call { .. }
            | Expr::SubgroupBallot { .. }
            | Expr::SubgroupShuffle { .. }
            | Expr::SubgroupReduce { .. }
            | Expr::Opaque(_) => return false,
            _ => push_expr_children(expr, &mut stack),
        }
    }
    true
}

fn try_fuse_regions(r1: &Node, r2: &Node, buffers: &[crate::ir::BufferDecl]) -> Option<Node> {
    if let (
        Node::Region {
            generator: g1,
            source_region: s1,
            body: b1,
        },
        Node::Region {
            generator: g2,
            body: b2,
            ..
        },
    ) = (r1, r2)
    {
        let writes1 = collect_buffer_writes(b1);
        let reads2 = collect_buffer_reads(b2);
        let writes2 = collect_buffer_writes(b2);
        let reads1 = collect_buffer_reads(b1);

        let mut shared = false;
        let mut dim1 = 1u32;
        let mut dim2 = 1u32;

        for buf in buffers {
            let rank = if buf.count() > 0 { buf.count() } else { 1 };
            let buf_ident = Ident::from(buf.name());
            if writes1.contains(&buf_ident) {
                dim1 = dim1.saturating_mul(rank);
                if reads2.contains(&buf_ident) {
                    shared = true;
                }
            }
            if writes2.contains(&buf_ident) {
                dim2 = dim2.saturating_mul(rank);
                if reads1.contains(&buf_ident) {
                    shared = true;
                }
            }
        }

        if !shared {
            return None;
        }

        if dim1.saturating_add(dim2) <= 1024 {
            let mut combined_body = Vec::with_capacity(b1.len() + b2.len());
            combined_body.extend_from_slice(b1.as_ref());
            combined_body.extend_from_slice(b2.as_ref());
            return Some(Node::Region {
                generator: format!("{g1}+{g2}").into(),
                source_region: s1.clone(),
                body: std::sync::Arc::new(combined_body),
            });
        }
    }
    None
}

pub(super) fn collect_buffer_writes(nodes: &[Node]) -> FxHashSet<Ident> {
    let mut writes = FxHashSet::default();
    for node in nodes {
        let _ = crate::visit::node_map::any_descendant(node, &mut |n| {
            match n {
                Node::Store { buffer, .. } => {
                    writes.insert(buffer.clone());
                }
                Node::AsyncLoad { destination, .. } | Node::AsyncStore { destination, .. } => {
                    writes.insert(destination.clone());
                }
                _ => {}
            }
            false
        });
    }
    writes
}

pub(super) fn collect_buffer_reads(nodes: &[Node]) -> FxHashSet<Ident> {
    let mut reads = FxHashSet::default();
    for node in nodes {
        let _ = crate::visit::node_map::any_descendant(node, &mut |n| {
            match n {
                Node::Let { value, .. } | Node::Assign { value, .. } => {
                    collect_expr_buffer_reads(value, &mut reads);
                }
                Node::Store { index, value, .. } => {
                    collect_expr_buffer_reads(index, &mut reads);
                    collect_expr_buffer_reads(value, &mut reads);
                }
                Node::AsyncLoad {
                    source,
                    offset,
                    size,
                    ..
                }
                | Node::AsyncStore {
                    source,
                    offset,
                    size,
                    ..
                } => {
                    reads.insert(source.clone());
                    collect_expr_buffer_reads(offset, &mut reads);
                    collect_expr_buffer_reads(size, &mut reads);
                }
                Node::IndirectDispatch { count_buffer, .. } => {
                    reads.insert(count_buffer.clone());
                }
                Node::Trap { address, .. } => {
                    collect_expr_buffer_reads(address, &mut reads);
                }
                Node::If { cond, .. } => {
                    collect_expr_buffer_reads(cond, &mut reads);
                }
                Node::Loop { from, to, .. } => {
                    collect_expr_buffer_reads(from, &mut reads);
                    collect_expr_buffer_reads(to, &mut reads);
                }
                _ => {}
            }
            false
        });
    }
    reads
}

fn collect_expr_buffer_reads(expr: &Expr, reads: &mut FxHashSet<Ident>) {
    let mut stack: SmallVec<[&Expr; 16]> = SmallVec::new();
    stack.push(expr);
    while let Some(expr) = stack.pop() {
        match expr {
            Expr::Load { buffer, .. } | Expr::BufLen { buffer } | Expr::Atomic { buffer, .. } => {
                reads.insert(buffer.clone());
            }
            _ => {}
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
        Expr::Var(_)
        | Expr::LitU32(_)
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

#[cfg(test)]
mod tests {
    include!("fusion_tests.rs");
}
