//! Region-inline pass.
//!
//! `Node::Region { body, .. }` is a debug-wrapper produced by
//! `vyre-libs` Category-A compositions. The generator/source_region
//! fields are informational; the body IR is no different from the
//! surrounding program. This pass flattens each Region into its body
//! when doing so does not cross a threshold (default: 64 nodes),
//! letting the CSE/DCE passes see compositions as one program instead
//! of a tree of black boxes.
//!
//! Keeping the threshold prevents 100-op compositions from inlining
//! and hiding the Region boundary in backtraces.

use crate::ir::Ident;
use crate::ir_inner::model::node::Node;
use crate::ir_inner::model::program::Program;
use crate::visit::bound_names::count_bound_names;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use std::sync::Arc;

/// Default node-count threshold. Regions whose bodies count ≤ this many
/// nodes inline; larger Regions stay wrapped so tracing spans and
/// conform certificates remain meaningful. A caller can override via
/// [`run_with_threshold`].
pub const DEFAULT_INLINE_THRESHOLD: usize = 64;

/// A child node after recursive flattening, awaiting the level-wide
/// collision decision made in [`inline_nodes_into`]'s second pass.
enum Staged {
    /// A fully-flattened, inlinable Region body. Whether it splices flat or
    /// is wrapped in a `Node::Block` depends on whether any of its top-level
    /// `Let` names also occurs in another sibling at this level.
    FlatRegion(Vec<Node>),
    /// Any other node, emitted verbatim (its children already recursively
    /// flattened).
    Keep(Node),
}

/// Run the pass with the default threshold.
#[must_use]
#[inline]
pub fn run(program: Program) -> Program {
    run_with_threshold(program, DEFAULT_INLINE_THRESHOLD)
}

/// Run the pass with an explicit inline threshold.
#[must_use]
pub fn run_with_threshold(program: Program, threshold: usize) -> Program {
    program.map_entry(|owned_entry| {
        let mut entry = Vec::with_capacity(owned_entry.len());
        inline_nodes_into(owned_entry, threshold, &mut entry);
        entry
    })
}

