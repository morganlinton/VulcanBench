//! Reusable Quest-style KV paging passes.
//!
//! These are Tier 2.5 building blocks: each pass is usable as a
//! standalone `Program`, and higher-level attention compositions can
//! wrap the same bodies with `source_region` metadata instead of
//! hiding multi-phase work in one monolithic op.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Stable op id for deterministic queue zero-fill.
pub const QUEST_ZERO_FILL_OP_ID: &str = "vyre-primitives::nn::quest_zero_fill";
/// Stable op id for query/page dot-product scoring.
pub const QUEST_SCORE_PAGES_OP_ID: &str = "vyre-primitives::nn::quest_score_pages";
/// Stable op id for deterministic top-k page selection.
pub const QUEST_SELECT_TOP_K_OP_ID: &str = "vyre-primitives::nn::quest_select_top_k";

/// Emit the body that zero-fills the full page queue.
#[must_use]
pub fn quest_zero_fill_body(io_queue: &str, num_pages: u32) -> Vec<Node> {
    let t = Expr::InvocationId { axis: 0 };
    vec![Node::loop_for(
        "loop_idx",
        Expr::u32(0),
        Expr::div(
            Expr::add(Expr::u32(num_pages), Expr::u32(255)),
            Expr::u32(256),
        ),
        vec![
            Node::let_bind(
                "z",
                Expr::add(Expr::mul(Expr::var("loop_idx"), Expr::u32(256)), t.clone()),
            ),
            Node::if_then(
                Expr::lt(Expr::var("z"), Expr::u32(num_pages)),
                vec![Node::store(io_queue, Expr::var("z"), Expr::u32(0))],
            ),
        ],
    )]
}

/// Standalone zero-fill Program.
#[must_use]
pub fn quest_zero_fill(io_queue: &str, num_pages: u32) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(io_queue, 0, BufferAccess::ReadWrite, DataType::U32)
                .with_count(num_pages),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(QUEST_ZERO_FILL_OP_ID),
            source_region: None,
            body: Arc::new(quest_zero_fill_body(io_queue, num_pages)),
        }],
    )
}

/// Emit the body that computes `scores[p] = dot(query, page_metadata[p])`.
#[must_use]
pub fn quest_score_pages_body(
    query: &str,
    page_metadata: &str,
    scores: &str,
    num_pages: u32,
    d_head: u32,
) -> Vec<Node> {
    let t = Expr::InvocationId { axis: 0 };
    let score_body = if d_head <= 8 {
        (0..d_head)
            .map(|lane| {
                Node::assign(
                    "score",
                    Expr::add(
                        Expr::var("score"),
                        Expr::mul(
                            Expr::load(query, Expr::u32(lane)),
                            Expr::load(
                                page_metadata,
                                Expr::add(
                                    Expr::mul(Expr::var("p"), Expr::u32(d_head)),
                                    Expr::u32(lane),
                                ),
                            ),
                        ),
                    ),
                )
            })
            .collect()
    } else {
        vec![Node::loop_for(
            "d",
            Expr::u32(0),
            Expr::u32(d_head),
            vec![Node::assign(
                "score",
                Expr::add(
                    Expr::var("score"),
                    Expr::mul(
                        Expr::load(query, Expr::var("d")),
                        Expr::load(
                            page_metadata,
                            Expr::add(Expr::mul(Expr::var("p"), Expr::u32(d_head)), Expr::var("d")),
                        ),
                    ),
                ),
            )],
        )]
    };
    vec![Node::loop_for(
        "loop_idx",
        Expr::u32(0),
        Expr::div(
            Expr::add(Expr::u32(num_pages), Expr::u32(255)),
            Expr::u32(256),
        ),
        vec![
            Node::let_bind(
                "p",
                Expr::add(Expr::mul(Expr::var("loop_idx"), Expr::u32(256)), t.clone()),
            ),
            Node::if_then(Expr::lt(Expr::var("p"), Expr::u32(num_pages)), {
                let mut body = Vec::with_capacity(score_body.len() + 2);
                body.push(Node::let_bind("score", Expr::f32(0.0)));
                body.extend(score_body.clone());
                body.push(Node::store(scores, Expr::var("p"), Expr::var("score")));
                body
            }),
        ],
    )]
}

