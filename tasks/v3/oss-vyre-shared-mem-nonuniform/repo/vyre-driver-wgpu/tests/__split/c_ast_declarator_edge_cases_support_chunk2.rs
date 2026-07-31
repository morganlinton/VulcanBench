/// ```c
/// int a, *b, c[4];
/// ```
fn fixture_multiple_declarators() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_STAR,
        TOK_IDENTIFIER,
        TOK_COMMA,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_INTEGER,
        TOK_RBRACKET,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// struct { int len; char data[]; };
/// ```
fn fixture_flexible_array_member() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_LBRACE,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_SEMICOLON,
        TOK_CHAR_KW,
        TOK_IDENTIFIER,
        TOK_LBRACKET,
        TOK_RBRACKET,
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

/// ```c
/// struct { int flags : 16; unsigned int : 0; };
/// ```
fn fixture_bitfield_declarators() -> (Vec<u32>, Vec<u32>, Vec<u32>) {
    let tok_types = vec![
        TOK_STRUCT,
        TOK_LBRACE,
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_COLON,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_UNSIGNED,
        TOK_INT,
        TOK_COLON,
        TOK_INTEGER,
        TOK_SEMICOLON,
        TOK_RBRACE,
        TOK_SEMICOLON,
    ];
    let tok_lens = vec![1; tok_types.len()];
    let tok_starts = starts_for_lens(&tok_lens);
    (tok_types, tok_starts, tok_lens)
}

// ---------------------------------------------------------------------------
// CPU reference contract tests
// ---------------------------------------------------------------------------

