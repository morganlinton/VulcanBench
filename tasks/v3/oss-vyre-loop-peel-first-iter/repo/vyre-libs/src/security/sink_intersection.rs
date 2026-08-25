//! `sink_intersection`  -  count how many of a query set are also in
//! a sink-family bitset. Used by rules that want a fractional
//! confidence ("X% of nodes reachable from source landed in sinks").

use std::sync::Arc;

use vyre::ir::model::expr::Ident;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program, UnOp};
use vyre_primitives::graph::csr_forward_traverse::bitset_words;

pub(crate) const OP_ID: &str = "vyre-libs::security::sink_intersection";

/// Build a sink-intersection-count Program. AND query with sink_set,
/// popcount the result, write to out_scalar.
#[must_use]
pub fn sink_intersection(
    node_count: u32,
    query_set: &str,
    sink_set: &str,
    intersect_buf: &str,
    out_scalar: &str,
) -> Program {
    let words = bitset_words(node_count);
    let t = Expr::InvocationId { axis: 0 };
    let body = vec![
        Node::let_bind(
            "intersect",
            Expr::bitand(
                Expr::load(query_set, t.clone()),
                Expr::load(sink_set, t.clone()),
            ),
        ),
        Node::store(intersect_buf, t.clone(), Expr::var("intersect")),
        Node::let_bind(
            "count",
            Expr::UnOp {
                op: UnOp::Popcount,
                operand: Box::new(Expr::var("intersect")),
            },
        ),
        Node::let_bind(
            "_",
            Expr::atomic_add(out_scalar, Expr::u32(0), Expr::var("count")),
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage(query_set, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
            BufferDecl::storage(sink_set, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
            BufferDecl::storage(intersect_buf, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
            BufferDecl::output(out_scalar, 3, DataType::U32).with_count(1),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(t.clone(), Expr::u32(words)),
                body,
            )]),
        }],
    )
}

/// CPU oracle: count of bits set in `query AND sink`.
#[must_use]
#[cfg(test)]
pub(crate) fn cpu_ref(query_set: &[u32], sink_set: &[u32]) -> u32 {
    let inter = vyre_primitives::bitset::and::cpu_ref(query_set, sink_set);
    inter.iter().map(|w| w.count_ones()).sum()
}

/// Soundness marker for [`sink_intersection`].
pub struct SinkIntersection;
impl vyre::soundness::SoundnessTagged for SinkIntersection {
    fn soundness(&self) -> vyre::soundness::Soundness {
        vyre::soundness::Soundness::Exact
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_overlap_counts_all_set_bits() {
        assert_eq!(cpu_ref(&[0b1111], &[0b1111]), 4);
    }

    #[test]
    fn no_overlap_returns_zero() {
        assert_eq!(cpu_ref(&[0b1010], &[0b0101]), 0);
    }

    #[test]
    fn partial_overlap_counts_intersection() {
        assert_eq!(cpu_ref(&[0b1110], &[0b0111]), 2);
    }

    #[test]
    fn distributes_across_words() {
        assert_eq!(cpu_ref(&[0xFF00, 0x00FF], &[0xFFFF, 0xFFFF]), 16);
    }
}
