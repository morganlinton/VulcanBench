//! Quest-style Query-Aware KV Paging.
//!
//! Only the "which pages are critical" decision  -  a pure score-and-select
//! pass. Scoring is `dot(query, page_metadata[p])` for each page; the
//! top-`k` highest-scoring pages are emitted, in descending order, into
//! `io_queue[0..k]`. The remainder of `io_queue` is zero-filled on the
//! first pass so the output is deterministic.
//!
//! Downstream DMA / `AsyncLoad` is the scheduler's job  -  this op only
//! tells the scheduler which pages to fetch.

use crate::region::wrap_anonymous;
use vyre::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_primitives::nn::quest_paging_passes::{
    quest_score_pages_body, quest_select_top_k_body, quest_zero_fill_body,
};

const OP_ID: &str = "vyre-libs::nn::attention::quest_paging";

// target builder / target-text rejects `inf` and NaN literals, so the argmax sentinel
// must be a large-magnitude finite value. `f32::MIN` is the most
// negative finite f32  -  strictly less than every reachable dot-product
// score when `query` and `page_metadata` are finite inputs.
const SCORE_SENTINEL: f32 = f32::MIN;

/// Build a Program that writes the top-`k` page indices (by query
/// similarity) into `io_queue`.
///
/// Buffers:
/// - `query` (ReadOnly, F32, `d_head`)
/// - `page_metadata` (ReadOnly, F32, `num_pages * d_head`)
/// - `scores` (ReadWrite, F32, `num_pages`)  -  per-page dot score scratch
/// - `io_queue` (ReadWrite, U32, `num_pages`)  -  index 0..k holds top-k,
///    rest holds 0
#[must_use]
pub fn quest_paging(
    query: &str,
    page_metadata: &str,
    scores: &str,
    io_queue: &str,
    num_pages: u32,
    k: u32,
    d_head: u32,
) -> Program {
    if num_pages <= 8 && k <= 4 && d_head <= 16 {
        let mut score_exprs = Vec::with_capacity(num_pages as usize);
        for page in 0..num_pages {
            let mut score = Expr::f32(0.0);
            for dim in 0..d_head {
                score = Expr::add(
                    score,
                    Expr::mul(
                        Expr::load(query, Expr::u32(dim)),
                        Expr::load(page_metadata, Expr::u32(page * d_head + dim)),
                    ),
                );
            }
            score_exprs.push(score);
        }

        let mut selected = Vec::<Expr>::with_capacity(k as usize);
        for _rank in 0..k {
            let mut best_score = Expr::f32(SCORE_SENTINEL);
            let mut best_idx = Expr::u32(0);
            for page in 0..num_pages {
                let mut eligible = Expr::bool(true);
                for prior in &selected {
                    eligible = Expr::select(
                        eligible,
                        Expr::ne(Expr::u32(page), prior.clone()),
                        Expr::bool(false),
                    );
                }
                let better = Expr::select(
                    eligible,
                    Expr::gt(score_exprs[page as usize].clone(), best_score.clone()),
                    Expr::bool(false),
                );
                best_score = Expr::select(
                    better.clone(),
                    score_exprs[page as usize].clone(),
                    best_score,
                );
                best_idx = Expr::select(better, Expr::u32(page), best_idx);
            }
            selected.push(best_idx);
        }

        let mut stores = Vec::with_capacity((num_pages * 2) as usize);
        for page in 0..num_pages {
            let mut picked = Expr::bool(false);
            for prior in &selected {
                picked = Expr::select(
                    picked,
                    Expr::bool(true),
                    Expr::eq(Expr::u32(page), prior.clone()),
                );
            }
            stores.push(Node::store(
                scores,
                Expr::u32(page),
                Expr::select(
                    picked,
                    Expr::f32(SCORE_SENTINEL),
                    score_exprs[page as usize].clone(),
                ),
            ));
        }
        for slot in 0..num_pages {
            let value = if slot < k {
                selected[slot as usize].clone()
            } else {
                Expr::u32(0)
            };
            stores.push(Node::store(io_queue, Expr::u32(slot), value));
        }

        return Program::wrapped(
            vec![
                BufferDecl::storage(query, 0, BufferAccess::ReadOnly, DataType::F32)
                    .with_count(d_head),
                BufferDecl::storage(page_metadata, 1, BufferAccess::ReadOnly, DataType::F32)
                    .with_count(num_pages * d_head),
                BufferDecl::storage(scores, 2, BufferAccess::ReadWrite, DataType::F32)
                    .with_count(num_pages),
                BufferDecl::storage(io_queue, 3, BufferAccess::ReadWrite, DataType::U32)
                    .with_count(num_pages),
            ],
            [1, 1, 1],
            vec![wrap_anonymous(
                OP_ID,
                vec![Node::if_then(
                    Expr::eq(Expr::InvocationId { axis: 0 }, Expr::u32(0)),
                    stores,
                )],
            )],
        );
    }
    // Single-invocation serial body so top-k selection is deterministic
    // regardless of backend. `num_pages` is small (typically ≤ 512 in
    // the KV-paging regime) so the O(num_pages · k) top-k is fine.
    let t = Expr::InvocationId { axis: 0 };
    let body = vec![
        // 1. Zero-fill io_queue so unused slots are deterministic.
        Node::Block(quest_zero_fill_body(io_queue, num_pages)),
        // 2. Score every page.
        Node::Block(quest_score_pages_body(
            query,
            page_metadata,
            scores,
            num_pages,
            d_head,
        )),
        Node::barrier(),
        // 3. Select top-k pages by repeated argmax. Each iteration sweeps
        //    `scores`, picks the current maximum, writes its index into
        //    `io_queue[j]`, then marks that slot with SCORE_SENTINEL so the
        //    next iteration skips it.
        Node::Block(vec![Node::if_then(
            Expr::eq(t.clone(), Expr::u32(0)),
            quest_select_top_k_body(scores, io_queue, num_pages, k, SCORE_SENTINEL),
        )]),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(query, 0, BufferAccess::ReadOnly, DataType::F32).with_count(d_head),
            BufferDecl::storage(page_metadata, 1, BufferAccess::ReadOnly, DataType::F32)
                .with_count(num_pages * d_head),
            BufferDecl::storage(scores, 2, BufferAccess::ReadWrite, DataType::F32)
                .with_count(num_pages),
            BufferDecl::storage(io_queue, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(num_pages),
        ],
        [256, 1, 1],
        vec![wrap_anonymous(OP_ID, body)],
    )
}

