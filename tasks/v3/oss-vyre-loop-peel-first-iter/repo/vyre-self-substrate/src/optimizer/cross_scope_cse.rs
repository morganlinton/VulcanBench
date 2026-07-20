//! Cross-scope expression CSE  -  hoists common subexpressions to
//! shared `let` bindings.
//!
//! Complements `apply_cse_let_dedupe` (which dedupes only `let`-RHS
//! pairs in the same scope) by handling Exprs that appear directly
//! as the value of `Node::Store` / `Node::Assign` / `Node::If` cond
//! / `Node::Loop` bounds. When the same canonical-equivalent Expr
//! appears at 2+ such top-level positions in a single scope, we
//! introduce a fresh `let __cse_<n> = E;` at the scope's start and
//! rewrite each occurrence to `Var(__cse_<n>)`.
//!
//! Walker uses the arena's `node_top_level_exprs` to identify per-
//! Node arena ids  -  robust to upstream rewrites that change inner
//! Expr shape, since Node-level structure is preserved.

use std::sync::Arc;

use rustc_hash::FxHashMap;
use vyre_foundation::ir::{Expr, Ident, Node, Program};

use super::cse_via_encoded::CanonicalLookup;
use super::expr_arena::ExprArenaEncoding;

/// Apply same-scope expression CSE. For each scope, identifies
/// non-trivial top-level Exprs whose canonical id is shared and
/// hoists them to a single `let __cse_<n> = E;` at the scope's
/// start.
pub fn apply_cross_scope_cse(
    program: &Program,
    arena: &ExprArenaEncoding,
    canonical: &[u32],
) -> Program {
    apply_cross_scope_cse_with_lookup(program, arena, canonical)
}

/// Sparse/dense-agnostic variant of [`apply_cross_scope_cse`].
pub fn apply_cross_scope_cse_with_lookup<C: CanonicalLookup + ?Sized>(
    program: &Program,
    arena: &ExprArenaEncoding,
    canonical: &C,
) -> Program {
    if canonical.is_empty() || arena.expr_count == 0 {
        return program.clone();
    }
    let body: Vec<Node> = match program.entry() {
        [Node::Region { body, .. }] => body.as_ref().clone(),
        entry => entry.to_vec(),
    };
    let mut walker = CseWalker {
        arena,
        canonical,
        node_index: 1, // index 0 = synthetic ROOT (no Exprs)
        next_let_id: 0,
    };
    let new_body = walker.rewrite_scope(&body);
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

struct CseWalker<'a, C: CanonicalLookup + ?Sized> {
    arena: &'a ExprArenaEncoding,
    canonical: &'a C,
    /// Mirrors the arena's `node_top_level_exprs` index  -  increments
    /// by one per Node visited in DFS prefix order.
    node_index: usize,
    /// Monotonic suffix for fresh `__cse_N` names.
    next_let_id: u32,
}

/// One top-level Expr position within a single scope's direct
/// Nodes. The canonical id and a clone of the actual Expr.
struct Occurrence {
    canon: u32,
    expr: Expr,
}

