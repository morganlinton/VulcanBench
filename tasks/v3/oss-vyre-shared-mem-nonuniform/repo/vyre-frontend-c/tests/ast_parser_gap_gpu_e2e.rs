//! GPU frontend regression coverage for the AST parser gap fixture.

mod support;

use support::*;

#[test]
fn ast_parser_gap_fixture_compiles_on_gpu_frontend() {
    let (object, _resident) = compile_source_with_resident(
        "ast_parser_gap_gpu",
        AST_PARSER_GAP_SOURCE,
        Vec::new(),
        Vec::new(),
    );
    object.assert_elf();

    let lex = object.lex();
    assert!(
        lex.tok_types.len() > 40,
        "gap fixture should produce a non-trivial token stream"
    );
    assert!(
        !object.section(SECTION_VAST).is_empty(),
        "VAST section present"
    );
    assert!(
        !object.section(SECTION_PROGRAM_GRAPH).is_empty(),
        "program graph section present"
    );
    assert!(
        !object.section(SECTION_SEMA_SCOPE).is_empty(),
        "semantic scope section present"
    );
    assert!(
        !object
            .section(SECTION_SEMANTIC_PROGRAM_GRAPH_NODES)
            .is_empty(),
        "semantic PG node section present"
    );
    assert!(
        !object
            .section(SECTION_SEMANTIC_PROGRAM_GRAPH_EDGES)
            .is_empty(),
        "semantic PG edge section present"
    );
}
