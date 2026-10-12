//! Clifford / geometric product over Cl(2, 0)  -  4-component
//! multivectors `(s, e1, e2, e12)` (scalar, two vector basis elements,
//! pseudoscalar).
//!
//! Cl(2, 0) is the simplest non-trivial Clifford algebra (real plane
//! with no negative-signature dimensions). The geometric product:
//!
//! ```text
//!   (a · b)_s   = a_s b_s + a_1 b_1 + a_2 b_2 - a_12 b_12
//!   (a · b)_1   = a_s b_1 + a_1 b_s - a_2 b_12 + a_12 b_2
//!   (a · b)_2   = a_s b_2 + a_2 b_s + a_1 b_12 - a_12 b_1
//!   (a · b)_12  = a_s b_12 + a_12 b_s + a_1 b_2 - a_2 b_1
//! ```
//!
//! Twelve mul-adds per product. Already GPU-trivial; the moat is
//! that no GPU IR has packaged this as a Tier-2.5 primitive.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | future `vyre-libs::geom::equivariant` | equivariant-by-construction NNs (#33 TFN) |
//! | future `vyre-libs::sim::physics` | rigid-body dynamics, conformal geometry |
//! | future `vyre-libs::vision::3d` | 3D rotations, dual-quaternion poses |
//! | future `vyre-libs::robotics` | screw motions, twists, wrenches |
//!
//! # Higher signatures (future)
//!
//! Cl(3, 0) (4-bit grades, 8-component multivectors) and Cl(3, 1)
//! (relativistic spacetime, 16 components) are next on the roadmap.
//! The pattern (one scalar mul-add per pair of basis elements with a
//! sign rule) generalizes; this file's 4-component implementation is
//! the simplest case that exercises every product structure.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::geom::clifford2_geometric_product";

/// Component count for Cl(2, 0) multivectors.
pub const MV_COMPONENTS: u32 = 4;

