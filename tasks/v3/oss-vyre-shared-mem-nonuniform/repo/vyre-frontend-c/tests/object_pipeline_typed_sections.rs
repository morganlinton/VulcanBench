//! Object pipeline invariant: token, class, VAST, typed VAST, ProgramGraph, and semantic
//! ProgramGraph sections are present and non-empty in every compiled translation unit.

mod support;

use support::{
    compile_source, ObjectEnvelope, SECTION_EXPRESSION_SHAPE, SECTION_LEX, SECTION_MACRO_TYPES,
    SECTION_PROGRAM_GRAPH, SECTION_SEMANTIC_PROGRAM_GRAPH_EDGES,
    SECTION_SEMANTIC_PROGRAM_GRAPH_NODES, SECTION_SEMA_SCOPE, SECTION_VAST, VAST_STRIDE_U32,
};
use vyre_frontend_c::api::object_decode::decode_object_sema_scope;

const MINIMAL_SOURCE: &str = "int main(void) { return 0; }\n";

#[test]
fn compiled_object_carries_token_class_vast_typed_vast_pg_and_semantic_pg_sections() {
    let object = compile_source("typed_pipeline", MINIMAL_SOURCE, Vec::new());
    object.assert_elf();
    let env = ObjectEnvelope::from_elf(object.into_inner());

    // Lex (token stream) and MacroTypes (class snapshot) must be present and non-empty.
    for (label, tag) in [
        ("token/lex", SECTION_LEX),
        ("class/macro-types", SECTION_MACRO_TYPES),
    ] {
        env.assert_section_present(tag);
        let data = env
            .section(tag)
            .unwrap_or_else(|| panic!("{label} section present"));
        assert!(!data.is_empty(), "{label} section is non-empty");
    }

    // VAST section carries typed AST node kinds (classify-pass output).
    env.assert_section_present(SECTION_VAST);
    let vast_data = env.section(SECTION_VAST).unwrap();
    assert!(!vast_data.is_empty(), "VAST section is non-empty");
    let vast_words = env.words(SECTION_VAST);
    assert_eq!(
        vast_words.len() % VAST_STRIDE_U32,
        0,
        "VAST section length is a whole multiple of row stride"
    );
    assert!(
        vast_words
            .chunks_exact(VAST_STRIDE_U32)
            .any(|row| row[0] != 0),
        "typed VAST contains at least one classified AST node kind"
    );

    // ProgramGraph and semantic ProgramGraph sections must be present and non-empty.
    for (label, tag) in [
        ("ProgramGraph", SECTION_PROGRAM_GRAPH),
        ("semantic-PG-nodes", SECTION_SEMANTIC_PROGRAM_GRAPH_NODES),
        ("semantic-PG-edges", SECTION_SEMANTIC_PROGRAM_GRAPH_EDGES),
    ] {
        env.assert_section_present(tag);
        let data = env
            .section(tag)
            .unwrap_or_else(|| panic!("{label} section present"));
        assert!(!data.is_empty(), "{label} section is non-empty");
    }

    // Expression-shape (classification of expressions) must also be present.
    env.assert_section_present(SECTION_EXPRESSION_SHAPE);
    assert!(
        !env.section(SECTION_EXPRESSION_SHAPE).unwrap().is_empty(),
        "expression-shape section is non-empty"
    );

    env.assert_section_present(SECTION_SEMA_SCOPE);
    let scope = decode_object_sema_scope(env.payload())
        .expect("real compiled object SemaScope section decodes through stable API");
    assert!(
        scope.records.iter().any(|record| record.token_len > 0),
        "SemaScope object rows carry source spans from GPU lexer output"
    );
    assert!(
        scope.records.iter().any(|record| record.has_declaration()),
        "SemaScope object rows carry declaration evidence"
    );
}
