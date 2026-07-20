//! Self-substrate scheduling wrappers for NN attention and KV paging passes.
//!
//! The primitive crate owns the executable IR. This module names the
//! self-substrate use sites that compose those primitives into inference-path
//! dispatch stages.

use vyre_foundation::ir::{Expr, Node, Program};
use vyre_primitives::nn::{
    attention_passes::{
        attention_max_pass, attention_max_pass_program, attention_sum_pass,
        attention_sum_pass_program, attention_write_pass, attention_write_pass_program,
        AttentionWritePassProgramSpec,
    },
    quest_paging_passes::{
        quest_score_pages, quest_score_pages_body, quest_select_top_k, quest_select_top_k_body,
        quest_zero_fill, quest_zero_fill_body,
    },
};

/// Emit the reusable max-score body for a single query row.
#[must_use]
pub fn attention_row_max_body(q: &str, k: &str, d: u32, s: u32) -> Vec<Node> {
    attention_max_pass(q, k, d, s, Expr::f32(1.0f32 / (d as f32).sqrt()))
}

/// Build the standalone max-score pass used by self-hosted attention planning.
#[must_use]
pub fn dispatch_attention_max_pass(q: &str, k: &str, out: &str, s: u32, d: u32) -> Program {
    attention_max_pass_program(q, k, out, s, d)
}

/// Emit the reusable normalization-sum body for a single query row.
#[must_use]
pub fn attention_row_sum_body(q: &str, k: &str, d: u32, s: u32) -> Vec<Node> {
    attention_sum_pass(q, k, d, s, Expr::f32(1.0f32 / (d as f32).sqrt()))
}

/// Build the standalone normalization-sum pass used by self-hosted attention planning.
#[must_use]
pub fn dispatch_attention_sum_pass(
    q: &str,
    k: &str,
    max_in: &str,
    out: &str,
    s: u32,
    d: u32,
) -> Program {
    attention_sum_pass_program(q, k, max_in, out, s, d)
}

/// Emit the reusable weighted-value body for a single query row.
#[must_use]
pub fn attention_row_write_body(q: &str, k: &str, v: &str, d: u32, s: u32, out: &str) -> Vec<Node> {
    attention_write_pass(q, k, v, d, s, Expr::f32(1.0f32 / (d as f32).sqrt()), out)
}

/// Build the standalone weighted-value write pass.
#[must_use]
pub fn dispatch_attention_write_pass(spec: AttentionWritePassProgramSpec<'_>) -> Program {
    attention_write_pass_program(spec)
}

/// Emit the reusable QUEST zero-fill body.
#[must_use]
pub fn quest_page_queue_zero_body(io_queue: &str, num_pages: u32) -> Vec<Node> {
    quest_zero_fill_body(io_queue, num_pages)
}

/// Build the standalone QUEST zero-fill pass.
#[must_use]
pub fn dispatch_quest_zero_fill(io_queue: &str, num_pages: u32) -> Program {
    quest_zero_fill(io_queue, num_pages)
}

/// Emit the reusable QUEST page-scoring body.
#[must_use]
pub fn quest_page_score_body(
    query: &str,
    page_metadata: &str,
    scores: &str,
    num_pages: u32,
    d_head: u32,
) -> Vec<Node> {
    quest_score_pages_body(query, page_metadata, scores, num_pages, d_head)
}

/// Build the standalone QUEST page-scoring pass.
#[must_use]
pub fn dispatch_quest_score_pages(
    query: &str,
    page_metadata: &str,
    scores: &str,
    num_pages: u32,
    d_head: u32,
) -> Program {
    quest_score_pages(query, page_metadata, scores, num_pages, d_head)
}

/// Emit the reusable QUEST top-k selection body.
#[must_use]
pub fn quest_page_select_top_k_body(
    scores: &str,
    io_queue: &str,
    num_pages: u32,
    k: u32,
    score_sentinel: f32,
) -> Vec<Node> {
    quest_select_top_k_body(scores, io_queue, num_pages, k, score_sentinel)
}

/// Build the standalone QUEST top-k selection pass.
#[must_use]
pub fn dispatch_quest_select_top_k(
    scores: &str,
    io_queue: &str,
    num_pages: u32,
    k: u32,
    score_sentinel: f32,
) -> Program {
    quest_select_top_k(scores, io_queue, num_pages, k, score_sentinel)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program_generator(program: &Program) -> &str {
        let Some(Node::Region { generator, .. }) = program.entry.first() else {
            panic!("Fix: NN self-substrate Program must start with a Region.");
        };
        generator.as_str()
    }

    #[test]
    fn attention_programs_emit_expected_primitives() {
        assert_eq!(
            program_generator(&dispatch_attention_max_pass("q", "k", "max", 4, 2)),
            "vyre-primitives::nn::attention_max_pass"
        );
        assert_eq!(
            program_generator(&dispatch_attention_sum_pass("q", "k", "max", "sum", 4, 2)),
            "vyre-primitives::nn::attention_sum_pass"
        );
        assert_eq!(
            program_generator(&dispatch_attention_write_pass(
                AttentionWritePassProgramSpec {
                    q: "q",
                    k: "k",
                    v: "v",
                    max_in: "max",
                    sum_in: "sum",
                    out: "out",
                    s: 4,
                    d: 2,
                }
            )),
            "vyre-primitives::nn::attention_write_pass"
        );
    }

    #[test]
    fn attention_bodies_are_composable_ir_blocks() {
        assert_ne!(attention_row_max_body("q", "k", 2, 4).len(), 0);
        assert_ne!(attention_row_sum_body("q", "k", 2, 4).len(), 0);
        assert_ne!(
            attention_row_write_body("q", "k", "v", 2, 4, "out").len(),
            0
        );
    }

    #[test]
    fn quest_programs_emit_expected_primitives() {
        assert_eq!(
            program_generator(&dispatch_quest_zero_fill("queue", 16)),
            "vyre-primitives::nn::quest_zero_fill"
        );
        assert_eq!(
            program_generator(&dispatch_quest_score_pages(
                "query", "pages", "scores", 16, 4
            )),
            "vyre-primitives::nn::quest_score_pages"
        );
        assert_eq!(
            program_generator(&dispatch_quest_select_top_k("scores", "queue", 16, 4, -1.0)),
            "vyre-primitives::nn::quest_select_top_k"
        );
    }

    #[test]
    fn quest_bodies_are_composable_ir_blocks() {
        assert_ne!(quest_page_queue_zero_body("queue", 16).len(), 0);
        assert_ne!(
            quest_page_score_body("query", "pages", "scores", 16, 4).len(),
            0
        );
        assert_ne!(
            quest_page_select_top_k_body("scores", "queue", 16, 4, -1.0).len(),
            0
        );
    }
}
