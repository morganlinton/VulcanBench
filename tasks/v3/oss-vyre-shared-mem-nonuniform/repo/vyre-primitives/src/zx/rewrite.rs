//! ZX-diagram rewrite rules: spider fusion, identity removal, color
//! change. Pure-CPU primitive over a `Vec<ZxSpider>` + edge multiset.

extern crate alloc;
use alloc::vec::Vec;

/// Spider color: Z (green / phase basis) or X (red / spider basis).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZxColor {
    /// Z spider (green).
    Z,
    /// X spider (red).
    X,
}

impl ZxColor {
    /// The opposite color (Hadamard conjugation).
    #[must_use]
    #[inline]
    pub const fn flip(self) -> Self {
        match self {
            Self::Z => Self::X,
            Self::X => Self::Z,
        }
    }
}

/// One ZX spider  -  a graph vertex with color + phase numerator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZxSpider {
    /// Z or X.
    pub color: ZxColor,
    /// Phase numerator over the diagram's `phase_denom`. Two phases
    /// are equal iff numerators agree mod the denominator.
    pub phase_num: u32,
}

/// ZX diagram: spider list + multiset of (u, v) edges. The
/// `phase_denom` defines the discrete phase group (2π · k / denom for
/// k = 0..denom-1). Self-loops are permitted (they're a no-op modulo
/// the diagram's algebra). Multi-edges are permitted (they're a
/// non-trivial scalar on Z↔X spiders but have no rewrite at this
/// level).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZxDiagram {
    /// Phase denominator. Must be > 0.
    pub phase_denom: u32,
    /// Spider list. Index = vertex id.
    pub spiders: Vec<ZxSpider>,
    /// Edge multiset; each (u, v) is one undirected edge.
    pub edges: Vec<(u32, u32)>,
}

/// Apply spider fusion (S1): merge every adjacent pair of
/// same-color spiders. The merged spider takes the color of its
/// constituents and phase = (a.phase + b.phase) mod denom.
/// Repeats until fixpoint.
///
/// Returns the simplified diagram. The result has every same-color
/// adjacency contracted; cross-color edges and self-loops are
/// preserved.
#[must_use]
pub fn apply_spider_fusion(mut diagram: ZxDiagram) -> ZxDiagram {
    loop {
        let merge_pair = diagram.edges.iter().copied().find(|(u, v)| {
            u != v && diagram.spiders[*u as usize].color == diagram.spiders[*v as usize].color
        });
        let Some((u, v)) = merge_pair else {
            break;
        };
        // Sum phases mod denom; keep `u`, drop `v`.
        let combined = (diagram.spiders[u as usize].phase_num
            + diagram.spiders[v as usize].phase_num)
            % diagram.phase_denom;
        diagram.spiders[u as usize].phase_num = combined;
        // Re-route every edge touching `v` to `u` (drop the merged
        // edge itself).
        let mut next_edges = Vec::with_capacity(diagram.edges.len() - 1);
        for &(a, b) in &diagram.edges {
            if (a == u && b == v) || (a == v && b == u) {
                continue;
            }
            let new_a = if a == v { u } else { a };
            let new_b = if b == v { u } else { b };
            next_edges.push((new_a, new_b));
        }
        diagram.edges = next_edges;
        // Remove spider `v`; renumber every id > v down by one.
        diagram.spiders.remove(v as usize);
        for e in &mut diagram.edges {
            if e.0 > v {
                e.0 -= 1;
            }
            if e.1 > v {
                e.1 -= 1;
            }
        }
    }
    diagram
}

/// Apply identity removal (S2): drop every phase-0 spider whose
/// degree is exactly 2 and whose two neighbors share its color,
/// splicing the two edges into one. Repeats until fixpoint.
#[must_use]
pub fn apply_identity_removal(mut diagram: ZxDiagram) -> ZxDiagram {
    loop {
        let mut removable: Option<u32> = None;
        for v in 0..diagram.spiders.len() {
            let s = diagram.spiders[v];
            if s.phase_num != 0 {
                continue;
            }
            // Collect neighbors via the edge list.
            let neighbors: Vec<u32> = diagram
                .edges
                .iter()
                .filter_map(|&(a, b)| {
                    if a as usize == v {
                        Some(b)
                    } else if b as usize == v {
                        Some(a)
                    } else {
                        None
                    }
                })
                .collect();
            if neighbors.len() != 2 {
                continue;
            }
            // Both neighbors must match the spider's color.
            let color_match = neighbors
                .iter()
                .all(|&n| diagram.spiders[n as usize].color == s.color);
            if color_match {
                removable = Some(v as u32);
                break;
            }
        }
        let Some(v) = removable else { break };
        // Splice: replace the two edges (v, a) and (v, b) with (a, b).
        let mut next_edges = Vec::new();
        let mut endpoints = Vec::new();
        for &(a, b) in &diagram.edges {
            if a == v {
                endpoints.push(b);
            } else if b == v {
                endpoints.push(a);
            } else {
                next_edges.push((a, b));
            }
        }
        if endpoints.len() == 2 {
            next_edges.push((endpoints[0], endpoints[1]));
        }
        diagram.edges = next_edges;
        diagram.spiders.remove(v as usize);
        for e in &mut diagram.edges {
            if e.0 > v {
                e.0 -= 1;
            }
            if e.1 > v {
                e.1 -= 1;
            }
        }
    }
    diagram
}

