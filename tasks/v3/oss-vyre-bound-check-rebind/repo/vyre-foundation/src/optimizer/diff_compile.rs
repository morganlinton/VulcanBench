//! Differential compilation via wire-content-hash Merkle.
//!
//! Op id: `vyre-foundation::optimizer::diff_compile`. Soundness: `Exact` over
//! the canonical wire-encoding contract. Cost-direction: read-only  -  never
//! mutates the IR, only computes content hashes. Preserves: every analysis.
//! Invalidates: nothing.
//!
//! ## Why
//!
//! When the optimizer rewrites a 100k-op Program, downstream backend compilation
//! (target emission / pipeline cache lookup) typically re-traverses the
//! whole Program from scratch. With ~250 passes landing in the catalog, the
//! same subtrees survive most rewrites unchanged  -  but without a stable
//! per-subtree fingerprint, the backend can't tell what's actually different
//! and pays the full traversal cost on every recompile.
//!
//! This module ships per-Node + per-Region content hashes derived from the
//! canonical wire encoding. Two Nodes that wire-encode to identical bytes
//! produce identical hashes; two Nodes that differ in any way produce
//! different hashes. The hashes are stable across version-compatible wire
//! encodings: if two subtrees encode to identical canonical bytes, their
//! subtree hashes match; if the canonical bytes differ, the hashes differ.
//!
//! ## Use
//!
//! Backends maintain a `<subtree_hash, CompiledArtifact>` cache. On
//! `ProgramPass::transform` boundary, walk the new Program; for each top-level
//! Region (or any subtree boundary), compute the hash; on cache hit, reuse the
//! compiled bytes; on miss, compile + cache. The end-to-end win is proportional
//! to the fraction of subtrees that survive each rewrite  -  for typical optimizer
//! workloads where one peephole touches < 1% of nodes, this drops recompile
//! time by 99%+.
//!
//! ## Algorithm
//!
//! For a `Node`:
//!   `subtree_hash = BLAKE3(put_node(node))`
//!
//! For a `Region`:
//!   `region_hash = BLAKE3(put_nodes([Node::Region { ... self }]))`
//!
//! For a `Program`'s top-level subtrees:
//!   walk `program.entry()`, hash each entry as a Node.
//!
//! The recursion is implicit in the canonical wire encoding: `put_node` on a
//! Region serializes its body inline, so the hash of a parent depends on the
//! bytes of every child. This is the Merkle property  -  change any leaf and
//! every ancestor's hash changes.
//!
//! ## Why not hash IR pointers
//!
//! Pointer-based identity (e.g. `Arc::ptr_eq` on `Arc<Vec<Node>>`) is faster
//! but fragile: any IR pass that clones-and-modifies a body produces a new Arc
//! even if the result is byte-identical, defeating cache hits across rewrites.
//! Content hashing pays a one-time per-subtree cost and survives identity-
//! erasing transforms.

use crate::ir::{Node, Program};
use crate::serial::wire::encode::{put_node, put_nodes};

/// 32-byte BLAKE3 content hash of a single `Node` and its entire subtree
/// (recursively, because the canonical wire encoding includes every child).
///
/// # Errors
///
/// Returns `Err(String)` only when the underlying canonical wire encoder
/// rejects `node`  -  every shipped `Node` variant has a defined encoding, so
/// this signals a substrate bug, not a normal failure mode.
///
/// # Examples
///
/// Two Nodes that wire-encode to identical bytes share a hash; modifying any
/// child changes the parent's hash.
pub fn node_subtree_hash(node: &Node) -> Result<[u8; 32], String> {
    let mut buf = Vec::with_capacity(64);
    put_node(&mut buf, node).map_err(String::from)?;
    Ok(*blake3::hash(&buf).as_bytes())
}

/// 32-byte BLAKE3 content hash of an entire `Node` slice as a single unit.
///
/// Use this when you want one hash that covers a body's exact ordered
/// contents  -  the natural granularity for hashing a `Node::Region`'s
/// inner body, an `If` branch arm, or a `Loop` body without wrapping it
/// in a synthetic enclosing Node. The hash matches the canonical wire
/// encoding of the slice (length prefix + per-node serialization), so
/// equal slices encode to equal bytes and produce equal hashes.
///
/// # Errors
///
/// Returns `Err(String)` only when the underlying canonical wire encoder
/// rejects any node in `body`; signals a substrate bug.
pub fn nodes_subtree_hash(body: &[Node]) -> Result<[u8; 32], String> {
    let mut buf = Vec::with_capacity(64usize.saturating_mul(body.len().max(1)));
    put_nodes(&mut buf, body).map_err(String::from)?;
    Ok(*blake3::hash(&buf).as_bytes())
}

