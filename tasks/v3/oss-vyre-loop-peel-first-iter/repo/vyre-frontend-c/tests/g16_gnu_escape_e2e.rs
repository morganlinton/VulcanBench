//! End-to-end coverage for GNU C escape sequences used by Linux sources.

use vyre_driver_cuda as _;
use vyre_driver_wgpu as _;
use vyre_frontend_c::api::parse_syntax_source;

#[test]
fn gnu_escape_e_is_accepted_in_string_literals() {
    let summary = parse_syntax_source(
        r#"
int main(void)
{
    const char *ansi = "\e[33m";
    return ansi[0];
}
"#,
    )
    .expect("GNU C escape \\e should parse in Linux-grade C sources");
    assert!(summary.token_count > 0);
    assert!(summary.ast_bytes > 0);
}
