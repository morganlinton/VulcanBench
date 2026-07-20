use super::lexer_fast_path::{
    regular_c_lexer_fast_path_safe, regular_c_ranked_lexer_fast_path_safe,
    regular_c_sparse_lexer_fast_path_safe,
};
use super::lexer_program_plan::LexProgramPlan;
use super::*;

pub(super) fn c11_lex_program_for_source(
    source: &str,
    haystack_len: u32,
    out_tok_types: &str,
    out_tok_starts: &str,
    out_tok_lens: &str,
    out_counts: &str,
) -> LexProgramPlan {
    if regular_c_sparse_lexer_fast_path_safe(source) {
        LexProgramPlan {
            program: c11_lexer_regular_sparse(
                "haystack",
                out_tok_types,
                out_tok_starts,
                out_tok_lens,
                out_counts,
                haystack_len,
            ),
            sparse_output: true,
            keyword_promoted: false,
        }
    } else if regular_c_ranked_lexer_fast_path_safe(source) {
        LexProgramPlan {
            program: c11_lexer_regular_ranked(
                "haystack",
                out_tok_types,
                out_tok_starts,
                out_tok_lens,
                out_counts,
                haystack_len,
            ),
            sparse_output: false,
            keyword_promoted: false,
        }
    } else if regular_c_lexer_fast_path_safe(source) {
        LexProgramPlan {
            program: c11_lex_regular_single_pass(
                "haystack",
                out_tok_types,
                out_tok_starts,
                out_tok_lens,
                out_counts,
                haystack_len,
                haystack_len.max(1),
            ),
            sparse_output: false,
            keyword_promoted: false,
        }
    } else {
        LexProgramPlan {
            program: c11_lex_single_pass(
                "haystack",
                out_tok_types,
                out_tok_starts,
                out_tok_lens,
                out_counts,
                haystack_len,
                haystack_len.max(1),
            ),
            sparse_output: false,
            keyword_promoted: false,
        }
    }
}
