//! Golden regression test for C11 max-munch lexing of `corpus/hello.c`.

use vyre_grammar_gen::host_preprocess::preprocess_c_host;
use vyre_grammar_gen::kinds_blake3;
use vyre_grammar_gen::lex_c11_max_munch_kinds;

#[test]
fn hello_max_munch_blake3() {
    let src = preprocess_c_host(include_str!("../corpus/hello.c"));
    let kinds = lex_c11_max_munch_kinds(src.as_bytes()).expect("lex");
    assert_eq!(
        kinds_blake3(&kinds).to_hex().as_str(),
        "905d30376cbd7fb35563216587a90ace38688e33e5d597a31c999df7bce4b15e"
    );
    assert_eq!(
        kinds,
        vec![107, 201, 1, 10, 109, 11, 201, 12, 201, 104, 201, 2, 16, 201, 13, 201]
    );
}
