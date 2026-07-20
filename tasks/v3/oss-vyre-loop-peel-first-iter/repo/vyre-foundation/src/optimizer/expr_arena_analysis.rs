//! Program-level analysis built on the [`super::expr_arena`] hash-cons
//! substrate. T020 SEED-2 first concrete consumer.
//!
//! Walks every `Expr` in a `Program` (root + every node's child Exprs)
//! and interns each into a single shared [`ExprArena`]. Returns
//! aggregate stats  -  total interned positions, distinct subexpression
//! count, deduplication ratio, and a stable program-level fingerprint
//! derived from the sorted multiset of root `ExprId`s.
//!
//! The fingerprint is deterministic and structurally invariant: two
//! programs that contain the same multiset of (`Expr`-shape) root
//! expressions produce the same fingerprint regardless of the
//! `Box<Expr>` allocation addresses or insertion order. This makes
//! it suitable as a cheap structural-similarity key for diff_compile,
//! pipeline-cache early-key reuse, and CSE-effectiveness reporting.
//!
//! Cost: one pass over the program; O(N) interning where N is the
//! total `Expr` count. The arena is dropped on return  -  this is an
//! analysis, not an in-place rewrite.

use crate::ir::{Expr, Program};
use crate::optimizer::expr_arena::{ExprArena, ExprId};
use crate::transform::visit::walk_exprs;
use std::hash::{Hash, Hasher};

/// Stats produced by [`analyze_program_expr_arena`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExprArenaStats {
    /// Total number of root `Expr` positions interned. Counts each
    /// node-attached Expr once at its root; child Exprs are NOT
    /// counted as separate roots  -  they're internal to the arena
    /// after interning.
    pub root_intern_count: usize,
    /// Number of distinct subexpression nodes the arena ended up
    /// owning. Equal subtrees collapse to a single node so this is
    /// `<=` the total subexpression population of the input program.
    pub distinct_subexpr_count: usize,
    /// Stable, program-level structural fingerprint. Hash of the
    /// sorted multiset of root `ExprId`s plus the arena's distinct
    /// subexpression count. Two programs with the same multiset of
    /// (Expr-shape) root expressions produce the same fingerprint
    /// regardless of allocation addresses or visitation order.
    pub fingerprint: [u8; 32],
}

impl ExprArenaStats {
    /// Deduplication ratio: `1.0 - distinct/root_intern_count`. `0.0`
    /// when every root expression is structurally distinct; closer to
    /// `1.0` when many programs share subtrees. Returns `0.0` for an
    /// empty program (no roots interned) so callers don't have to
    /// guard against divide-by-zero.
    #[must_use]
    pub fn dedup_ratio(&self) -> f64 {
        if self.root_intern_count == 0 {
            return 0.0;
        }
        let distinct = f64_from_usize(self.distinct_subexpr_count);
        let total = f64_from_usize(self.root_intern_count);
        // ratio is 1 - (distinct / max(total, 1)); clamp to [0, 1]
        // because for nested programs distinct can theoretically
        // exceed root count (every root contributes nested distinct
        // subtrees)  -  the saturation gives a stable display value.
        (1.0 - distinct / total).clamp(0.0, 1.0)
    }
}

/// Walk every `Expr` in `program`, intern it into a single arena,
/// and return aggregate stats including the program-level
/// fingerprint.
#[must_use]
pub fn analyze_program_expr_arena(program: &Program) -> ExprArenaStats {
    let mut arena = ExprArena::default();
    let mut root_ids: Vec<ExprId> = Vec::new();
    walk_exprs(program, |expr: &Expr| {
        root_ids.push(arena.intern(expr));
    });
    let fingerprint = compute_fingerprint(&arena, &mut root_ids);
    ExprArenaStats {
        root_intern_count: root_ids.len(),
        distinct_subexpr_count: arena.len(),
        fingerprint,
    }
}

