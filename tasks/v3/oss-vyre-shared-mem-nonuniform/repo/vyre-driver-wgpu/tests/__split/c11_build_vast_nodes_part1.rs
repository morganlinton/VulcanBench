use super::*;

#[test]
fn cpu_reference_builds_delimiter_tree_for_function_body() {
    let tok_types = [
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RETURN,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_starts = [0u32, 4, 8, 9, 10, 11, 18, 19, 20];
    let tok_lens = [3u32, 4, 1, 1, 1, 6, 1, 1, 1];
    let rows = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);

    assert_vast_row(&rows, 0, TOK_INT, u32::MAX, u32::MAX, 1);
    assert_vast_row(&rows, 1, TOK_IDENTIFIER, u32::MAX, u32::MAX, 2);
    assert_vast_row(&rows, 2, TOK_LPAREN, u32::MAX, 3, 4);
    assert_vast_row(&rows, 3, TOK_RPAREN, 2, u32::MAX, u32::MAX);
    assert_vast_row(&rows, 4, TOK_LBRACE, u32::MAX, 5, u32::MAX);
    assert_vast_row(&rows, 5, TOK_RETURN, 4, u32::MAX, 6);
    assert_vast_row(&rows, 6, TOK_INTEGER, 4, u32::MAX, 7);
    assert_vast_row(&rows, 7, TOK_SEMICOLON, 4, u32::MAX, 8);
    assert_vast_row(&rows, 8, TOK_RBRACE, 4, u32::MAX, u32::MAX);
}

