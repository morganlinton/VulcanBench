use super::*;

#[test]
fn attribute_names_classify_as_specific_kinds_gpu_cpu_parity() {
    let fix = fixture_attribute_names();

    let lexed = lex_c11_max_munch_kinds(fix.source.as_bytes()).expect("fixture must lex");
    let lexed_non_ws = lexed
        .into_iter()
        .filter(|k| *k != TOK_WHITESPACE && *k != TOK_COMMENT)
        .collect::<Vec<_>>();
    assert_eq!(lexed_non_ws, fix.raw_kinds);

    assert_full_pipeline_parity(&fix, "attribute_names");

    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    let typed = reference_c11_classify_vast_node_kinds(&annotated);
    assert_eq!(row_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE), vec![0]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ATTRIBUTE_SECTION), vec![3]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ATTRIBUTE_WEAK), vec![8]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ATTRIBUTE_ALIAS), vec![10]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ATTRIBUTE_ALIGNED), vec![15]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ATTRIBUTE_USED), vec![20]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ATTRIBUTE_UNUSED), vec![22]);
    assert_eq!(row_indices(&typed, C_AST_KIND_ATTRIBUTE_NAKED), vec![24]);
    assert_eq!(
        row_indices(&typed, C_AST_KIND_ATTRIBUTE_VISIBILITY),
        vec![26]
    );
}