/// Emit the geometric-product Program for `n_pairs` independent
/// multivector pairs.
///
/// Inputs:
/// - `lhs`: `n_pairs * 4` u32  -  (s, e1, e2, e12) per pair, 16.16 fp.
/// - `rhs`: `n_pairs * 4` u32  -  same layout.
///
/// Output:
/// - `out`: `n_pairs * 4` u32.
///
/// Each lane handles one full multivector product (12 mul-adds).
#[must_use]
pub fn clifford2_product(lhs: &str, rhs: &str, out: &str, n_pairs: u32) -> Program {
    if n_pairs == 0 {
        return crate::invalid_output_program(
            OP_ID,
            out,
            DataType::U32,
            format!("Fix: clifford2_product requires n_pairs > 0, got {n_pairs}."),
        );
    }

    let t = Expr::InvocationId { axis: 0 };
    let base = Expr::mul(t.clone(), Expr::u32(MV_COMPONENTS));

    let load_l = |off: u32| Expr::load(lhs, Expr::add(base.clone(), Expr::u32(off)));
    let load_r = |off: u32| Expr::load(rhs, Expr::add(base.clone(), Expr::u32(off)));
    let mul_shr = crate::fixed_mul_16_16_expr;

    // out_s = a_s b_s + a_1 b_1 + a_2 b_2 - a_12 b_12
    let out_s = Expr::sub(
        Expr::add(
            Expr::add(mul_shr(load_l(0), load_r(0)), mul_shr(load_l(1), load_r(1))),
            mul_shr(load_l(2), load_r(2)),
        ),
        mul_shr(load_l(3), load_r(3)),
    );
    // out_1 = a_s b_1 + a_1 b_s - a_2 b_12 + a_12 b_2
    let out_1 = Expr::add(
        Expr::sub(
            Expr::add(mul_shr(load_l(0), load_r(1)), mul_shr(load_l(1), load_r(0))),
            mul_shr(load_l(2), load_r(3)),
        ),
        mul_shr(load_l(3), load_r(2)),
    );
    // out_2 = a_s b_2 + a_2 b_s + a_1 b_12 - a_12 b_1
    let out_2 = Expr::sub(
        Expr::add(
            Expr::add(mul_shr(load_l(0), load_r(2)), mul_shr(load_l(2), load_r(0))),
            mul_shr(load_l(1), load_r(3)),
        ),
        mul_shr(load_l(3), load_r(1)),
    );
    // out_12 = a_s b_12 + a_12 b_s + a_1 b_2 - a_2 b_1
    let out_12 = Expr::sub(
        Expr::add(
            Expr::add(mul_shr(load_l(0), load_r(3)), mul_shr(load_l(3), load_r(0))),
            mul_shr(load_l(1), load_r(2)),
        ),
        mul_shr(load_l(2), load_r(1)),
    );

    let store_to =
        |off: u32, val: Expr| Node::store(out, Expr::add(base.clone(), Expr::u32(off)), val);

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(n_pairs)),
        vec![
            store_to(0, out_s),
            store_to(1, out_1),
            store_to(2, out_2),
            store_to(3, out_12),
        ],
    )];

    Program::wrapped(
        vec![
            BufferDecl::storage(lhs, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_pairs * MV_COMPONENTS),
            BufferDecl::storage(rhs, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n_pairs * MV_COMPONENTS),
            BufferDecl::storage(out, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n_pairs * MV_COMPONENTS),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

/// Cl(2,0) multivector as four f64 components.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cl2Mv {
    /// Scalar grade-0 component.
    pub s: f64,
    /// e1 grade-1 vector component.
    pub e1: f64,
    /// e2 grade-1 vector component.
    pub e2: f64,
    /// e12 grade-2 pseudoscalar component.
    pub e12: f64,
}

impl Cl2Mv {
    /// Multiplicative identity (scalar 1).
    pub const IDENTITY: Self = Self {
        s: 1.0,
        e1: 0.0,
        e2: 0.0,
        e12: 0.0,
    };

    /// Pure scalar multivector.
    #[must_use]
    pub const fn scalar(s: f64) -> Self {
        Self {
            s,
            e1: 0.0,
            e2: 0.0,
            e12: 0.0,
        }
    }

    /// Pure vector multivector.
    #[must_use]
    pub const fn vector(x: f64, y: f64) -> Self {
        Self {
            s: 0.0,
            e1: x,
            e2: y,
            e12: 0.0,
        }
    }
}

/// CPU reference for the Cl(2, 0) geometric product.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn clifford2_product_cpu(a: Cl2Mv, b: Cl2Mv) -> Cl2Mv {
    Cl2Mv {
        s: a.s * b.s + a.e1 * b.e1 + a.e2 * b.e2 - a.e12 * b.e12,
        e1: a.s * b.e1 + a.e1 * b.s - a.e2 * b.e12 + a.e12 * b.e2,
        e2: a.s * b.e2 + a.e2 * b.s + a.e1 * b.e12 - a.e12 * b.e1,
        e12: a.s * b.e12 + a.e12 * b.s + a.e1 * b.e2 - a.e2 * b.e1,
    }
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || clifford2_product("lhs", "rhs", "out", 2),
        Some(|| {
            let one = 1u32 << 16;
            vec![vec![
                crate::wire::pack_u32_slice(&[one, 0, 0, 0, one, 0, 0, 0]),
                crate::wire::pack_u32_slice(&[2 * one, 3 * one, 0, 0, 4 * one, 0, 5 * one, 0]),
                crate::wire::pack_u32_slice(&[0; 8]),
            ]]
        }),
        Some(|| {
            let one = 1u32 << 16;
            vec![vec![crate::wire::pack_u32_slice(&[
                2 * one, 3 * one, 0, 0, 4 * one, 0, 5 * one, 0,
            ])]]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-10 * (1.0 + a.abs() + b.abs())
    }

    fn mv_eq(a: Cl2Mv, b: Cl2Mv) -> bool {
        approx_eq(a.s, b.s)
            && approx_eq(a.e1, b.e1)
            && approx_eq(a.e2, b.e2)
            && approx_eq(a.e12, b.e12)
    }

    #[test]
    fn cpu_identity_left_unit() {
        let v = Cl2Mv::vector(2.0, 3.0);
        let out = clifford2_product_cpu(Cl2Mv::IDENTITY, v);
        assert!(mv_eq(out, v));
    }

    #[test]
    fn cpu_identity_right_unit() {
        let v = Cl2Mv::vector(2.0, 3.0);
        let out = clifford2_product_cpu(v, Cl2Mv::IDENTITY);
        assert!(mv_eq(out, v));
    }

    #[test]
    fn cpu_basis_vector_squares_to_one() {
        // e1 · e1 = 1 (Cl(2, 0) signature is (+, +)).
        let e1 = Cl2Mv::vector(1.0, 0.0);
        let out = clifford2_product_cpu(e1, e1);
        assert!(approx_eq(out.s, 1.0));
        assert!(approx_eq(out.e1, 0.0));
        assert!(approx_eq(out.e2, 0.0));
        assert!(approx_eq(out.e12, 0.0));
    }

    #[test]
    fn cpu_e12_squared_is_minus_one() {
        // e12² = -1 for Cl(2, 0), making e12 act like the imaginary
        // unit. Any unit pseudoscalar in any Cl(p, 0) with p even
        // squares to -1.
        let e12 = Cl2Mv {
            s: 0.0,
            e1: 0.0,
            e2: 0.0,
            e12: 1.0,
        };
        let out = clifford2_product_cpu(e12, e12);
        assert!(approx_eq(out.s, -1.0));
    }

    #[test]
    fn cpu_basis_anticommutes() {
        // e1 · e2 = e12;  e2 · e1 = -e12.
        let e1 = Cl2Mv::vector(1.0, 0.0);
        let e2 = Cl2Mv::vector(0.0, 1.0);
        let p1 = clifford2_product_cpu(e1, e2);
        let p2 = clifford2_product_cpu(e2, e1);
        assert!(approx_eq(p1.e12, 1.0));
        assert!(approx_eq(p2.e12, -1.0));
    }

    #[test]
    fn cpu_pseudoscalar_anticommutes_with_vector() {
        // (e12) · e1 = -e2 ;  e1 · (e12) = e2.
        let e1 = Cl2Mv::vector(1.0, 0.0);
        let e12 = Cl2Mv {
            s: 0.0,
            e1: 0.0,
            e2: 0.0,
            e12: 1.0,
        };
        let left = clifford2_product_cpu(e12, e1);
        let right = clifford2_product_cpu(e1, e12);
        assert!(approx_eq(left.e2, -1.0));
        assert!(approx_eq(right.e2, 1.0));
    }

    #[test]
    fn cpu_geometric_product_distributes() {
        // a · (b + c) = a · b + a · c  -  verify with concrete
        // multivectors.
        let a = Cl2Mv {
            s: 1.0,
            e1: 2.0,
            e2: 3.0,
            e12: 4.0,
        };
        let b = Cl2Mv::vector(1.0, 0.0);
        let c = Cl2Mv {
            s: 0.5,
            e1: 0.0,
            e2: 0.5,
            e12: 0.0,
        };
        let bc = Cl2Mv {
            s: b.s + c.s,
            e1: b.e1 + c.e1,
            e2: b.e2 + c.e2,
            e12: b.e12 + c.e12,
        };
        let lhs = clifford2_product_cpu(a, bc);
        let ab = clifford2_product_cpu(a, b);
        let ac = clifford2_product_cpu(a, c);
        let rhs = Cl2Mv {
            s: ab.s + ac.s,
            e1: ab.e1 + ac.e1,
            e2: ab.e2 + ac.e2,
            e12: ab.e12 + ac.e12,
        };
        assert!(mv_eq(lhs, rhs));
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = clifford2_product("a", "b", "out", 8);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["a", "b", "out"]);
        for buf in p.buffers.iter() {
            assert_eq!(buf.count(), 32); // 8 pairs * 4 components
        }
    }

    #[test]
    fn zero_n_pairs_traps() {
        let p = clifford2_product("a", "b", "out", 0);
        assert!(p.stats().trap());
    }
}
