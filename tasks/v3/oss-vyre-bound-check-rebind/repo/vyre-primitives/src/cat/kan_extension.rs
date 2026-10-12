//! Kan extension primitive (P-PRIM-18).
//!
//! For a functor `K: M → C` and `F: M → Set`, the **left Kan extension**
//! `Lan_K F: C → Set` is the universal natural extension along K.
//! At the substrate (finite, set-valued) level the value at a target
//! object `c ∈ C` reduces to a colimit / sum:
//!
//! ```text
//! (Lan_K F)(c) = ∑_{m : K(m) = c} F(m)
//! ```
//!
//! The **right Kan extension** is the dual product:
//!
//! ```text
//! (Ran_K F)(c) = ∏_{m : K(m) = c} F(m)
//! ```
//!
//! For the optimizer / pass-composition substrate this gives a
//! pointwise way to extend a partially-defined functor along a
//! re-indexing functor without materializing the full diagram.

extern crate alloc;
use alloc::vec::Vec;

use super::adjoint::FiniteFunctor;

/// Left Kan extension at one object: sum of `F(m)` over the
/// preimage `K^{-1}(c)`. Returns 0 when no `m` maps to `c` (the
/// initial-set semantics for an empty colimit).
#[must_use]
pub fn kan_extension_left(k: &FiniteFunctor, f_image: &[u32], c: u32) -> u32 {
    debug_assert_eq!(k.object_map.len(), f_image.len());
    let mut acc: u32 = 0;
    for (m, &kc) in k.object_map.iter().enumerate() {
        if kc == c {
            acc = acc.saturating_add(f_image[m]);
        }
    }
    acc
}

/// Right Kan extension at one object: product of `F(m)` over the
/// preimage `K^{-1}(c)`. Returns 1 when no `m` maps to `c` (the
/// terminal-set semantics for an empty limit).
#[must_use]
pub fn kan_extension_right(k: &FiniteFunctor, f_image: &[u32], c: u32) -> u32 {
    debug_assert_eq!(k.object_map.len(), f_image.len());
    let mut acc: u32 = 1;
    for (m, &kc) in k.object_map.iter().enumerate() {
        if kc == c {
            acc = acc.saturating_mul(f_image[m]);
        }
    }
    acc
}

/// Vector of Lan_K F over every object in the codomain of size `c_n`.
#[must_use]
pub fn kan_extension_left_table(k: &FiniteFunctor, f_image: &[u32], c_n: u32) -> Vec<u32> {
    (0..c_n)
        .map(|c| kan_extension_left(k, f_image, c))
        .collect()
}

/// Vector of Ran_K F over every object in the codomain of size `c_n`.
#[must_use]
pub fn kan_extension_right_table(k: &FiniteFunctor, f_image: &[u32], c_n: u32) -> Vec<u32> {
    (0..c_n)
        .map(|c| kan_extension_right(k, f_image, c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lan_at_unmapped_object_is_zero() {
        // K maps both M-objects to C-object 0; ask for Lan at C-object 1.
        let k = FiniteFunctor {
            object_map: alloc::vec![0, 0],
        };
        let f = alloc::vec![3u32, 5];
        assert_eq!(kan_extension_left(&k, &f, 1), 0);
    }

    #[test]
    fn lan_sums_preimage() {
        // K(0)=0, K(1)=0, K(2)=1. F = [3, 5, 7].
        // Lan at 0: F(0) + F(1) = 8. Lan at 1: F(2) = 7.
        let k = FiniteFunctor {
            object_map: alloc::vec![0, 0, 1],
        };
        let f = alloc::vec![3u32, 5, 7];
        assert_eq!(kan_extension_left(&k, &f, 0), 8);
        assert_eq!(kan_extension_left(&k, &f, 1), 7);
    }

    #[test]
    fn ran_at_unmapped_object_is_one() {
        let k = FiniteFunctor {
            object_map: alloc::vec![0, 0],
        };
        let f = alloc::vec![3u32, 5];
        assert_eq!(kan_extension_right(&k, &f, 1), 1);
    }

    #[test]
    fn ran_multiplies_preimage() {
        // K(0)=0, K(1)=0, K(2)=1. F = [3, 5, 7].
        // Ran at 0: F(0) * F(1) = 15. Ran at 1: F(2) = 7.
        let k = FiniteFunctor {
            object_map: alloc::vec![0, 0, 1],
        };
        let f = alloc::vec![3u32, 5, 7];
        assert_eq!(kan_extension_right(&k, &f, 0), 15);
        assert_eq!(kan_extension_right(&k, &f, 1), 7);
    }

    /// Closure-bar: table form must agree with pointwise calls.
    #[test]
    fn table_matches_pointwise() {
        let k = FiniteFunctor {
            object_map: alloc::vec![0, 1, 0, 2, 1],
        };
        let f = alloc::vec![1u32, 2, 3, 4, 5];
        let lan_table = kan_extension_left_table(&k, &f, 3);
        let ran_table = kan_extension_right_table(&k, &f, 3);
        for c in 0..3u32 {
            assert_eq!(lan_table[c as usize], kan_extension_left(&k, &f, c));
            assert_eq!(ran_table[c as usize], kan_extension_right(&k, &f, c));
        }
    }

    /// Adversarial: identity functor's Lan/Ran reduces to F itself.
    #[test]
    fn identity_kan_extension_is_f() {
        let k = FiniteFunctor::identity(4);
        let f = alloc::vec![2u32, 3, 5, 7];
        for c in 0..4u32 {
            assert_eq!(kan_extension_left(&k, &f, c), f[c as usize]);
            assert_eq!(kan_extension_right(&k, &f, c), f[c as usize]);
        }
    }

    /// Adversarial: saturating arithmetic prevents overflow on
    /// pathological inputs.
    #[test]
    fn saturating_protects_overflow() {
        let k = FiniteFunctor {
            object_map: alloc::vec![0, 0, 0],
        };
        let f = alloc::vec![u32::MAX, u32::MAX, u32::MAX];
        // Sum saturates instead of wrapping.
        assert_eq!(kan_extension_left(&k, &f, 0), u32::MAX);
        // Product likewise.
        assert_eq!(kan_extension_right(&k, &f, 0), u32::MAX);
    }
}
