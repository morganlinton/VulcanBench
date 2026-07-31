//! Categorical-substrate consumer (P-PRIM-16/17/18).
//!
//! Wires Yoneda-embedding cardinality, adjoint-pair detection, and
//! Kan extension into the dispatch path. The pass scheduler's
//! functorial_pass_composition consumer reaches for these when
//! reasoning about pass-tree equivalences (when does `F ⊣ G` imply
//! the optimizer can re-order F-then-G into G-then-F?).

use vyre_primitives::cat::adjoint::{
    is_adjoint_pair as primitive_is_adjoint, AdjointPair, FiniteFunctor,
};
use vyre_primitives::cat::kan_extension::{
    kan_extension_left as primitive_lan, kan_extension_right as primitive_ran,
};
use vyre_primitives::cat::yoneda::{yoneda_natural_iso as primitive_yoneda, FiniteCategory};

/// Cardinality of `Nat(Hom(-, x), F)` via Yoneda. Bumps the
/// dataflow-fixpoint substrate counter.
#[must_use]
pub fn natural_transformation_count(category: &FiniteCategory, x: u32, f_at_x: u32) -> u32 {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    primitive_yoneda(category, x, f_at_x)
}

/// Decide whether `F ⊣ G` on the given finite categories. Bumps
/// the dataflow-fixpoint substrate counter.
#[must_use]
pub fn check_adjunction(
    c_cat: &FiniteCategory,
    d_cat: &FiniteCategory,
    f: &FiniteFunctor,
    g: &FiniteFunctor,
) -> AdjointPair {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    primitive_is_adjoint(c_cat, d_cat, f, g)
}

/// Left Kan extension at `c`: sum of F over `K^{-1}(c)`. Bumps
/// the dataflow-fixpoint substrate counter.
#[must_use]
pub fn left_kan_at(k: &FiniteFunctor, f_image: &[u32], c: u32) -> u32 {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    primitive_lan(k, f_image, c)
}

/// Right Kan extension at `c`: product of F over `K^{-1}(c)`.
/// Bumps the dataflow-fixpoint substrate counter.
#[must_use]
pub fn right_kan_at(k: &FiniteFunctor, f_image: &[u32], c: u32) -> u32 {
    use crate::observability::{bump, dataflow_fixpoint_calls};
    bump(&dataflow_fixpoint_calls);
    primitive_ran(k, f_image, c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yoneda_iso_equals_f_image() {
        let cat = FiniteCategory::discrete(3);
        for f_at_x in [0u32, 1, 5, 100] {
            assert_eq!(natural_transformation_count(&cat, 1, 0), 0u32);
            assert_eq!(natural_transformation_count(&cat, 1, f_at_x), f_at_x);
        }
    }

    #[test]
    fn identity_self_adjoint() {
        let cat = FiniteCategory::discrete(3);
        let id = FiniteFunctor::identity(3);
        let result = check_adjunction(&cat, &cat, &id, &id);
        assert!(result.is_adjoint);
    }

    #[test]
    fn left_kan_sums_preimage() {
        let k = FiniteFunctor {
            object_map: vec![0, 0, 1],
        };
        let f = vec![3u32, 5, 7];
        assert_eq!(left_kan_at(&k, &f, 0), 8);
        assert_eq!(left_kan_at(&k, &f, 1), 7);
    }

    #[test]
    fn right_kan_multiplies_preimage() {
        let k = FiniteFunctor {
            object_map: vec![0, 0, 1],
        };
        let f = vec![3u32, 5, 7];
        assert_eq!(right_kan_at(&k, &f, 0), 15);
        assert_eq!(right_kan_at(&k, &f, 1), 7);
    }

    /// Closure-bar: substrate output equals primitive output for
    /// each of the three categorical primitives.
    #[test]
    fn matches_primitive_directly() {
        let cat = FiniteCategory::discrete(3);
        let id = FiniteFunctor::identity(3);
        let k = FiniteFunctor {
            object_map: vec![0, 1, 0],
        };
        let f = vec![2u32, 4, 6];

        assert_eq!(
            natural_transformation_count(&cat, 0, 5),
            primitive_yoneda(&cat, 0, 5)
        );
        assert_eq!(
            check_adjunction(&cat, &cat, &id, &id),
            primitive_is_adjoint(&cat, &cat, &id, &id)
        );
        for c in 0..2u32 {
            assert_eq!(left_kan_at(&k, &f, c), primitive_lan(&k, &f, c));
            assert_eq!(right_kan_at(&k, &f, c), primitive_ran(&k, &f, c));
        }
    }

    /// Adversarial: empty Kan-preimage gives Lan = 0, Ran = 1.
    #[test]
    fn empty_preimage_initial_terminal_distinction() {
        let k = FiniteFunctor {
            object_map: vec![0, 0],
        };
        let f = vec![3u32, 5];
        assert_eq!(left_kan_at(&k, &f, 1), 0, "Lan over empty preimage = 0");
        assert_eq!(right_kan_at(&k, &f, 1), 1, "Ran over empty preimage = 1");
    }

    /// Adversarial: non-adjoint pair must report a witness.
    #[test]
    fn non_adjoint_pair_reports_witness() {
        let cat = FiniteCategory::discrete(2);
        let f = FiniteFunctor {
            object_map: vec![0, 0],
        };
        let g = FiniteFunctor::identity(2);
        let result = check_adjunction(&cat, &cat, &f, &g);
        assert!(!result.is_adjoint);
        assert!(result.witness.is_some());
    }
}