inventory::submit! {
    crate::harness::OpEntry {
        id: OP_ID,
        build: || quest_paging("q", "meta", "scores", "io", 4, 2, 2),
        test_inputs: Some(|| {
            let to_f32_bytes =
                |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);
            // num_pages=4, d_head=2, k=2.
            // query = [1.0, 0.0]
            // page_metadata[p, d]: page 0=[0, 0], page 1=[1, 0], page 2=[2, 0], page 3=[0.5, 0].
            // scores[p] = dot(query, page_metadata[p]) = page_metadata[p, 0].
            //   → scores = [0.0, 1.0, 2.0, 0.5].
            // Top-2 by descending score → indices [2, 1].
            vec![vec![
                to_f32_bytes(&[1.0, 0.0]),
                to_f32_bytes(&[0.0, 0.0, 1.0, 0.0, 2.0, 0.0, 0.5, 0.0]),
                vec![0u8; 4 * 4],
                vec![0u8; 4 * 4],
            ]]
        }),
        expected_output: Some(|| {
            let to_f32_bytes =
                |w: &[f32]| vyre_primitives::wire::pack_f32_slice(w);

            // scores after selection: only slots that were picked
            // (indices 2 and 1) are overwritten with SCORE_SENTINEL.
            // Indices 0 and 3 retain their pass-1 dot-product scores.
            let scores = [0.0, SCORE_SENTINEL, SCORE_SENTINEL, 0.5];
            // io_queue[0..2] = [2, 1] (top-2 in descending score).
            // io_queue[2..4] = [0, 0] (zero-filled on pass 1).
            let io_queue = [2u32, 1, 0, 0];
            vec![vec![to_f32_bytes(&scores), crate::test_support::byte_pack::u32_bytes(&io_queue)]]
        }),
        category: Some("nn"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::byte_pack::bytes_to_u32 as decode_u32;
    use crate::test_support::byte_pack::decode_f32;
    use crate::test_support::byte_pack::f32_bytes;
    use vyre_reference::value::Value;

    #[test]
    fn quest_paging_nan_in_query_produces_nan_scores() {
        let query = [f32::NAN, 0.0];
        let meta = [0.0f32, 0.0, 1.0, 0.0];
        let program = quest_paging("q", "meta", "scores", "io", 2, 1, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&query)),
                Value::from(f32_bytes(&meta)),
                Value::from(vec![0u8; 8]),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: quest_paging must not panic on NaN query");
        let scores = decode_f32(&outputs[0].to_bytes());
        assert!(
            scores.iter().any(|v| v.is_nan()),
            "quest_paging NaN query must produce at least one NaN score"
        );
    }

    #[test]
    fn quest_paging_zero_pages() {
        let query = [1.0f32, 0.0];
        let program = quest_paging("q", "meta", "scores", "io", 0, 0, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&query)),
                Value::from(vec![]),
                Value::from(vec![]),
                Value::from(vec![]),
            ],
        )
        .expect("Fix: quest_paging num_pages=0 must not panic");
        assert!(outputs[0].to_bytes().is_empty());
        assert!(outputs[1].to_bytes().is_empty());
    }

    #[test]
    fn quest_paging_single_page() {
        let query = [1.0f32, 0.0];
        let meta = [2.0f32, 0.0];
        let program = quest_paging("q", "meta", "scores", "io", 1, 1, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&query)),
                Value::from(f32_bytes(&meta)),
                Value::from(vec![0u8; 4]),
                Value::from(vec![0u8; 4]),
            ],
        )
        .expect("Fix: quest_paging single page must execute");
        let io_queue = decode_u32(&outputs[1].to_bytes());
        assert_eq!(io_queue[0], 0, "single page top-1 must be index 0");
    }

    #[test]
    fn quest_paging_k_zero() {
        let query = [1.0f32, 0.0];
        let meta = [1.0f32, 0.0, 2.0, 0.0];
        let program = quest_paging("q", "meta", "scores", "io", 2, 0, 2);
        let outputs = vyre_reference::reference_eval(
            &program,
            &[
                Value::from(f32_bytes(&query)),
                Value::from(f32_bytes(&meta)),
                Value::from(vec![0u8; 8]),
                Value::from(vec![0u8; 8]),
            ],
        )
        .expect("Fix: quest_paging k=0 must not panic");
        let io_queue = decode_u32(&outputs[1].to_bytes());
        assert_eq!(io_queue, vec![0, 0], "k=0 must zero-fill io_queue");
    }
}
