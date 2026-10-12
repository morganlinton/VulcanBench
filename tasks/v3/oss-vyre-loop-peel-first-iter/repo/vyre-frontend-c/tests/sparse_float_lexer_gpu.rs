//! CUDA sparse lexer contracts for floating numeric literal spans.

mod support;

use support::{compile_source_with_resident, find_token};
use vyre_libs::parsing::c::lex::tokens::TOK_FLOAT;

#[test]
fn sparse_cuda_lexer_emits_float_tokens_with_full_spans() {
    let source = r#"
double a = 3.14;
double b = 3.;
double c = .5;
double d = 1e+10;
double e = 0x1.8p+2;
double f = .5e+2;
"#;

    let (object, resident) =
        compile_source_with_resident("sparse_float_lexer_gpu", source, Vec::new(), Vec::new());
    let lex = object.lex();

    for literal in ["3.14", "3.", ".5", "1e+10", "0x1.8p+2", ".5e+2"] {
        let token_idx = find_token(&resident, &lex.starts, &lex.lens, literal);
        assert_eq!(
            lex.tok_types[token_idx], TOK_FLOAT,
            "{literal} must be emitted as a single TOK_FLOAT by the sparse CUDA lexer"
        );
        assert_eq!(
            lex.lens[token_idx] as usize,
            literal.len(),
            "{literal} must preserve its full byte span"
        );
    }
}