impl<C: CanonicalLookup + ?Sized> CseWalker<'_, C> {
    fn rewrite_scope(&mut self, body: &[Node]) -> Vec<Node> {
        let prefix_len = super::encode::reachable_prefix_len(body);

        // ---- Pass 1: collect occurrences for THIS scope's direct
        // Nodes only. Recurse into nested scopes purely to advance
        // node_index past their Nodes. -------------------------
        let saved_index = self.node_index;
        let mut occurrences: Vec<Occurrence> = Vec::new();
        for node in &body[..prefix_len] {
            self.collect_node(node, &mut occurrences);
        }

        // Identify hoist-worthy canonicals (count >= 2, non-trivial,
        // atomic-free).
        let mut counts: FxHashMap<u32, (u32, Expr)> = FxHashMap::default();
        let mut order: Vec<u32> = Vec::new();
        for occ in occurrences {
            counts
                .entry(occ.canon)
                .and_modify(|(c, _)| *c += 1)
                .or_insert_with(|| {
                    order.push(occ.canon);
                    (1, occ.expr)
                });
        }
        let mut hoist_plan: FxHashMap<u32, Ident> = FxHashMap::default();
        let mut hoist_lets: Vec<Node> = Vec::new();
        for canon in &order {
            let (count, expr) = match counts.get(canon).cloned() {
                Some(p) => p,
                None => continue,
            };
            if count < 2 {
                continue;
            }
            if !is_hoist_worthy(&expr) {
                continue;
            }
            if !expr_no_atomic(&expr) {
                continue;
            }
            let name = self.fresh_name();
            hoist_lets.push(Node::let_bind(name.clone(), expr));
            hoist_plan.insert(*canon, name);
        }

        // ---- Pass 2: rewrite, substituting hoisted Vars ----------
        // Restore node_index to the start of this scope so pass 2
        // sees identical mappings.
        self.node_index = saved_index;
        let mut out: Vec<Node> = Vec::with_capacity(prefix_len + hoist_lets.len());
        out.extend(hoist_lets);
        for node in &body[..prefix_len] {
            out.push(self.rewrite_node(node, &hoist_plan));
        }
        out
    }

    /// Pass 1: record this Node's top-level Expr canonicals/clones in
    /// `occs`, then advance `node_index` past nested scopes without
    /// recording.
    fn collect_node(&mut self, node: &Node, occs: &mut Vec<Occurrence>) {
        let idx = self.node_index;
        self.node_index += 1;
        let top_ids = self
            .arena
            .node_top_level_exprs
            .get(idx)
            .cloned()
            .unwrap_or_default();
        match node {
            Node::Let { value, .. } | Node::Assign { value, .. } => {
                self.record(&top_ids, 0, value, occs);
            }
            Node::Store { index, value, .. } => {
                self.record(&top_ids, 0, index, occs);
                self.record(&top_ids, 1, value, occs);
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                self.record(&top_ids, 0, cond, occs);
                self.advance_through_scope(then);
                self.advance_through_scope(otherwise);
            }
            Node::Loop { from, to, body, .. } => {
                self.record(&top_ids, 0, from, occs);
                self.record(&top_ids, 1, to, occs);
                self.advance_through_scope(body);
            }
            Node::Block(body) => self.advance_through_scope(body),
            Node::Region { body, .. } => self.advance_through_scope(body.as_slice()),
            Node::AsyncLoad { offset, size, .. } | Node::AsyncStore { offset, size, .. } => {
                self.record(&top_ids, 0, offset, occs);
                self.record(&top_ids, 1, size, occs);
            }
            Node::Trap { address, .. } => self.record(&top_ids, 0, address, occs),
            _ => {}
        }
    }

    fn record(&self, top_ids: &[u32], slot: usize, expr: &Expr, occs: &mut Vec<Occurrence>) {
        let arena_id = match top_ids.get(slot).copied() {
            Some(id) => id,
            None => return,
        };
        let canon = self.canonical.canonical_of(arena_id);
        occs.push(Occurrence {
            canon,
            expr: expr.clone(),
        });
    }

    fn advance_through_scope(&mut self, body: &[Node]) {
        let prefix_len = super::encode::reachable_prefix_len(body);
        for node in &body[..prefix_len] {
            self.advance_through_node(node);
        }
    }

    fn advance_through_node(&mut self, node: &Node) {
        self.node_index += 1;
        match node {
            Node::If {
                then, otherwise, ..
            } => {
                self.advance_through_scope(then);
                self.advance_through_scope(otherwise);
            }
            Node::Loop { body, .. } => self.advance_through_scope(body),
            Node::Block(body) => self.advance_through_scope(body),
            Node::Region { body, .. } => self.advance_through_scope(body.as_slice()),
            _ => {}
        }
    }

    fn fresh_name(&mut self) -> Ident {
        let id = self.next_let_id;
        self.next_let_id += 1;
        Ident::new(Arc::from(format!("__cse_{id}")))
    }

    fn rewrite_node(&mut self, node: &Node, plan: &FxHashMap<u32, Ident>) -> Node {
        let idx = self.node_index;
        self.node_index += 1;
        let top_ids: Vec<u32> = self
            .arena
            .node_top_level_exprs
            .get(idx)
            .cloned()
            .unwrap_or_default();
        match node {
            Node::Let { name, value } => {
                let new_value = self.rewrite_top(&top_ids, 0, value, plan);
                Node::let_bind(name.clone(), new_value)
            }
            Node::Assign { name, value } => {
                let new_value = self.rewrite_top(&top_ids, 0, value, plan);
                Node::assign(name.clone(), new_value)
            }
            Node::Store {
                buffer,
                index,
                value,
            } => {
                let new_index = self.rewrite_top(&top_ids, 0, index, plan);
                let new_value = self.rewrite_top(&top_ids, 1, value, plan);
                Node::store(buffer.clone(), new_index, new_value)
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                let new_cond = self.rewrite_top(&top_ids, 0, cond, plan);
                let new_then = self.rewrite_scope(then);
                let new_otherwise = self.rewrite_scope(otherwise);
                Node::if_then_else(new_cond, new_then, new_otherwise)
            }
            Node::Loop {
                var,
                from,
                to,
                body,
            } => {
                let new_from = self.rewrite_top(&top_ids, 0, from, plan);
                let new_to = self.rewrite_top(&top_ids, 1, to, plan);
                let new_body = self.rewrite_scope(body);
                Node::loop_for(var.clone(), new_from, new_to, new_body)
            }
            Node::Block(body) => Node::Block(self.rewrite_scope(body)),
            Node::Region {
                generator,
                source_region,
                body,
            } => Node::Region {
                generator: generator.clone(),
                source_region: source_region.clone(),
                body: Arc::new(self.rewrite_scope(body.as_slice())),
            },
            other => other.clone(),
        }
    }

    fn rewrite_top(
        &self,
        top_ids: &[u32],
        slot: usize,
        expr: &Expr,
        plan: &FxHashMap<u32, Ident>,
    ) -> Expr {
        let arena_id = match top_ids.get(slot).copied() {
            Some(id) => id,
            None => return expr.clone(),
        };
        let canon = self.canonical.canonical_of(arena_id);
        if let Some(name) = plan.get(&canon) {
            return Expr::var(name.clone());
        }
        expr.clone()
    }
}

