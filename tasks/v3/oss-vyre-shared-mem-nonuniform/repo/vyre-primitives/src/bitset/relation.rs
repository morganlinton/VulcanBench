//! Shared packed-bitset scalar relation reductions.
//!
//! Relation ops all scan `lhs` and `rhs` word-wise, reduce each
//! per-word predicate into `out_scalar[0]` with atomic AND, and differ
//! only in the predicate they apply per word.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::MemoryOrdering;

const WORKGROUP_SIZE: u32 = 256;

/// Supported bitset-wide scalar relations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BitsetRelation {
    /// Every word must match exactly.
    Equal,
    /// Every `lhs` bit must also be present in `rhs`.
    SubsetOf,
}

impl BitsetRelation {
    fn predicate(self, lhs_word: Expr, rhs_word: Expr) -> Expr {
        match self {
            Self::Equal => Expr::eq(lhs_word, rhs_word),
            Self::SubsetOf => {
                Expr::eq(Expr::bitand(lhs_word, Expr::bitnot(rhs_word)), Expr::u32(0))
            }
        }
    }
}

/// Build `out_scalar[0] = forall w: relation(lhs[w], rhs[w])`.
#[must_use]
pub(crate) fn bitset_relation_program(
    op_id: &'static str,
    lhs: &str,
    rhs: &str,
    out_scalar: &str,
    words: u32,
    relation: BitsetRelation,
) -> Program {
    let lane = Expr::InvocationId { axis: 0 };
    let chunk_count = Expr::div(
        Expr::add(Expr::u32(words), Expr::u32(WORKGROUP_SIZE - 1)),
        Expr::u32(WORKGROUP_SIZE),
    );
    let predicate = relation.predicate(
        Expr::load(lhs, Expr::var("w")),
        Expr::load(rhs, Expr::var("w")),
    );
    let body = vec![
        Node::if_then(
            Expr::eq(lane.clone(), Expr::u32(0)),
            vec![Node::store(out_scalar, Expr::u32(0), Expr::u32(1))],
        ),
        Node::Barrier {
            ordering: MemoryOrdering::SeqCst,
        },
        Node::loop_for(
            "chunk",
            Expr::u32(0),
            chunk_count,
            vec![
                Node::let_bind(
                    "w",
                    Expr::add(
                        Expr::mul(Expr::var("chunk"), Expr::u32(WORKGROUP_SIZE)),
                        lane.clone(),
                    ),
                ),
                Node::if_then(
                    Expr::lt(Expr::var("w"), Expr::u32(words)),
                    vec![Node::let_bind(
                        "_relation_prev",
                        Expr::atomic_and(
                            out_scalar,
                            Expr::u32(0),
                            Expr::select(predicate, Expr::u32(1), Expr::u32(0)),
                        ),
                    )],
                ),
            ],
        ),
    ];
    Program::wrapped(
        vec![
            BufferDecl::storage(lhs, 0, BufferAccess::ReadOnly, DataType::U32).with_count(words),
            BufferDecl::storage(rhs, 1, BufferAccess::ReadOnly, DataType::U32).with_count(words),
            BufferDecl::storage(out_scalar, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [WORKGROUP_SIZE, 1, 1],
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}
