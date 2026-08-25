//! Validate that c11_lexer lowers cleanly through the Naga backend.

#![cfg(feature = "c-parser")]

use vyre_libs::parsing::c::lex::lexer::c11_lexer;

#[test]
fn c11_lexer_naga_validates() {
    let prog = c11_lexer(
        "haystack",
        "out_tok_types",
        "out_tok_starts",
        "out_tok_lens",
        "out_counts",
        256,
    );
    let lk = vyre_lower::lower_for_emit(&prog).expect("c11_lexer must lower to KernelDescriptor");
    let module = vyre_emit_naga::emit(&lk.descriptor).expect("c11_lexer must emit to Naga module");
    let res = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module);
    if let Err(err) = res {
        panic!("c11_lexer naga validation failed: {err:?}");
    }
}
