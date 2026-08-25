//! Adjoint-pair detection primitive (P-PRIM-17).
//!
//! Two functors `F: C → D` and `G: D → C` form an adjoint pair
//! `F ⊣ G` iff there is a natural bijection
//! `Hom_D(F(c), d) ≅ Hom_C(c, G(d))` for all c ∈ C, d ∈ D.
//!
//! For finite categories this reduces to a Hom-set cardinality
//! equality on every (c, d) pair: `|Hom_D(F(c), d)| == |Hom_C(c, G(d))|`.
//! The substrate primitive does that pointwise check.

extern crate alloc;
use alloc::vec::Vec;

use super::yoneda::FiniteCategory;

/// Functor between finite categories: object map only. Hom-set
/// preservation is implied by the Hom-cardinality tables on the
/// source/target categories.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiniteFunctor {
    /// `object_map[c] = F(c)` for c < domain.n.
    pub object_map: Vec<u32>,
}

impl FiniteFunctor {
    /// Identity functor on an n-object category.
    #[must_use]
    pub fn identity(n: u32) -> Self {
        Self {
            object_map: (0..n).collect(),
        }
    }

    /// Apply F to an object.
    ///
    #[must_use]
    pub fn apply(&self, c: u32) -> u32 {
        self.object_map.get(c as usize).copied().unwrap_or(u32::MAX)
    }
}

/// Result of an adjoint-pair check, exposing per-pair witness for
/// debugging when the check fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdjointPair {
    /// Whether the bijection holds at every `(c, d)`.
    pub is_adjoint: bool,
    /// First failing `(c, d)` pair if `is_adjoint` is false.
    pub witness: Option<(u32, u32)>,
}

/// Check `F ⊣ G` on finite categories `C, D`.
///
/// Returns [`AdjointPair`] with `is_adjoint = true` iff
/// `|Hom_D(F(c), d)| == |Hom_C(c, G(d))|` for every (c, d).
/// On failure, `witness` holds the first counterexample pair.
///
#[must_use]
pub fn is_adjoint_pair(
    c_cat: &FiniteCategory,
    d_cat: &FiniteCategory,
    f: &FiniteFunctor,
    g: &FiniteFunctor,
) -> AdjointPair {
    if f.object_map.len() as u32 != c_cat.n || g.object_map.len() as u32 != d_cat.n {
        return AdjointPair {
            is_adjoint: false,
            witness: Some((0, 0)),
        };
    }

    for c in 0..c_cat.n {
        for d in 0..d_cat.n {
            let lhs = d_cat.hom(f.apply(c), d);
            let rhs = c_cat.hom(c, g.apply(d));
            if lhs != rhs {
                return AdjointPair {
                    is_adjoint: false,
                    witness: Some((c, d)),
                };
            }
        }
    }
    AdjointPair {
        is_adjoint: true,
        witness: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_self_adjoint_on_discrete() {
        let cat = FiniteCategory::discrete(3);
        let id = FiniteFunctor::identity(3);
        let result = is_adjoint_pair(&cat, &cat, &id, &id);
        assert!(result.is_adjoint);
        assert!(result.witness.is_none());
    }

    /// Closure-bar: when F and G witness an adjunction the witness
    /// field stays None; when they don't it pins the failure.
    #[test]
    fn non_adjoint_pinpoints_failure() {
        // C, D both 2-object discrete. F maps both to 0; G is identity.
        // |Hom_D(F(0), 1)| = |Hom_D(0, 1)| = 0
        // |Hom_C(0, G(1))| = |Hom_C(0, 1)| = 0  (matches by accident)
        // But |Hom_D(F(1), 1)| = |Hom_D(0, 1)| = 0
        //     |Hom_C(1, G(1))| = |Hom_C(1, 1)| = 1  ← mismatch
        let cat = FiniteCategory::discrete(2);
        let f = FiniteFunctor {
            object_map: alloc::vec![0, 0],
        };
        let g = FiniteFunctor::identity(2);
        let result = is_adjoint_pair(&cat, &cat, &f, &g);
        assert!(!result.is_adjoint);
        assert!(result.witness.is_some());
    }

    /// Adversarial: adjunction must hold on every (c, d), not just
    /// some.
    #[test]
    fn partial_adjunction_rejected() {
        // 2-object discrete. F=id, G swap.
        let cat = FiniteCategory::discrete(2);
        let f = FiniteFunctor::identity(2);
        let g = FiniteFunctor {
            object_map: alloc::vec![1, 0],
        };
        let result = is_adjoint_pair(&cat, &cat, &f, &g);
        assert!(!result.is_adjoint);
    }

    /// Idempotence: id ⊣ id always holds for any discrete category.
    #[test]
    fn identity_adjoint_pair_for_any_size() {
        for n in [1u32, 2, 4, 8] {
            let cat = FiniteCategory::discrete(n);
            let id = FiniteFunctor::identity(n);
            assert!(is_adjoint_pair(&cat, &cat, &id, &id).is_adjoint);
        }
    }
}