/// 32-byte BLAKE3 content hashes of every top-level entry in `program`'s
/// dispatch sequence, paired with the entry index.
///
/// Backends use this to drive `<subtree_hash, CompiledArtifact>` cache
/// lookup at the natural granularity of one entry per dispatch arm. The hash
/// is computed via `node_subtree_hash` on each entry; ordering matches
/// `program.entry()`.
///
/// # Errors
///
/// Propagates `node_subtree_hash` errors; signals a substrate bug.
pub fn program_subtree_hashes(program: &Program) -> Result<Vec<(usize, [u8; 32])>, String> {
    program
        .entry()
        .iter()
        .enumerate()
        .map(|(idx, node)| node_subtree_hash(node).map(|hash| (idx, hash)))
        .collect()
}

/// Compute the diff between two programs as the set of entry-index positions
/// whose content hash changed. Positions present in `before` but not `after`
/// (or vice versa) appear as `Diff::Removed` / `Diff::Added`. Positions whose
/// hash is unchanged appear as `Diff::Unchanged`. Positions whose hash
/// differs at the same index appear as `Diff::Changed`.
///
/// Backends use this to skip recompilation of `Diff::Unchanged` entries and
/// only recompile the rest. The end-to-end speedup is proportional to the
/// fraction of `Diff::Unchanged` entries.
///
/// # Errors
///
/// Propagates `program_subtree_hashes` errors.
pub fn program_diff(before: &Program, after: &Program) -> Result<Vec<Diff>, String> {
    let before_hashes = program_subtree_hashes(before)?;
    let after_hashes = program_subtree_hashes(after)?;
    let mut out = Vec::with_capacity(before_hashes.len().max(after_hashes.len()));
    for idx in 0..before_hashes.len().max(after_hashes.len()) {
        match (before_hashes.get(idx), after_hashes.get(idx)) {
            (Some(b), Some(a)) if b.1 == a.1 => out.push(Diff::Unchanged {
                index: idx,
                hash: a.1,
            }),
            (Some(b), Some(a)) => out.push(Diff::Changed {
                index: idx,
                before_hash: b.1,
                after_hash: a.1,
            }),
            (Some(b), None) => out.push(Diff::Removed {
                index: idx,
                hash: b.1,
            }),
            (None, Some(a)) => out.push(Diff::Added {
                index: idx,
                hash: a.1,
            }),
            (None, None) => unreachable!("loop bounds prevent both being None"),
        }
    }
    Ok(out)
}