#[test]
fn cpu_reference_classifies_gnu_c_style_function_definition() {
    let tok_types = [
        TOK_STATIC,
        TOK_INLINE,
        TOK_LONG,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_CONST,
        TOK_CHAR_KW,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RETURN,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_QUESTION,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_COLON,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_lens = [
        6, 6, 4, 1, 9, 1, 6, 6, 1, 3, 1, 5, 4, 6, 1, 3, 1, 1, 6, 6, 1, 3, 1, 1, 7, 1, 3, 1, 1, 1,
        1, 1,
    ];
    let tok_starts = starts_for_lens(&tok_lens);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_kind(&typed, 4, C_AST_KIND_FUNCTION_DEFINITION);
    assert_kind(&typed, 17, node_kind::BASIC_BLOCK);
    assert_kind(&typed, 19, node_kind::CALL);
    assert_kind(&typed, 21, node_kind::VARIABLE);
    assert_kind(&typed, 24, node_kind::CALL);
    assert_kind(&typed, 29, node_kind::LITERAL);
    assert_eq!(
        word_at(&typed, 4 * VAST_STRIDE_U32 + 5),
        tok_starts[4],
        "function span start must survive classification"
    );
}

#[test]
fn cpu_reference_classifies_gnu_c_typedef_attributes_blocks_and_calls() {
    let tok_types = [
        TOK_TYPEDEF,
        TOK_LONG,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_STATIC,
        TOK_INLINE,
        TOK_LONG,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_STRUCT,
        TOK_IDENTIFIER,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_CONST,
        TOK_CHAR_KW,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_GNU_ATTRIBUTE,
        TOK_LPAREN,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_LBRACE,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_IF,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_AMP,
        TOK_INTEGER,
        TOK_RPAREN,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_GNU_ASM,
        TOK_VOLATILE,
        TOK_LPAREN,
        TOK_STRING,
        TOK_COLON,
        TOK_COLON,
        TOK_COLON,
        TOK_STRING,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_RETURN,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_RETURN,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_lens = [
        7, 4, 10, 1, 6, 4, 1, 1, 1, 3, 5, 1, 1, 6, 6, 4, 12, 1, 6, 4, 1, 1, 1, 5, 4, 1, 4, 1, 3, 5,
        1, 13, 1, 1, 13, 1, 1, 1, 1, 11, 1, 4, 1, 1, 1, 2, 1, 6, 1, 5, 1, 1, 1, 1, 1, 3, 8, 1, 7,
        1, 1, 1, 8, 1, 1, 6, 8, 1, 1, 1, 5, 1, 1, 1, 6, 1, 1, 1,
    ];
    let tok_starts = starts_for_lens(&tok_lens);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    let functions = typed_indices(&typed, node_kind::FUNCTION_DECL);
    let function_defs = typed_indices(&typed, C_AST_KIND_FUNCTION_DEFINITION);
    let calls = typed_indices(&typed, node_kind::CALL);
    let blocks = typed_indices(&typed, node_kind::BASIC_BLOCK);
    let literals = typed_indices(&typed, node_kind::LITERAL);
    let attributes = typed_indices(&typed, C_AST_KIND_GNU_ATTRIBUTE);
    let inline_asm = typed_indices(&typed, C_AST_KIND_INLINE_ASM);
    let asm_templates = typed_indices(&typed, C_AST_KIND_ASM_TEMPLATE);
    let asm_clobbers = typed_indices(&typed, C_AST_KIND_ASM_CLOBBERS_LIST);

    assert_eq!(
        functions,
        vec![2],
        "typedef prototype must remain a generic function declaration"
    );
    assert_eq!(
        function_defs,
        vec![16],
        "attributed GNU-C definition with a body must be a first-class function definition"
    );
    assert!(
        calls.len() >= 3,
        "trace_fault, likely, and do_fault calls must be typed; got {calls:?}"
    );
    assert!(
        blocks.len() >= 3,
        "outer function, nested block, and if body must be basic blocks"
    );
    assert!(
        literals.len() >= 2,
        "integer literals must survive classification"
    );
    assert_eq!(
        attributes,
        vec![31],
        "GNU attribute syntax must be a first-class VAST node"
    );
    assert_eq!(
        inline_asm,
        vec![55],
        "inline asm syntax must be a first-class VAST node"
    );
    assert_eq!(
        asm_templates,
        vec![58],
        "inline asm template strings must be first-class VAST nodes"
    );
    assert_eq!(
        asm_clobbers,
        vec![62],
        "inline asm clobber strings must be first-class VAST nodes"
    );
    assert_vast_row(&typed, 37, node_kind::BASIC_BLOCK, u32::MAX, 38, u32::MAX);
    assert_vast_row(&typed, 38, node_kind::BASIC_BLOCK, 37, 39, 45);
    assert_vast_row(&typed, 54, node_kind::BASIC_BLOCK, 37, 55, 74);
    assert_ne!(
        word_at(&typed, 31 * VAST_STRIDE_U32),
        node_kind::FUNCTION_DECL,
        "GNU attribute suffix must not be mistaken for the function declarator"
    );
}

#[test]
fn cpu_reference_classifies_c_statement_keywords_as_first_class_vast_nodes() {
    let tok_types = [
        TOK_IF,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_RETURN,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_ELSE,
        TOK_FOR,
        TOK_LPAREN,
        TOK_SEMICOLON,
        TOK_SEMICOLON,
        TOK_RPAREN,
        TOK_WHILE,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_DO,
        TOK_CONTINUE,
        TOK_SEMICOLON,
        TOK_WHILE,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_SWITCH,
        TOK_LPAREN,
        TOK_IDENTIFIER,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_CASE,
        TOK_INTEGER,
        TOK_COLON,
        TOK_BREAK,
        TOK_SEMICOLON,
        TOK_DEFAULT,
        TOK_COLON,
        TOK_GOTO,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let tok_lens = [
        2, 1, 1, 1, 6, 1, 1, 4, 3, 1, 1, 1, 1, 5, 1, 1, 1, 2, 8, 1, 5, 1, 1, 1, 1, 6, 1, 1, 1, 1,
        4, 1, 1, 5, 1, 7, 1, 4, 3, 1, 1,
    ];
    let tok_starts = starts_for_lens(&tok_lens);
    let raw = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed = reference_c11_classify_vast_node_kinds(&raw);

    assert_kind(&typed, 0, C_AST_KIND_IF_STMT);
    assert_kind(&typed, 4, C_AST_KIND_RETURN_STMT);
    assert_kind(&typed, 7, C_AST_KIND_ELSE_STMT);
    assert_kind(&typed, 8, C_AST_KIND_FOR_STMT);
    assert_kind(&typed, 13, C_AST_KIND_WHILE_STMT);
    assert_kind(&typed, 17, C_AST_KIND_DO_STMT);
    assert_kind(&typed, 18, C_AST_KIND_CONTINUE_STMT);
    assert_kind(&typed, 20, C_AST_KIND_WHILE_STMT);
    assert_kind(&typed, 25, C_AST_KIND_SWITCH_STMT);
    assert_kind(&typed, 30, C_AST_KIND_CASE_STMT);
    assert_kind(&typed, 33, C_AST_KIND_BREAK_STMT);
    assert_kind(&typed, 35, C_AST_KIND_DEFAULT_STMT);
    assert_kind(&typed, 37, C_AST_KIND_GOTO_STMT);
    assert_kind(&typed, 38, node_kind::VARIABLE);
    assert_kind(&typed, 29, node_kind::BASIC_BLOCK);
}

