//! Shared partial dot-product accumulator.
//!
//! This is the reusable inner kernel extracted from attention-style
//! score passes: walk `dk` from `0..d`, load `q[q_base + dk]` and
//! `k[k_base + dk]`, and accumulate the product into `accum_var`.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Stable Tier 2.5 op id for the attention dot-product child region.
pub const OP_ID: &str = "vyre-primitives::math::dot_partial";

/// Emit the `dk` loop that accumulates a partial dot product into `accum_var`.
#[must_use]
pub fn dot_partial(
    q_buffer: &str,
    k_buffer: &str,
    accum_var: &str,
    q_base: Expr,
    k_base: Expr,
    d: u32,
) -> Node {
    if d <= 8 {
        return Node::Block(
            (0..d)
                .map(|lane| {
                    Node::assign(
                        accum_var,
                        Expr::add(
                            Expr::var(accum_var),
                            Expr::mul(
                                Expr::load(q_buffer, Expr::add(q_base.clone(), Expr::u32(lane))),
                                Expr::load(k_buffer, Expr::add(k_base.clone(), Expr::u32(lane))),
                            ),
                        ),
                    )
                })
                .collect(),
        );
    }

    Node::loop_for(
        "dk",
        Expr::u32(0),
        Expr::u32(d),
        vec![Node::assign(
            accum_var,
            Expr::add(
                Expr::var(accum_var),
                Expr::mul(
                    Expr::load(q_buffer, Expr::add(q_base, Expr::var("dk"))),
                    Expr::load(k_buffer, Expr::add(k_base, Expr::var("dk"))),
                ),
            ),
        )],
    )
}

/// Standalone dot-partial Program.
#[must_use]
pub fn dot_partial_program(q_buffer: &str, k_buffer: &str, out: &str, d: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(q_buffer, 0, BufferAccess::ReadOnly, DataType::F32).with_count(d),
            BufferDecl::storage(k_buffer, 1, BufferAccess::ReadOnly, DataType::F32).with_count(d),
            BufferDecl::storage(out, 2, BufferAccess::ReadWrite, DataType::F32).with_count(1),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![
                Node::let_bind("accum", Expr::f32(0.0)),
                dot_partial(q_buffer, k_buffer, "accum", Expr::u32(0), Expr::u32(0), d),
                Node::store(out, Expr::u32(0), Expr::var("accum")),
            ]),
        }],
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || dot_partial_program("q", "k", "out", 2),
        Some(|| {
            let to_f32_bytes = |w: &[f32]| crate::wire::pack_f32_slice(w);
            vec![vec![
                to_f32_bytes(&[2.0, 3.0]),
                to_f32_bytes(&[4.0, 5.0]),
                vec![0u8; 4],
            ]]
        }),
        Some(|| {
            let to_f32_bytes = |w: &[f32]| crate::wire::pack_f32_slice(w);
            vec![vec![to_f32_bytes(&[23.0])]]
        }),
    )
}
