//! **Tier 3  -  named C11 GPU pipeline stages** (no CLI, no filesystem).
//!
//! Frontends orchestrate dispatches; embedders can import only the stages they
//! need. Buffer layouts are defined by each `Program`’s `BufferDecl`
//! (`with_count`, read/write); see each builder's module for harness fixtures.
//!
//! Full roadmap: `docs/COMPILER_E2E_PLAN.md`.

pub use crate::parsing::c::lex::diagnostics::{
    first_c11_lexer_diagnostic, C11LexerDiagnostic, C11LexerDiagnosticKind,
};
pub use crate::parsing::c::lex::keyword::{c_keyword, c_keyword_packed_haystack};
pub use crate::parsing::c::lex::lexer::{c11_lex_digraphs, c11_lexer};
pub use crate::parsing::c::lex::tokens::is_c_lexer_error_token;
pub use crate::parsing::c::lower::{c_lower_ast_to_pg_nodes, c_lower_ast_to_pg_semantic_graph};
pub use crate::parsing::c::parse::declarations::opt_propagate_type_specifiers;
pub use crate::parsing::c::parse::gnu_builtins::c11_gnu_builtins_pass;
pub use crate::parsing::c::parse::inline_asm::c11_gnu_inline_asm_pass;
pub use crate::parsing::c::parse::structure::{c11_extract_calls, c11_extract_functions};
pub use crate::parsing::c::parse::vast::{
    c11_annotate_typedef_names, c11_annotate_typedef_names_packed_haystack,
    c11_annotate_typedef_names_precomputed_context,
    c11_annotate_typedef_names_precomputed_context_packed_haystack,
    c11_annotate_typedef_names_precomputed_scope,
    c11_annotate_typedef_names_precomputed_scope_packed_haystack, c11_build_expression_shape_nodes,
    c11_build_vast_nodes, c11_classify_vast_node_kinds, c11_link_vast_typedef_symbols,
    c11_precompute_vast_decl_contexts, c11_precompute_vast_scopes, c11_prehash_vast_identifiers,
    c11_prehash_vast_identifiers_packed_haystack,
};
pub use crate::parsing::c::preprocess::effects::{
    classify_c_preprocessor_side_effect, CPreprocessorSideEffect, CPreprocessorSideEffectKind,
    CPreprocessorSideEffectMetadata,
};
pub use crate::parsing::c::preprocess::expansion::{
    opt_conditional_mask, opt_conditional_mask_with_directives, opt_dynamic_macro_expansion,
    opt_named_macro_expansion, opt_named_macro_expansion_materialized, C_MACRO_KIND_FUNCTION_LIKE,
    C_MACRO_KIND_OBJECT_LIKE, C_MACRO_REPLACEMENT_LITERAL,
};
pub use crate::parsing::c::preprocess::materialization::C_MACRO_SOURCE_COUNT_BYTES;
pub use crate::parsing::c::preprocess::source::{
    parse_c_include_request, CIncludeRequest, CIncludeStyle, CPreprocessorSourceManager,
    CResolvedInclude, CSourceFile,
};
pub use crate::parsing::c::preprocess::synthesis::{
    stringification_token_type, synthesize_token_paste_type, C_TOKEN_PASTE_RULES,
};
pub use crate::parsing::c::preprocess::{c_translation_phase_line_splice, CLineSplicedSource};
pub use crate::parsing::c::sema::registry::{c_sema_scope, c_sema_scope_packed_haystack};
pub use crate::parsing::core::ast::shunting::ast_shunting_yard;
pub use crate::{
    c11_build_cfg_and_gotos, c11_compute_alignments, opt_lower_elf, opt_stack_layout_generation,
    opt_x86_64_register_allocation,
};

/// Upper bound on token stream length for `ast_shunting_yard` / padded tok buffers.
/// Must match `vyre-libs` `ast_shunting_yard` implementation.
pub const C11_AST_MAX_TOK_SCAN: u32 = 65536;

#[cfg(test)]
mod tests {
    const SOURCE: &str = include_str!("stages.rs");

    #[test]
    fn c_pipeline_stage_surface_is_consumer_neutral() {
        for forbidden in [
            concat!("vy", "rec"),
            concat!("we", "ir"),
            concat!("sur", "gec"),
            concat!("gos", "san"),
            concat!("key", "hog"),
            concat!("vyre-frontend", "-c"),
        ] {
            assert!(
                !SOURCE.to_ascii_lowercase().contains(forbidden),
                "C pipeline stages must describe generic frontend embedders, not consumer names"
            );
        }
    }
}