/// Standalone scoring Program.
#[must_use]
pub fn quest_score_pages(
    query: &str,
    page_metadata: &str,
    scores: &str,
    num_pages: u32,
    d_head: u32,
) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(query, 0, BufferAccess::ReadOnly, DataType::F32).with_count(d_head),
            BufferDecl::storage(page_metadata, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(num_pages.saturating_mul(d_head)),
            BufferDecl::storage(scores, 2, BufferAccess::ReadWrite, DataType::F32)
                .with_count(num_pages),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(QUEST_SCORE_PAGES_OP_ID),
            source_region: None,
            body: Arc::new(quest_score_pages_body(
                query,
                page_metadata,
                scores,
                num_pages,
                d_head,
            )),
        }],
    )
}

/// Emit the deterministic repeated-argmax top-k body.
#[must_use]
pub fn quest_select_top_k_body(
    scores: &str,
    io_queue: &str,
    num_pages: u32,
    k: u32,
    score_sentinel: f32,
) -> Vec<Node> {
    vec![Node::loop_for(
        "j",
        Expr::u32(0),
        Expr::u32(k),
        vec![
            Node::let_bind("best_idx", Expr::u32(0)),
            Node::let_bind("best_score", Expr::f32(score_sentinel)),
            Node::loop_for(
                "q",
                Expr::u32(0),
                Expr::u32(num_pages),
                vec![
                    Node::let_bind("cand", Expr::load(scores, Expr::var("q"))),
                    Node::if_then(
                        Expr::gt(Expr::var("cand"), Expr::var("best_score")),
                        vec![
                            Node::assign("best_score", Expr::var("cand")),
                            Node::assign("best_idx", Expr::var("q")),
                        ],
                    ),
                ],
            ),
            Node::store(io_queue, Expr::var("j"), Expr::var("best_idx")),
            Node::store(scores, Expr::var("best_idx"), Expr::f32(score_sentinel)),
        ],
    )]
}

/// Standalone top-k Program.
#[must_use]
pub fn quest_select_top_k(
    scores: &str,
    io_queue: &str,
    num_pages: u32,
    k: u32,
    score_sentinel: f32,
) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(scores, 0, BufferAccess::ReadWrite, DataType::F32)
                .with_count(num_pages),
            BufferDecl::storage(io_queue, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(num_pages),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(QUEST_SELECT_TOP_K_OP_ID),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
                quest_select_top_k_body(scores, io_queue, num_pages, k, score_sentinel),
            )]),
        }],
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        QUEST_ZERO_FILL_OP_ID,
        || quest_zero_fill("io", 4),
        Some(|| {
            vec![vec![vec![0xFF; 4 * 4]]]
        }),
        Some(|| {
            vec![vec![vec![0u8; 4 * 4]]]
        }),
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        QUEST_SCORE_PAGES_OP_ID,
        || quest_score_pages("q", "meta", "scores", 4, 2),
        Some(|| {
            let to_f32_bytes = |w: &[f32]| crate::wire::pack_f32_slice(w);
            vec![vec![
                to_f32_bytes(&[1.0, 0.0]),
                to_f32_bytes(&[0.0, 0.0, 1.0, 0.0, 2.0, 0.0, 0.5, 0.0]),
                vec![0u8; 4 * 4],
            ]]
        }),
        Some(|| {
            let to_f32_bytes = |w: &[f32]| crate::wire::pack_f32_slice(w);
            vec![vec![to_f32_bytes(&[0.0, 1.0, 2.0, 0.5])]]
        }),
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        QUEST_SELECT_TOP_K_OP_ID,
        || quest_select_top_k("scores", "io", 4, 1, -1.0),
        Some(|| {
            let to_f32_bytes = |w: &[f32]| crate::wire::pack_f32_slice(w);
            let to_u32_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_f32_bytes(&[0.0, 1.0, 2.0, 0.5]),
                to_u32_bytes(&[0, 0, 0, 0]),
            ]]
        }),
        Some(|| {
            let to_f32_bytes = |w: &[f32]| crate::wire::pack_f32_slice(w);
            let to_u32_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_f32_bytes(&[0.0, 1.0, -1.0, 0.5]),
                to_u32_bytes(&[2, 0, 0, 0]),
            ]]
        }),
    )
}