/// Per-entry diff verdict between two Programs. Backends consume `Vec<Diff>`
/// from `program_diff` to drive incremental recompile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Diff {
    /// Same hash at the same entry index  -  backend may reuse the cached
    /// compiled artifact for this entry.
    Unchanged {
        /// Entry-position index within `program.entry()`.
        index: usize,
        /// Content hash shared by `before` and `after`.
        hash: [u8; 32],
    },
    /// Different hash at the same entry index  -  backend must recompile.
    Changed {
        /// Entry-position index within `program.entry()`.
        index: usize,
        /// Content hash from the `before` Program.
        before_hash: [u8; 32],
        /// Content hash from the `after` Program.
        after_hash: [u8; 32],
    },
    /// Entry index present in `before` but not in `after`  -  backend may
    /// release the cached artifact (or keep it for future reuse, the diff
    /// simply names the absence).
    Removed {
        /// Entry-position index within `before.entry()`.
        index: usize,
        /// Content hash from the `before` Program.
        hash: [u8; 32],
    },
    /// Entry index present in `after` but not in `before`  -  backend must
    /// compile fresh.
    Added {
        /// Entry-position index within `after.entry()`.
        index: usize,
        /// Content hash from the `after` Program.
        hash: [u8; 32],
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

    fn program_with(node: Node) -> Program {
        Program::wrapped(
            vec![
                BufferDecl::storage("buf", 0, BufferAccess::ReadWrite, DataType::U32).with_count(4),
            ],
            [1, 1, 1],
            vec![node],
        )
    }

    #[test]
    fn identical_nodes_share_hash() {
        let n1 = Node::store("buf", Expr::u32(0), Expr::u32(7));
        let n2 = Node::store("buf", Expr::u32(0), Expr::u32(7));
        let h1 = node_subtree_hash(&n1).expect("Fix: encoding must succeed for valid Node");
        let h2 = node_subtree_hash(&n2).expect("Fix: encoding must succeed for valid Node");
        assert_eq!(h1, h2, "identical nodes must produce identical hashes");
    }

    #[test]
    fn nodes_subtree_hash_matches_for_identical_bodies() {
        let body_a = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(7)),
            Node::store("buf", Expr::u32(1), Expr::u32(8)),
        ];
        let body_b = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(7)),
            Node::store("buf", Expr::u32(1), Expr::u32(8)),
        ];
        let h_a = nodes_subtree_hash(&body_a).expect("Fix: encoding must succeed");
        let h_b = nodes_subtree_hash(&body_b).expect("Fix: encoding must succeed");
        assert_eq!(h_a, h_b);
    }

    #[test]
    fn nodes_subtree_hash_differs_when_order_changes() {
        let body_in_order = vec![
            Node::store("buf", Expr::u32(0), Expr::u32(7)),
            Node::store("buf", Expr::u32(1), Expr::u32(8)),
        ];
        let body_swapped = vec![
            Node::store("buf", Expr::u32(1), Expr::u32(8)),
            Node::store("buf", Expr::u32(0), Expr::u32(7)),
        ];
        let h_in_order = nodes_subtree_hash(&body_in_order).expect("Fix: encoding must succeed");
        let h_swapped = nodes_subtree_hash(&body_swapped).expect("Fix: encoding must succeed");
        assert_ne!(
            h_in_order, h_swapped,
            "ordered slice contents must affect the hash  -  backends rely on body order being load-bearing"
        );
    }

    #[test]
    fn differing_nodes_differ_in_hash() {
        let n1 = Node::store("buf", Expr::u32(0), Expr::u32(7));
        let n2 = Node::store("buf", Expr::u32(0), Expr::u32(8));
        let h1 = node_subtree_hash(&n1).expect("Fix: encoding must succeed for valid Node");
        let h2 = node_subtree_hash(&n2).expect("Fix: encoding must succeed for valid Node");
        assert_ne!(
            h1, h2,
            "Nodes that differ in any field must produce different hashes (Merkle property)"
        );
    }

    #[test]
    fn nested_change_propagates_to_parent_hash() {
        // Outer If wraps an inner Store. Change the Store's value; the outer
        // If's hash MUST change too (Merkle property).
        let inner1 = vec![Node::store("buf", Expr::u32(0), Expr::u32(7))];
        let inner2 = vec![Node::store("buf", Expr::u32(0), Expr::u32(8))];
        let outer1 = Node::if_then(Expr::bool(true), inner1);
        let outer2 = Node::if_then(Expr::bool(true), inner2);
        let h1 = node_subtree_hash(&outer1).expect("Fix: encoding must succeed");
        let h2 = node_subtree_hash(&outer2).expect("Fix: encoding must succeed");
        assert_ne!(
            h1, h2,
            "changing a leaf must change the parent's hash  -  without this the cache would \
             return stale compiled artifacts after a deep rewrite"
        );
    }

    #[test]
    fn program_subtree_hashes_indexes_each_top_level_entry() {
        // Program::wrapped puts everything inside one outer Region, so the
        // subtree-hash list has length 1 (the wrapper Region), and the hash
        // depends on the body inside.
        let p1 = program_with(Node::store("buf", Expr::u32(0), Expr::u32(7)));
        let p2 = program_with(Node::store("buf", Expr::u32(0), Expr::u32(8)));
        let hs1 = program_subtree_hashes(&p1).expect("Fix: encoding must succeed");
        let hs2 = program_subtree_hashes(&p2).expect("Fix: encoding must succeed");
        assert_eq!(
            hs1.len(),
            1,
            "Program::wrapped exposes one top-level Region"
        );
        assert_eq!(hs2.len(), 1);
        assert_eq!(hs1[0].0, 0, "index ordering follows program.entry()");
        assert_ne!(
            hs1[0].1, hs2[0].1,
            "differing inner content must produce differing top-level hashes"
        );
    }

    #[test]
    fn program_diff_marks_unchanged_entries() {
        let p1 = program_with(Node::store("buf", Expr::u32(0), Expr::u32(7)));
        let p2 = program_with(Node::store("buf", Expr::u32(0), Expr::u32(7)));
        let diffs = program_diff(&p1, &p2).expect("Fix: encoding must succeed");
        assert_eq!(diffs.len(), 1);
        assert!(matches!(diffs[0], Diff::Unchanged { index: 0, .. }));
    }

    #[test]
    fn program_diff_marks_changed_entries() {
        let p1 = program_with(Node::store("buf", Expr::u32(0), Expr::u32(7)));
        let p2 = program_with(Node::store("buf", Expr::u32(0), Expr::u32(99)));
        let diffs = program_diff(&p1, &p2).expect("Fix: encoding must succeed");
        assert_eq!(diffs.len(), 1);
        match &diffs[0] {
            Diff::Changed {
                index,
                before_hash,
                after_hash,
            } => {
                assert_eq!(*index, 0);
                assert_ne!(before_hash, after_hash);
            }
            other => panic!(
                "expected Diff::Changed for an entry whose inner content differs; got {other:?}"
            ),
        }
    }

    #[test]
    fn hash_is_stable_across_repeated_calls() {
        // Same Node, same input bytes, same hash on every call. Without this
        // contract, cache lookups would silently miss on repeat queries.
        let node = Node::store("buf", Expr::u32(0), Expr::u32(7));
        let h1 = node_subtree_hash(&node).expect("Fix: encoding must succeed");
        let h2 = node_subtree_hash(&node).expect("Fix: encoding must succeed");
        let h3 = node_subtree_hash(&node).expect("Fix: encoding must succeed");
        assert_eq!(h1, h2);
        assert_eq!(h2, h3);
    }
}