/// Recursively inline regions, writing the transformed nodes into `out`.
///
/// A flattenable Region's body normally splices straight into `out`, dropping
/// the Region wrapper. But a Region exists precisely to SCOPE its `Let`
/// bindings, and the same sub-op composed more than once into one parent emits
/// the SAME binding names each time (e.g. three FFT stages each binding
/// `u_re_s1_b0_k0` — FINDING-GPU-11). Splicing those flat collides the names as
/// duplicate siblings (V032); wrapping only some of them re-collides as a
/// wrapped sibling shadowing a flat one (V008). So this runs in two passes:
///
/// 1. Recursively flatten every child into a [`Staged`] record and tally how
///    many times each top-level `Let` name occurs across the whole sibling
///    level (a flattened region's top-level names plus any bare sibling `Let`).
/// 2. Emit in order. A flattened region whose body declares a top-level `Let`
///    whose name occurs more than once at this level is wrapped in a
///    `Node::Block` (a lexical scope the validator honors) so EVERY colliding
///    sibling lands in its own scope. Regions whose names are all unique at this
///    level splice flat — the common case, including the lone root Region every
///    `Program::wrapped` builds, so non-colliding programs are byte-unchanged.
fn inline_nodes_into(nodes: Vec<Node>, threshold: usize, out: &mut Vec<Node>) {
    let mut staged: Vec<Staged> = Vec::with_capacity(nodes.len());

    for node in nodes {
        match node {
            Node::Region {
                body,
                generator,
                source_region,
            } => {
                let count = count_nodes_capped(&body, threshold);
                // VYRE_IR_HOTSPOTS CRIT: `(*body).clone()` cloned the whole inner
                // Vec<Node> unconditionally. try_unwrap first so a uniquely-owned
                // Arc yields the inner Vec without copying; clone only when
                // another owner still holds the Arc.
                let body_vec = match Arc::try_unwrap(body) {
                    Ok(v) => v,
                    Err(arc) => (*arc).clone(),
                };
                if count <= threshold {
                    let mut inlined = Vec::with_capacity(body_vec.len());
                    inline_nodes_into(body_vec, threshold, &mut inlined);
                    staged.push(Staged::FlatRegion(inlined));
                } else {
                    let mut new_body = Vec::with_capacity(body_vec.len());
                    inline_nodes_into(body_vec, threshold, &mut new_body);
                    staged.push(Staged::Keep(Node::Region {
                        generator,
                        source_region,
                        body: Arc::new(new_body),
                    }));
                }
            }
            Node::Block(children) => {
                let mut new_children = Vec::with_capacity(children.len());
                inline_nodes_into(children, threshold, &mut new_children);
                staged.push(Staged::Keep(Node::Block(new_children)));
            }
            Node::Loop {
                var,
                from,
                to,
                body,
            } => {
                let mut new_body = Vec::with_capacity(body.len());
                inline_nodes_into(body, threshold, &mut new_body);
                staged.push(Staged::Keep(Node::Loop {
                    var,
                    from,
                    to,
                    body: new_body,
                }));
            }
            Node::If {
                cond,
                then,
                otherwise,
            } => {
                let mut new_then = Vec::with_capacity(then.len());
                let mut new_otherwise = Vec::with_capacity(otherwise.len());
                inline_nodes_into(then, threshold, &mut new_then);
                inline_nodes_into(otherwise, threshold, &mut new_otherwise);
                staged.push(Staged::Keep(Node::If {
                    cond,
                    then: new_then,
                    otherwise: new_otherwise,
                }));
            }
            other => staged.push(Staged::Keep(other)),
        }
    }

    // Count every binding (each `Let` name and loop variable) at this
    // sibling level, recursively. A flattened Region's top-level `let x`
    // leaks into the parent scope; if `x` is ALSO bound anywhere else at
    // this level — another top-level let, OR a binding NESTED inside a
    // sibling's If/Loop/Block — the leaked binding overlaps that other
    // binder and collides ("V008: duplicate local binding `x`"). The IR
    // disallows shadowing, so each name binds at most once per live scope:
    // a level-wide count >= 2 means two distinct binders that the original
    // Region scope kept apart. (The earlier tally counted only TOP-LEVEL
    // sibling lets, so it missed nested binders — see
    // tests/region_inline_scope.rs for the oracle-differential proof.)
    let mut bound_counts: FxHashMap<Ident, usize> = FxHashMap::default();
    for item in &staged {
        match item {
            Staged::FlatRegion(body) => count_bound_names(body, &mut bound_counts),
            Staged::Keep(node) => count_bound_names(std::slice::from_ref(node), &mut bound_counts),
        }
    }

    for item in staged {
        match item {
            Staged::FlatRegion(mut body) => {
                let collides = body.iter().any(|node| match node {
                    Node::Let { name, .. } => {
                        bound_counts.get(name.as_str()).copied().unwrap_or(0) >= 2
                    }
                    _ => false,
                });
                if collides {
                    out.push(Node::Block(body));
                } else {
                    out.append(&mut body);
                }
            }
            Staged::Keep(node) => out.push(node),
        }
    }
}

