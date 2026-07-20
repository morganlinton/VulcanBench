#[test]
fn test_kernel_list_head_parity() {
    let source = "struct list_head {\n  struct list_head *next, *prev;\n};\nstatic inline void INIT_LIST_HEAD(struct list_head *list) {\n  list->next = list;\n  list->prev = list;\n}";
    let tokens = [
        ("struct", TOK_STRUCT),
        (" ", TOK_WHITESPACE),
        ("list_head", TOK_IDENTIFIER),
        (" ", TOK_WHITESPACE),
        ("{", TOK_LBRACE),
        ("\n  ", TOK_WHITESPACE),
        ("struct", TOK_STRUCT),
        (" ", TOK_WHITESPACE),
        ("list_head", TOK_IDENTIFIER),
        (" ", TOK_WHITESPACE),
        ("*", TOK_STAR),
        ("next", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        (" ", TOK_WHITESPACE),
        ("*", TOK_STAR),
        ("prev", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("\n", TOK_WHITESPACE),
        ("}", TOK_RBRACE),
        (";", TOK_SEMICOLON),
        ("\n", TOK_WHITESPACE),
        ("static", TOK_STATIC),
        (" ", TOK_WHITESPACE),
        ("inline", TOK_INLINE),
        (" ", TOK_WHITESPACE),
        ("void", TOK_VOID),
        (" ", TOK_WHITESPACE),
        ("INIT_LIST_HEAD", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("struct", TOK_STRUCT),
        (" ", TOK_WHITESPACE),
        ("list_head", TOK_IDENTIFIER),
        (" ", TOK_WHITESPACE),
        ("*", TOK_STAR),
        ("list", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (" ", TOK_WHITESPACE),
        ("{", TOK_LBRACE),
        ("\n  ", TOK_WHITESPACE),
        ("list", TOK_IDENTIFIER),
        ("->", TOK_ARROW),
        ("next", TOK_IDENTIFIER),
        (" ", TOK_WHITESPACE),
        ("=", TOK_ASSIGN),
        (" ", TOK_WHITESPACE),
        ("list", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("\n  ", TOK_WHITESPACE),
        ("list", TOK_IDENTIFIER),
        ("->", TOK_ARROW),
        ("prev", TOK_IDENTIFIER),
        (" ", TOK_WHITESPACE),
        ("=", TOK_ASSIGN),
        (" ", TOK_WHITESPACE),
        ("list", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("\n", TOK_WHITESPACE),
        ("}", TOK_RBRACE),
    ];
    run_harness("kernel_list_head", source, &tokens);
}

#[test]
fn test_libc_errno_parity() {
    let source = "extern int *__errno_location(void) __attribute__((__const__));";
    let tokens = [
        ("extern", TOK_EXTERN),
        (" ", TOK_WHITESPACE),
        ("int", TOK_INT),
        (" ", TOK_WHITESPACE),
        ("*", TOK_STAR),
        ("__errno_location", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("void", TOK_VOID),
        (")", TOK_RPAREN),
        (" ", TOK_WHITESPACE),
        ("__attribute__", TOK_GNU_ATTRIBUTE),
        ("(", TOK_LPAREN),
        ("(", TOK_LPAREN),
        ("__const__", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ];
    run_harness("libc_errno", source, &tokens);
}

#[test]
fn test_complex_declarator_parity() {
    let source = "int (*(*f(void))(int))[5];";
    let tokens = [
        ("int", TOK_INT),
        (" ", TOK_WHITESPACE),
        ("(", TOK_LPAREN),
        ("*", TOK_STAR),
        ("(", TOK_LPAREN),
        ("*", TOK_STAR),
        ("f", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("void", TOK_VOID),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("(", TOK_LPAREN),
        ("int", TOK_INT),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("[", TOK_LBRACKET),
        ("5", TOK_INTEGER),
        ("]", TOK_RBRACKET),
        (";", TOK_SEMICOLON),
    ];
    run_harness("complex_declarator", source, &tokens);
}