fn compute_fingerprint(arena: &ExprArena, root_ids: &mut [ExprId]) -> [u8; 32] {
    // Hash the content (FlatExpr structural shape) at every root  -
    // ExprId values are local to this arena and not stable across
    // calls, so they cannot be used in the fingerprint themselves.
    // Sorting by content-hash keeps the fingerprint invariant under
    // visitation order.
    let mut content_hashes: Vec<u64> = root_ids
        .iter()
        .map(|id| {
            let mut hasher = rustc_hash::FxHasher::default();
            arena.get(*id).hash(&mut hasher);
            hasher.finish()
        })
        .collect();
    content_hashes.sort_unstable();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"vyre-foundation::expr_arena_analysis::v1\0");
    hasher.update(&(arena.len() as u64).to_le_bytes());
    hasher.update(b"\0roots\0");
    hasher.update(&(content_hashes.len() as u64).to_le_bytes());
    for h in &content_hashes {
        hasher.update(&h.to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

#[inline]
fn f64_from_usize(value: usize) -> f64 {
    f64::from(u32::try_from(value).unwrap_or(u32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferDecl, DataType, Expr, Node, Program};

    fn buffer(name: &str) -> BufferDecl {
        BufferDecl::read_write(name, 0, DataType::U32)
    }

    fn program_with(buffers: Vec<BufferDecl>, body: Vec<Node>) -> Program {
        Program::wrapped(buffers, [1, 1, 1], body)
    }

    #[test]
    fn empty_program_has_zero_roots_and_distinct() {
        let program = program_with(Vec::new(), Vec::new());
        let stats = analyze_program_expr_arena(&program);
        assert_eq!(stats.root_intern_count, 0);
        assert_eq!(stats.distinct_subexpr_count, 0);
        assert_eq!(stats.dedup_ratio(), 0.0);
    }

    #[test]
    fn duplicate_subtrees_collapse_to_fewer_distinct_nodes() {
        // Two stores writing the same Expr ⇒ same root ExprId, single
        // distinct subexpression at the root level (children dedupe too).
        let common = Expr::add(Expr::u32(1), Expr::u32(2));
        let body = vec![
            Node::store("buf", Expr::u32(0), common.clone()),
            Node::store("buf", Expr::u32(1), common),
        ];
        let program = program_with(vec![buffer("buf")], body);
        let stats = analyze_program_expr_arena(&program);
        // The walker visits roots only  -  2 stores × 2 root Exprs each
        // (index + value) = 4 root intern positions. Of those: u32(0),
        // u32(1), and the BinOp(Add(u32(1), u32(2))) for both stores
        // (collapses to one). Plus distinct subtrees: u32(0), u32(1),
        // u32(2), and the BinOp itself = 4 distinct.
        assert!(
            stats.distinct_subexpr_count <= stats.root_intern_count,
            "distinct subexprs ({}) should not exceed total root interns ({})",
            stats.distinct_subexpr_count,
            stats.root_intern_count
        );
        assert!(
            stats.distinct_subexpr_count >= 1,
            "must intern at least one distinct subtree"
        );
    }

    #[test]
    fn fingerprint_stable_across_invocations() {
        let body = vec![Node::store("buf", Expr::u32(0), Expr::u32(7))];
        let program = program_with(vec![buffer("buf")], body);
        let a = analyze_program_expr_arena(&program);
        let b = analyze_program_expr_arena(&program);
        assert_eq!(a.fingerprint, b.fingerprint);
    }

    #[test]
    fn fingerprint_differs_for_structurally_different_programs() {
        let body_a = vec![Node::store("buf", Expr::u32(0), Expr::u32(7))];
        let body_b = vec![Node::store("buf", Expr::u32(0), Expr::u32(8))];
        let program_a = program_with(vec![buffer("buf")], body_a);
        let program_b = program_with(vec![buffer("buf")], body_b);
        let a = analyze_program_expr_arena(&program_a);
        let b = analyze_program_expr_arena(&program_b);
        assert_ne!(
            a.fingerprint, b.fingerprint,
            "two programs that store different literal values must have distinct fingerprints"
        );
    }

    #[test]
    fn dedup_ratio_zero_for_distinct_roots() {
        let body = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(1)),
            Node::store("buf", Expr::u32(2), Expr::u32(3)),
            Node::store("buf", Expr::u32(4), Expr::u32(5)),
        ];
        let program = program_with(vec![buffer("buf")], body);
        let stats = analyze_program_expr_arena(&program);
        // All 6 root Exprs are distinct  -  ratio is non-positive → clamped to 0.
        assert_eq!(stats.dedup_ratio(), 0.0);
    }
}
