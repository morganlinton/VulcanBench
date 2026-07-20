use super::*;

#[test]
fn macro_expanded_dense_attribute_asm_typeof_stream() {
    // This source looks like the output of heavy macro expansion:
    // no superfluous whitespace, double-underscore spellings everywhere.
    let src = r#"__attribute__((__section__(".text"))) static __inline__ __typeof__(int) *foo(void) { __asm__ __volatile__("nop" ::: "memory"); return 0; }"#;
    let p = parse_source(src);
    let v = &p.typed_vast;

    // All expected kinds must be present.
    let _ = find_single(v, C_AST_KIND_GNU_ATTRIBUTE);
    let _ = find_single(v, C_AST_KIND_ATTRIBUTE_SECTION);
    let _ = find_single(v, C_AST_KIND_INLINE_ASM);
    let _ = find_single(v, C_AST_KIND_ASM_TEMPLATE);
    let _ = find_single(v, C_AST_KIND_ASM_CLOBBERS_LIST);

    // `__inline__` must have been promoted to TOK_INLINE.
    let inline_toks: Vec<usize> = p
        .tok_types
        .iter()
        .enumerate()
        .filter_map(|(idx, &k)| (k == TOK_INLINE).then_some(idx))
        .collect();
    assert_eq!(
        inline_toks.len(),
        1,
        "__inline__ must be promoted to exactly one TOK_INLINE"
    );

    // `__typeof__` must have been promoted to TOK_GNU_TYPEOF.
    let typeof_toks: Vec<usize> = p
        .tok_types
        .iter()
        .enumerate()
        .filter_map(|(idx, &k)| (k == TOK_GNU_TYPEOF).then_some(idx))
        .collect();
    assert_eq!(
        typeof_toks.len(),
        1,
        "__typeof__ must be promoted to exactly one TOK_GNU_TYPEOF"
    );

    // `__asm__` must have been promoted to TOK_GNU_ASM.
    let asm_toks: Vec<usize> = p
        .tok_types
        .iter()
        .enumerate()
        .filter_map(|(idx, &k)| (k == TOK_GNU_ASM).then_some(idx))
        .collect();
    assert_eq!(
        asm_toks.len(),
        1,
        "__asm__ must be promoted to exactly one TOK_GNU_ASM"
    );

    // Span monotonicity already asserted by parse_source, but double-check
    // that every emitted row has a valid parent or is a root.
    let n = node_count_from_vast(v);
    for i in 0..n {
        let parent = parent_at(v, i);
        if parent != u32::MAX {
            assert!(
                (parent as usize) < n,
                "parent index {parent} out of bounds for node {i}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 7. Cross-cutting negative contracts
// ---------------------------------------------------------------------------

#[test]
fn section_identifier_outside_attribute_not_misclassified() {
    // A variable named `section` must not get ATTRIBUTE_SECTION.
    let p = parse_source(r#"int section = 0;"#);
    let v = &p.typed_vast;

    let section_attrs = indices_with_kind(v, C_AST_KIND_ATTRIBUTE_SECTION);
    assert!(
        section_attrs.is_empty(),
        "bare identifier 'section' must not be classified as ATTRIBUTE_SECTION"
    );
}

#[test]
fn aligned_identifier_outside_attribute_not_misclassified() {
    let p = parse_source(r#"int aligned = 0;"#);
    let v = &p.typed_vast;

    let aligned_attrs = indices_with_kind(v, C_AST_KIND_ATTRIBUTE_ALIGNED);
    assert!(
        aligned_attrs.is_empty(),
        "bare identifier 'aligned' must not be classified as ATTRIBUTE_ALIGNED"
    );
}
