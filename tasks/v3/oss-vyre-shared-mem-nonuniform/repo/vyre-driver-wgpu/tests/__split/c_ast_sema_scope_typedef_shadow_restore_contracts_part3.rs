use super::*;

#[test]
fn gpu_parity_annotation_deep_shadow_chain() {
    let mut atoms = vec![
        tok(TOK_TYPEDEF),
        tok(TOK_INT),
        ident("T"),
        tok(TOK_SEMICOLON),
        tok(TOK_VOID),
        ident("f"),
        tok(TOK_LPAREN),
        tok(TOK_VOID),
        tok(TOK_RPAREN),
        tok(TOK_LBRACE),
    ];
    for _ in 0..6 {
        atoms.push(tok(TOK_LBRACE));
        atoms.push(tok(TOK_INT));
        atoms.push(ident("T"));
        atoms.push(tok(TOK_SEMICOLON));
        atoms.push(tok(TOK_RBRACE));
    }
    atoms.push(tok(TOK_LPAREN));
    atoms.push(ident("T"));
    atoms.push(tok(TOK_RPAREN));
    atoms.push(tok(TOK_STAR));
    atoms.push(ident("p"));
    atoms.push(tok(TOK_SEMICOLON));
    atoms.push(tok(TOK_RBRACE));
    let fix = fixture("gpu_deep_ann", &atoms);
    let expected_ann = annotate_cpu(&fix);
    let gpu_ann = run_gpu_annotate(&fix);
    assert_eq!(
        gpu_ann, expected_ann,
        "GPU annotation must match CPU for deep shadow chain"
    );
}
