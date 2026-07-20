use super::*;
use vyre_libs::parsing::c::lex::keyword::fnv1a32;

#[test]
fn attribute_double_underscore_section_classified_correctly() {
    let p = parse_source(r#"__attribute__((__section__(".init.text"))) void foo(void) {}"#);
    let v = &p.typed_vast;

    // Exactly one GNU attribute wrapper and one section attribute kind.
    let attr_idxs = indices_with_kind(v, C_AST_KIND_GNU_ATTRIBUTE);
    let section_idxs = indices_with_kind(v, C_AST_KIND_ATTRIBUTE_SECTION);
    assert_eq!(attr_idxs.len(), 1, "expected exactly one GNU_ATTRIBUTE");
    assert_eq!(
        section_idxs.len(),
        1,
        "expected exactly one ATTRIBUTE_SECTION"
    );

    // The section attribute token must lex as `__section__`.
    let section_idx = section_idxs[0];
    let lex = lexeme_at(&p.source, start_at(v, section_idx), len_at(v, section_idx));
    assert_eq!(lex, Some(b"__section__" as &[u8]));

    // The string literal `.init.text` must exist somewhere in the source spans.
    let strings: Vec<usize> = v
        .chunks_exact(VAST_STRIDE_U32 * 4)
        .enumerate()
        .filter_map(|(idx, row)| {
            let row_kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
            (row_kind == TOK_STRING).then_some(idx)
        })
        .collect();
    assert_eq!(strings.len(), 1, "expected exactly one string literal");
    let lex = lexeme_at(&p.source, start_at(v, strings[0]), len_at(v, strings[0]));
    assert_eq!(lex, Some(b"\".init.text\"" as &[u8]));
}

#[test]
fn attribute_double_underscore_aligned_classified_correctly() {
    let p = parse_source(r#"__attribute__((__aligned__(64))) int buf[8];"#);
    let v = &p.typed_vast;

    let attr_idxs = indices_with_kind(v, C_AST_KIND_GNU_ATTRIBUTE);
    let aligned_idxs = indices_with_kind(v, C_AST_KIND_ATTRIBUTE_ALIGNED);
    assert_eq!(attr_idxs.len(), 1);
    assert_eq!(aligned_idxs.len(), 1);

    let aligned_idx = aligned_idxs[0];
    let lex = lexeme_at(&p.source, start_at(v, aligned_idx), len_at(v, aligned_idx));
    assert_eq!(lex, Some(b"__aligned__" as &[u8]));

    // The integer argument 64 must be present as a literal (classifier rewrites
    // TOK_INTEGER to node_kind::LITERAL).
    let lit_idxs: Vec<usize> = v
        .chunks_exact(VAST_STRIDE_U32 * 4)
        .enumerate()
        .filter_map(|(idx, row)| {
            let row_kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
            (row_kind == node_kind::LITERAL).then_some(idx)
        })
        .collect();
    assert!(!lit_idxs.is_empty(), "expected at least one literal");
    let lit64 = lit_idxs
        .iter()
        .find(|&&idx| lexeme_at(&p.source, start_at(v, idx), len_at(v, idx)) == Some(b"64"));
    assert!(lit64.is_some(), "literal '64' must be present");
}

#[test]
fn attribute_double_underscore_weak_classified_correctly() {
    let p = parse_source(r#"__attribute__((__weak__)) int sym;"#);
    let v = &p.typed_vast;

    let attr_idxs = indices_with_kind(v, C_AST_KIND_GNU_ATTRIBUTE);
    let weak_idxs = indices_with_kind(v, C_AST_KIND_ATTRIBUTE_WEAK);
    assert_eq!(attr_idxs.len(), 1);
    assert_eq!(weak_idxs.len(), 1);

    let weak_idx = weak_idxs[0];
    let lex = lexeme_at(&p.source, start_at(v, weak_idx), len_at(v, weak_idx));
    assert_eq!(lex, Some(b"__weak__" as &[u8]));

    // The attribute token is a leaf (no children in the delimiter tree).
    assert_eq!(
        first_child_at(v, weak_idx),
        u32::MAX,
        "weak attribute token must be a leaf node"
    );
}

#[test]
fn bare_weak_identifier_not_misclassified_as_attribute() {
    // `weak` used as an ordinary variable name must not receive ATTRIBUTE_WEAK.
    let p = parse_source(r#"int weak = 1;"#);
    let v = &p.typed_vast;

    let weak_idxs = indices_with_kind(v, C_AST_KIND_ATTRIBUTE_WEAK);
    assert!(
        weak_idxs.is_empty(),
        "bare identifier 'weak' must not be classified as ATTRIBUTE_WEAK"
    );

    // Verify the identifier is present as a normal token.
    let idents: Vec<usize> = v
        .chunks_exact(VAST_STRIDE_U32 * 4)
        .enumerate()
        .filter_map(|(idx, row)| {
            let row_kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
            let hash = word_at(v, idx * VAST_STRIDE_U32 + 9);
            (row_kind == TOK_IDENTIFIER && hash == fnv1a32(b"weak")).then_some(idx)
        })
        .collect();
    assert_eq!(
        idents.len(),
        1,
        "exactly one 'weak' identifier token must exist"
    );
}

// ---------------------------------------------------------------------------
// 2. Inline asm with operands, clobbers, and asm goto labels
// ---------------------------------------------------------------------------

#[test]
fn inline_asm_extended_operands_and_clobbers_structure() {
    let p =
        parse_source(r#"asm volatile ("mov %1, %0" : "=r" (out) : "r" (in) : "memory", "cc");"#);
    let v = &p.typed_vast;

    // There is one asm keyword, one template string, one output operand paren,
    // one input operand paren, and two clobber strings.
    let asm_idxs = indices_with_kind(v, C_AST_KIND_INLINE_ASM);
    let template_idxs = indices_with_kind(v, C_AST_KIND_ASM_TEMPLATE);
    let out_idxs = indices_with_kind(v, C_AST_KIND_ASM_OUTPUT_OPERAND);
    let in_idxs = indices_with_kind(v, C_AST_KIND_ASM_INPUT_OPERAND);
    let clob_idxs = indices_with_kind(v, C_AST_KIND_ASM_CLOBBERS_LIST);

    assert_eq!(asm_idxs.len(), 1, "expected exactly one INLINE_ASM");
    assert_eq!(template_idxs.len(), 1, "expected exactly one ASM_TEMPLATE");
    assert_eq!(out_idxs.len(), 1, "expected exactly one ASM_OUTPUT_OPERAND");
    assert_eq!(in_idxs.len(), 1, "expected exactly one ASM_INPUT_OPERAND");
    assert_eq!(clob_idxs.len(), 2, "expected exactly two ASM_CLOBBERS_LIST");

    // Qualifier (volatile) must exist.
    let qual_idxs = indices_with_kind(v, C_AST_KIND_ASM_QUALIFIER);
    assert!(
        !qual_idxs.is_empty(),
        "asm volatile must produce at least one qualifier node"
    );

    // The output operand paren must contain the identifier `out`.
    let out_idents: Vec<usize> = v
        .chunks_exact(VAST_STRIDE_U32 * 4)
        .enumerate()
        .filter_map(|(idx, row)| {
            let row_kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
            (row_kind == TOK_IDENTIFIER).then_some(idx)
        })
        .collect();
    let out_id = out_idents
        .iter()
        .find(|&&idx| lexeme_at(&p.source, start_at(v, idx), len_at(v, idx)) == Some(b"out"));
    assert!(
        out_id.is_some(),
        "output operand must contain identifier 'out'"
    );

    // The input operand paren must contain the identifier `in`.
    let in_id = out_idents
        .iter()
        .find(|&&idx| lexeme_at(&p.source, start_at(v, idx), len_at(v, idx)) == Some(b"in"));
    assert!(
        in_id.is_some(),
        "input operand must contain identifier 'in'"
    );

    // Clobber strings must be "memory" and "cc".
    let clob_lexemes: Vec<&[u8]> = clob_idxs
        .iter()
        .map(|&idx| lexeme_at(&p.source, start_at(v, idx), len_at(v, idx)).unwrap_or(b""))
        .collect();
    assert!(clob_lexemes.contains(&(b"\"memory\"" as &[u8])));
    assert!(clob_lexemes.contains(&(b"\"cc\"" as &[u8])));
}

#[test]
fn asm_goto_labels_structure() {
    let p = parse_source(r#"asm goto ("jmp %l0" ::: : fail, ok);"#);
    let v = &p.typed_vast;

    let asm_idxs = indices_with_kind(v, C_AST_KIND_INLINE_ASM);
    let label_idxs = indices_with_kind(v, C_AST_KIND_ASM_GOTO_LABELS);
    assert_eq!(asm_idxs.len(), 1, "expected exactly one INLINE_ASM");
    assert_eq!(label_idxs.len(), 2, "expected exactly two ASM_GOTO_LABELS");

    // Each label must lex to the expected identifier.
    let label_lexemes: Vec<&[u8]> = label_idxs
        .iter()
        .map(|&idx| lexeme_at(&p.source, start_at(v, idx), len_at(v, idx)).unwrap_or(b""))
        .collect();
    assert!(label_lexemes.contains(&(b"fail" as &[u8])));
    assert!(label_lexemes.contains(&(b"ok" as &[u8])));

    // There must be a goto qualifier.
    let qual_idxs = indices_with_kind(v, C_AST_KIND_ASM_QUALIFIER);
    let goto_qual = qual_idxs
        .iter()
        .copied()
        .find(|&idx| lexeme_at(&p.source, start_at(v, idx), len_at(v, idx)) == Some(b"goto"));
    assert!(
        goto_qual.is_some(),
        "asm goto must carry a 'goto' qualifier node"
    );
}

// ---------------------------------------------------------------------------
// 3. typeof / __typeof__ / __typeof__
// ---------------------------------------------------------------------------

#[test]
fn typeof_forms_promoted_and_structured() {
    // All three spellings must produce the same structural shape.
    for src in [
        r#"typeof(int) *p;"#,
        r#"__typeof(int) *p;"#,
        r#"__typeof__(int) *p;"#,
    ] {
        let p = parse_source(src);
        let v = &p.typed_vast;

        // After keyword promotion the typeof token must be TOK_GNU_TYPEOF.
        let typeof_toks: Vec<usize> = p
            .tok_types
            .iter()
            .enumerate()
            .filter_map(|(idx, &k)| (k == TOK_GNU_TYPEOF).then_some(idx))
            .collect();
        assert_eq!(
            typeof_toks.len(),
            1,
            "{src}: exactly one typeof token must be promoted to TOK_GNU_TYPEOF"
        );

        // The declarator `*p` must be present; at minimum the declaration must
        // contain an identifier `p`.
        let p_idents: Vec<usize> = v
            .chunks_exact(VAST_STRIDE_U32 * 4)
            .enumerate()
            .filter_map(|(idx, row)| {
                let row_kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
                let hash = word_at(v, idx * VAST_STRIDE_U32 + 9);
                (row_kind == TOK_IDENTIFIER && hash == fnv1a32(b"p")).then_some(idx)
            })
            .collect();
        assert_eq!(
            p_idents.len(),
            1,
            "{src}: exactly one 'p' identifier must exist"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. Statement expressions
// ---------------------------------------------------------------------------

#[test]
fn statement_expr_in_initializer_context() {
    let p = parse_source(r#"int x = ({ int y = 1; y; });"#);
    let v = &p.typed_vast;

    let stmt_expr_idxs = indices_with_kind(v, C_AST_KIND_GNU_STATEMENT_EXPR);
    assert_eq!(
        stmt_expr_idxs.len(),
        1,
        "expected exactly one GNU_STATEMENT_EXPR"
    );

    // The statement expression must contain assignment expressions for `y = 1`.
    let assign_idxs = indices_with_kind(v, C_AST_KIND_ASSIGN_EXPR);
    assert!(
        !assign_idxs.is_empty(),
        "statement expression must contain at least one assignment"
    );

    // Verify the `int y = 1;` assignment is inside the statement expr.
    // In the delimiter tree, `y = 1` ASSIGN_EXPR is inside the `({ ... })`
    // braces, so its parent chain reaches the `{` that is a sibling of `(`.
    let inside_stmt_expr = assign_idxs.iter().any(|&idx| {
        let mut cur = idx as u32;
        while cur != u32::MAX {
            let k = kind_at(v, cur as usize);
            if k == C_AST_KIND_GNU_STATEMENT_EXPR {
                return true;
            }
            // Also accept if we reach the opening brace of the stmt-expr
            // because the classifier may not rewrite the `{` kind itself.
            if k == TOK_LBRACE {
                return true;
            }
            cur = parent_at(v, cur as usize);
        }
        false
    });
    assert!(
        inside_stmt_expr,
        "assignment must be nested inside the statement expression"
    );
}

// ---------------------------------------------------------------------------
// 5. Designated initializers
// ---------------------------------------------------------------------------

#[test]
fn designated_initializer_struct_fields() {
    let p = parse_source(r#"struct { int a; int b; } s = { .a = 1, .b = 2 };"#);
    let v = &p.typed_vast;

    let init_idxs = indices_with_kind(v, C_AST_KIND_INITIALIZER_LIST);
    assert!(
        !init_idxs.is_empty(),
        "expected at least one INITIALIZER_LIST"
    );

    // Field designators `.a` and `.b` must be present as MEMBER_ACCESS_EXPR.
    let member_idxs = indices_with_kind(v, C_AST_KIND_MEMBER_ACCESS_EXPR);
    assert_eq!(member_idxs.len(), 2, "expected two field designators");

    // The identifiers immediately following the `.` tokens must be `a` and `b`.
    let dot_idents: Vec<&[u8]> = member_idxs
        .iter()
        .filter_map(|&midx| {
            let nxt = next_sibling_at(v, midx);
            if nxt == u32::MAX {
                return None;
            }
            lexeme_at(
                &p.source,
                start_at(v, nxt as usize),
                len_at(v, nxt as usize),
            )
        })
        .collect();
    assert!(dot_idents.iter().any(|&lex| lex == b"a"));
    assert!(dot_idents.iter().any(|&lex| lex == b"b"));
}

#[test]
fn designated_initializer_array_range() {
    let p = parse_source(r#"int arr[4] = { [0] = 1, [1 ... 2] = 3 };"#);
    let v = &p.typed_vast;

    let init_idxs = indices_with_kind(v, C_AST_KIND_INITIALIZER_LIST);
    assert!(
        !init_idxs.is_empty(),
        "expected at least one INITIALIZER_LIST"
    );

    // Must contain exactly one range designator (the `...` token).
    let range_idxs = indices_with_kind(v, C_AST_KIND_RANGE_DESIGNATOR_EXPR);
    assert_eq!(range_idxs.len(), 1, "expected exactly one range designator");

    // Verify the ellipsis lexeme.
    let range_idx = range_idxs[0];
    let lex = lexeme_at(&p.source, start_at(v, range_idx), len_at(v, range_idx));
    assert_eq!(lex, Some(b"..." as &[u8]));

    // Integer literals 1 and 2 must exist (classifier rewrites them to LITERAL).
    let lit_idxs: Vec<usize> = v
        .chunks_exact(VAST_STRIDE_U32 * 4)
        .enumerate()
        .filter_map(|(idx, row)| {
            let row_kind = u32::from_le_bytes(row[0..4].try_into().unwrap());
            (row_kind == node_kind::LITERAL).then_some(idx)
        })
        .collect();
    let lit1 = lit_idxs
        .iter()
        .find(|&&idx| lexeme_at(&p.source, start_at(v, idx), len_at(v, idx)) == Some(b"1"));
    let lit2 = lit_idxs
        .iter()
        .find(|&&idx| lexeme_at(&p.source, start_at(v, idx), len_at(v, idx)) == Some(b"2"));
    assert!(lit1.is_some(), "literal '1' must exist");
    assert!(lit2.is_some(), "literal '2' must exist");
}

// ---------------------------------------------------------------------------
// 6. Macro-expanded-looking dense token streams
// ---------------------------------------------------------------------------