fn count_nodes_capped(nodes: &[Node], threshold: usize) -> usize {
    let cap = threshold.saturating_add(1);
    let mut count = 0usize;
    let mut stack: SmallVec<[&[Node]; 16]> = SmallVec::new();
    stack.push(nodes);

    while let Some(nodes) = stack.pop() {
        for node in nodes {
            count = count.saturating_add(1);
            if count >= cap {
                return cap;
            }
            match node {
                Node::Block(children) | Node::Loop { body: children, .. } => {
                    stack.push(children);
                }
                Node::If {
                    then, otherwise, ..
                } => {
                    stack.push(otherwise);
                    stack.push(then);
                }
                Node::Region { body, .. } => {
                    stack.push(body);
                }
                _ => {}
            }
        }
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{BufferDecl, DataType, Expr, Program};

    #[test]
    fn small_region_inlines() {
        let body = vec![Node::store("out", Expr::u32(0), Expr::u32(42))];
        let region = Node::Region {
            generator: "test".into(),
            source_region: None,
            body: std::sync::Arc::new(body),
        };
        let prog = Program::wrapped(
            vec![BufferDecl::read_write("out", 0, DataType::U32)],
            [1, 1, 1],
            vec![region],
        );
        let optimized = run(prog);
        assert!(
            !matches!(&optimized.entry()[0], Node::Region { .. }),
            "small Region must inline"
        );
        assert!(matches!(&optimized.entry()[0], Node::Store { .. }));
    }

    #[test]
    fn large_region_stays_wrapped() {
        let body: Vec<Node> = (0..100)
            .map(|i| Node::store("out", Expr::u32(i), Expr::u32(i)))
            .collect();
        let region = Node::Region {
            generator: "test".into(),
            source_region: None,
            body: std::sync::Arc::new(body),
        };
        let prog = Program::wrapped(
            vec![BufferDecl::read_write("out", 0, DataType::U32)],
            [1, 1, 1],
            vec![region],
        );
        let optimized = run_with_threshold(prog, 64);
        assert!(
            matches!(&optimized.entry()[0], Node::Region { .. }),
            "large Region must stay wrapped"
        );
    }

    #[test]
    fn generated_large_region_count_is_capped_at_inline_threshold() {
        let body: Vec<Node> = (0..4096)
            .map(|i| Node::store("out", Expr::u32(i), Expr::u32(i)))
            .collect();

        assert_eq!(
            count_nodes_capped(&body, 64),
            65,
            "Fix: region-inline must stop counting once a generated body exceeds the inline threshold."
        );
    }

    #[test]
    fn nested_small_regions_all_inline() {
        let inner = Node::Region {
            generator: "inner".into(),
            source_region: None,
            body: std::sync::Arc::new(vec![Node::store("out", Expr::u32(0), Expr::u32(1))]),
        };
        let outer = Node::Region {
            generator: "outer".into(),
            source_region: None,
            body: std::sync::Arc::new(vec![inner]),
        };
        let prog = Program::wrapped(
            vec![BufferDecl::read_write("out", 0, DataType::U32)],
            [1, 1, 1],
            vec![outer],
        );
        let optimized = run(prog);
        // Both Regions inlined  -  only the Store remains.
        assert_eq!(optimized.entry().len(), 1);
        assert!(matches!(&optimized.entry()[0], Node::Store { .. }));
    }

    #[test]
    fn colliding_sibling_lets_are_each_block_scoped() {
        // Two sibling regions that each bind the SAME name (the FFT-stage
        // pattern behind FINDING-GPU-11). Splicing either flat would expose
        // `u_re_s1_b0_k0` at the parent scope, so the other sibling — whether
        // spliced (V032 duplicate sibling) or wrapped (V008 shadow of the flat
        // one) — would fail validation. The collision is detected level-wide,
        // so BOTH siblings are wrapped in their own Block: two co-equal scopes,
        // neither shadowing the other, the shared name absent from top level.
        let mk = |gen: &str| Node::Region {
            generator: gen.into(),
            source_region: None,
            body: Arc::new(vec![
                Node::let_bind("u_re_s1_b0_k0", Expr::u32(1)),
                Node::store("out", Expr::u32(0), Expr::var("u_re_s1_b0_k0")),
            ]),
        };
        let prog = Program::wrapped(
            vec![BufferDecl::read_write("out", 0, DataType::U32)],
            [1, 1, 1],
            vec![mk("fft_stage_a"), mk("fft_stage_b")],
        );
        let entry = run(prog).into_entry_vec();

        // No surviving Region wrappers, and the shared name is NEVER a
        // top-level sibling — it lives only inside the per-sibling Blocks.
        assert!(
            !entry.iter().any(|n| matches!(n, Node::Region { .. })),
            "both small regions must inline, got {entry:?}"
        );
        assert!(
            !entry
                .iter()
                .any(|n| matches!(n, Node::Let { name, .. } if name == "u_re_s1_b0_k0")),
            "the shared name must not appear at top level (would collide), got {entry:?}"
        );

        // Exactly two Blocks, each carrying its own copy of the binding.
        let blocks: Vec<&Vec<Node>> = entry
            .iter()
            .filter_map(|n| match n {
                Node::Block(b) => Some(b),
                _ => None,
            })
            .collect();
        assert_eq!(blocks.len(), 2, "each colliding sibling gets its own Block");
        for block_body in blocks {
            assert!(
                matches!(&block_body[0], Node::Let { name, .. } if name == "u_re_s1_b0_k0"),
                "each Block scopes one copy of the shared let, got {block_body:?}"
            );
        }
    }

    #[test]
    fn regions_inside_loops_also_inline() {
        let region = Node::Region {
            generator: "inner".into(),
            source_region: None,
            body: std::sync::Arc::new(vec![Node::store("out", Expr::var("i"), Expr::u32(1))]),
        };
        let loop_node = Node::loop_for("i", Expr::u32(0), Expr::u32(4), vec![region]);
        let prog = Program::wrapped(
            vec![BufferDecl::read_write("out", 0, DataType::U32)],
            [1, 1, 1],
            vec![loop_node],
        );
        let optimized = run(prog);
        let Node::Loop { body, .. } = &optimized.entry()[0] else {
            panic!("expected Loop");
        };
        assert_eq!(body.len(), 1);
        assert!(
            matches!(&body[0], Node::Store { .. }),
            "Region inside Loop must inline to just the Store"
        );
    }

    /// Structural twin of `tests/region_inline_scope.rs`: a small Region
    /// binding `let x` followed by a sibling `if c { let x = ... }`. The
    /// nested `let x` lives in a scope the original Region kept disjoint, so
    /// flattening the Region must NOT splice its `let x` flat into the parent
    /// (that would leak `x` live across the If, colliding with the nested
    /// binder -> "V008: duplicate local binding `x`"). The level-wide
    /// bound-name count sees the nested `x`, so the flattened Region is
    /// wrapped in its own `Block`.
    ///
    /// Pre-fix the tally counted only TOP-LEVEL sibling lets, missed the
    /// nested `let x`, and spliced the Region flat -> `out[0]` was a bare
    /// leaked `Node::Let`, not a `Node::Block`. (The oracle-differential proof
    /// that the leaked program is scope-invalid lives in
    /// `tests/region_inline_scope.rs`.)
    #[test]
    fn region_with_nested_sibling_binder_is_block_scoped() {
        let region = Node::Region {
            generator: "stage".into(),
            source_region: None,
            body: Arc::new(vec![
                Node::let_bind("x", Expr::u32(1)),
                Node::store("out", Expr::u32(0), Expr::var("x")),
            ]),
        };
        let sibling = Node::If {
            cond: Expr::eq(Expr::u32(0), Expr::u32(0)),
            then: vec![
                Node::let_bind("x", Expr::u32(2)),
                Node::store("out", Expr::u32(0), Expr::var("x")),
            ],
            otherwise: vec![],
        };

        let mut out = Vec::new();
        inline_nodes_into(vec![region, sibling], DEFAULT_INLINE_THRESHOLD, &mut out);

        // The Region must NOT be spliced flat: its `let x` collides level-wide
        // with the If's nested `let x`, so it is re-wrapped in its own Block.
        assert_eq!(out.len(), 2, "Region wrapped + If kept, got {out:?}");
        let Node::Block(block_body) = &out[0] else {
            panic!(
                "flattened Region must be re-wrapped in a Block, got {:?}",
                out[0]
            );
        };
        assert!(
            matches!(&block_body[0], Node::Let { name, .. } if name == "x"),
            "the Block scopes the Region's `let x`, got {block_body:?}"
        );
        // The sibling If is untouched and still binds its own nested `x`.
        assert!(
            matches!(&out[1], Node::If { .. }),
            "sibling If preserved, got {:?}",
            out[1]
        );
        // `x` must NEVER appear as a bare top-level Let -- that is the leak.
        assert!(
            !out
                .iter()
                .any(|n| matches!(n, Node::Let { name, .. } if name == "x")),
            "no bare top-level `let x` may leak into the parent scope, got {out:?}"
        );
    }
}