/// Apply color-change (H) at vertex `v`: flip the color, leave the
/// phase. Equivalent to conjugating that single spider by Hadamards
/// on every incident edge.
///
/// # Panics
///
/// Panics if `v` is out of range.
pub fn apply_color_change(diagram: &mut ZxDiagram, v: u32) {
    let s = &mut diagram.spiders[v as usize];
    s.color = s.color.flip();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z(phase: u32) -> ZxSpider {
        ZxSpider {
            color: ZxColor::Z,
            phase_num: phase,
        }
    }
    fn x(phase: u32) -> ZxSpider {
        ZxSpider {
            color: ZxColor::X,
            phase_num: phase,
        }
    }

    #[test]
    fn fusion_merges_two_z_spiders() {
        // Two Z spiders connected by one edge, phases 1 and 3
        // mod 8 → merged phase 4.
        let d = ZxDiagram {
            phase_denom: 8,
            spiders: vec![z(1), z(3)],
            edges: vec![(0, 1)],
        };
        let out = apply_spider_fusion(d);
        assert_eq!(out.spiders.len(), 1);
        assert_eq!(out.spiders[0].phase_num, 4);
        assert!(out.edges.is_empty());
    }

    #[test]
    fn fusion_does_not_merge_cross_color() {
        // Z - X edge: no fusion.
        let d = ZxDiagram {
            phase_denom: 8,
            spiders: vec![z(1), x(2)],
            edges: vec![(0, 1)],
        };
        let out = apply_spider_fusion(d.clone());
        assert_eq!(out, d);
    }

    #[test]
    fn fusion_phase_wraps_mod_denom() {
        // Z 5 + Z 5 = 10 ≡ 2 (mod 8).
        let d = ZxDiagram {
            phase_denom: 8,
            spiders: vec![z(5), z(5)],
            edges: vec![(0, 1)],
        };
        let out = apply_spider_fusion(d);
        assert_eq!(out.spiders[0].phase_num, 2);
    }

    #[test]
    fn fusion_chain_collapses_to_one() {
        // Z-Z-Z chain: merges to one spider with summed phase.
        let d = ZxDiagram {
            phase_denom: 8,
            spiders: vec![z(1), z(2), z(3)],
            edges: vec![(0, 1), (1, 2)],
        };
        let out = apply_spider_fusion(d);
        assert_eq!(out.spiders.len(), 1);
        assert_eq!(out.spiders[0].phase_num, 6);
    }

    #[test]
    fn identity_removal_drops_phase_zero_degree_two() {
        // Z(α) - Z(0) - Z(β): middle Z is identity, drop it.
        // Result: Z(α) - Z(β) (a single edge).
        let d = ZxDiagram {
            phase_denom: 8,
            spiders: vec![z(1), z(0), z(2)],
            edges: vec![(0, 1), (1, 2)],
        };
        let out = apply_identity_removal(d);
        assert_eq!(out.spiders.len(), 2);
        assert_eq!(out.edges, vec![(0, 1)]);
    }

    #[test]
    fn identity_removal_keeps_nonzero_phase() {
        // Middle spider has phase 1: not identity, keep it.
        let d = ZxDiagram {
            phase_denom: 8,
            spiders: vec![z(1), z(1), z(2)],
            edges: vec![(0, 1), (1, 2)],
        };
        let out = apply_identity_removal(d.clone());
        assert_eq!(out, d);
    }

    /// Closure-bar: identity removal on a chain Z(0) - Z(0) - Z(0)
    /// must collapse to a single edge between the endpoints.
    #[test]
    fn identity_removal_iterates_to_fixpoint() {
        // Z(α) - Z(0) - Z(0) - Z(β).
        // After two iterations of S2: Z(α) - Z(β).
        let d = ZxDiagram {
            phase_denom: 8,
            spiders: vec![z(1), z(0), z(0), z(2)],
            edges: vec![(0, 1), (1, 2), (2, 3)],
        };
        let out = apply_identity_removal(d);
        assert_eq!(out.spiders.len(), 2);
        assert_eq!(out.edges.len(), 1);
    }

    #[test]
    fn color_change_flips_color_keeps_phase() {
        let mut d = ZxDiagram {
            phase_denom: 8,
            spiders: vec![z(3)],
            edges: vec![],
        };
        apply_color_change(&mut d, 0);
        assert_eq!(d.spiders[0].color, ZxColor::X);
        assert_eq!(d.spiders[0].phase_num, 3);
        // Apply twice → back to Z.
        apply_color_change(&mut d, 0);
        assert_eq!(d.spiders[0].color, ZxColor::Z);
    }

    /// Adversarial: Z and X mixed should NOT cause cross-fusion.
    #[test]
    fn mixed_diagram_preserves_cross_color_structure() {
        // Z - Z - X chain: only the Z - Z merges; the X stays.
        let d = ZxDiagram {
            phase_denom: 8,
            spiders: vec![z(1), z(2), x(3)],
            edges: vec![(0, 1), (1, 2)],
        };
        let out = apply_spider_fusion(d);
        assert_eq!(out.spiders.len(), 2);
        // Z(merged) and X(3) connected by one edge.
        assert_eq!(out.edges.len(), 1);
    }

    /// Adversarial: self-loop must not trigger fusion (u == v).
    #[test]
    fn self_loop_does_not_fuse() {
        let d = ZxDiagram {
            phase_denom: 8,
            spiders: vec![z(1)],
            edges: vec![(0, 0)],
        };
        let out = apply_spider_fusion(d.clone());
        assert_eq!(out, d);
    }
}
