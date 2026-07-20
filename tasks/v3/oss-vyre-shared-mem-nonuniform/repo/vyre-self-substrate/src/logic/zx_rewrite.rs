//! ZX-rewriter substrate consumer (P-PRIM-5).
//!
//! Wires the ZX-calculus rewriter into the dispatch path. The
//! optimizer's pattern-simplification pass treats commutative
//! same-color operators in the IR Region tree as Z spiders and
//! identity-folds them via apply_spider_fusion; phase-zero
//! identity ops fold via apply_identity_removal.

use vyre_primitives::zx::{
    apply_color_change as primitive_color_change,
    apply_identity_removal as primitive_identity_removal,
    apply_spider_fusion as primitive_spider_fusion, ZxDiagram,
};
#[cfg(test)]
use vyre_primitives::zx::{ZxColor, ZxSpider};

/// Run spider-fusion to fixpoint, bumping the substrate counter.
#[must_use]
pub fn fuse_diagram(diagram: ZxDiagram) -> ZxDiagram {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    primitive_spider_fusion(diagram)
}

/// Run identity-removal to fixpoint, bumping the substrate counter.
#[must_use]
pub fn remove_identities(diagram: ZxDiagram) -> ZxDiagram {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    primitive_identity_removal(diagram)
}

/// Apply both rewrites  -  fusion, then identity removal  -  to
/// fixpoint. The standard simplification chain.
#[must_use]
pub fn simplify_diagram(diagram: ZxDiagram) -> ZxDiagram {
    let after_fusion = fuse_diagram(diagram);
    remove_identities(after_fusion)
}

/// Hadamard a single spider (color change). Mutates in place; bumps
/// the substrate counter.
pub fn flip_spider(diagram: &mut ZxDiagram, v: u32) {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    primitive_color_change(diagram, v);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z(p: u32) -> ZxSpider {
        ZxSpider {
            color: ZxColor::Z,
            phase_num: p,
        }
    }

    #[test]
    fn fuse_collapses_z_z() {
        let d = ZxDiagram {
            phase_denom: 8,
            spiders: vec![z(1), z(3)],
            edges: vec![(0, 1)],
        };
        let out = fuse_diagram(d);
        assert_eq!(out.spiders.len(), 1);
        assert_eq!(out.spiders[0].phase_num, 4);
    }

    #[test]
    fn remove_identities_drops_phase_zero() {
        let d = ZxDiagram {
            phase_denom: 8,
            spiders: vec![z(1), z(0), z(2)],
            edges: vec![(0, 1), (1, 2)],
        };
        let out = remove_identities(d);
        assert_eq!(out.spiders.len(), 2);
        assert_eq!(out.edges, vec![(0, 1)]);
    }

    #[test]
    fn simplify_chain_fuses_then_removes() {
        // Z(1) - Z(0) - Z(0) - Z(2): chain of 4. After fusion (any
        // pair can fuse first) the chain collapses to one Z-spider
        // with summed phase 3.
        let d = ZxDiagram {
            phase_denom: 8,
            spiders: vec![z(1), z(0), z(0), z(2)],
            edges: vec![(0, 1), (1, 2), (2, 3)],
        };
        let out = simplify_diagram(d);
        assert_eq!(out.spiders.len(), 1);
        assert_eq!(out.spiders[0].phase_num, 3);
    }

    /// Closure-bar: substrate output equals primitive output exactly.
    #[test]
    fn matches_primitive_directly() {
        let d = ZxDiagram {
            phase_denom: 8,
            spiders: vec![z(1), z(2), z(3)],
            edges: vec![(0, 1), (1, 2)],
        };
        let via_substrate = fuse_diagram(d.clone());
        let via_primitive = primitive_spider_fusion(d);
        assert_eq!(via_substrate, via_primitive);
    }

    #[test]
    fn flip_changes_color() {
        let mut d = ZxDiagram {
            phase_denom: 8,
            spiders: vec![z(3)],
            edges: vec![],
        };
        flip_spider(&mut d, 0);
        assert_eq!(d.spiders[0].color, ZxColor::X);
        flip_spider(&mut d, 0);
        assert_eq!(d.spiders[0].color, ZxColor::Z);
    }

    /// Adversarial: empty diagram passes through both rewrites
    /// unchanged.
    #[test]
    fn empty_diagram_unchanged() {
        let d = ZxDiagram {
            phase_denom: 8,
            spiders: vec![],
            edges: vec![],
        };
        let out = simplify_diagram(d.clone());
        assert_eq!(out, d);
    }
}