/// Decide if an Expr is worth hoisting. Skip leaves  -  duplicating
/// those is cheaper than an extra Var indirection.
fn is_hoist_worthy(expr: &Expr) -> bool {
    !matches!(
        expr,
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
    )
}

/// True iff `expr` contains no `Expr::Atomic` anywhere  -  hoisting
/// past an atomic op would change observable order.
fn expr_no_atomic(expr: &Expr) -> bool {
    match expr {
        Expr::Atomic { .. } => false,
        Expr::BinOp { left, right, .. } => expr_no_atomic(left) && expr_no_atomic(right),
        Expr::UnOp { operand, .. } => expr_no_atomic(operand),
        Expr::Select {
            cond,
            true_val,
            false_val,
        } => expr_no_atomic(cond) && expr_no_atomic(true_val) && expr_no_atomic(false_val),
        Expr::Fma { a, b, c } => expr_no_atomic(a) && expr_no_atomic(b) && expr_no_atomic(c),
        Expr::Load { index, .. } => expr_no_atomic(index),
        Expr::Cast { value, .. } => expr_no_atomic(value),
        Expr::Call { args, .. } => args.iter().all(expr_no_atomic),
        Expr::SubgroupBallot { cond } => expr_no_atomic(cond),
        Expr::SubgroupShuffle { value, lane } => expr_no_atomic(value) && expr_no_atomic(lane),
        Expr::SubgroupReduce { value, .. } => expr_no_atomic(value),
        Expr::Opaque(_) => false,
        _ => true,
    }
}
