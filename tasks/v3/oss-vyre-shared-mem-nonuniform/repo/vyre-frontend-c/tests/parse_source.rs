//! GPU C parser/sema integration tests for the pre-lowering parse API.

use vyre_driver_cuda as _;
use vyre_driver_wgpu as _;
use vyre_frontend_c::api::parse_source;
use vyre_frontend_c::pipeline::preferred_backend_id;

#[test]
fn parse_source_uses_same_cuda_release_path_for_small_translation_unit() {
    let source = "int main(void) { return 0; }\n";
    assert!(
        source.len() <= 4096,
        "test fixture must stay below the former parser fast-path threshold"
    );
    assert_eq!(
        preferred_backend_id().expect("GPU backend should be selectable"),
        "cuda",
        "parser release path must select CUDA"
    );

    let summary = parse_source(source)
        .expect("parser/sema GPU C spine should parse a small translation unit");

    assert_eq!(summary.source_bytes, source.len() as u64);
    assert!(summary.token_count > 0, "expected nonzero token count");
    assert!(summary.ast_bytes > 0, "expected AST evidence bytes");
    assert!(
        summary.function_record_bytes >= 3 * 4,
        "expected function extraction evidence"
    );
    assert!(
        summary.call_record_bytes > 0,
        "expected call extraction evidence"
    );
}

#[test]
fn parse_source_uses_cuda_release_backend_on_large_c_translation_unit() {
    let mut source = String::from("static int sink;\n");
    for index in 0..2048u32 {
        source.push_str("static int f");
        source.push_str(&index.to_string());
        source.push_str("(int x) { sink += x; return sink + ");
        source.push_str(&(index % 17).to_string());
        source.push_str("; }\n");
    }
    source.push_str("int main(void) { return f0(1) + f2047(2); }\n");

    assert!(
        source.len() > 4096,
        "test fixture must exercise the sparse large-input lexer path"
    );
    assert_eq!(
        preferred_backend_id().expect("GPU backend should be selectable"),
        "cuda",
        "parser release path must select CUDA"
    );

    let summary = parse_source(&source)
        .expect("parser/sema GPU C spine should parse a large generated translation unit");

    assert_eq!(summary.source_bytes, source.len() as u64);
    assert!(summary.token_count > 4096, "expected thousands of tokens");
    assert!(summary.ast_bytes > 0, "expected AST evidence bytes");
    assert!(
        summary.function_record_bytes >= 2048 * 3 * 4,
        "expected function extraction evidence for generated functions, got {} bytes",
        summary.function_record_bytes
    );
    assert!(
        summary.call_record_bytes > 0,
        "expected call extraction evidence for main calls"
    );
}

#[test]
fn parse_source_handles_local_typedef_name_shadowing() {
    let source = r#"
typedef int T;
static int f(void) {
    int T = 7;
    return T;
}
"#;
    assert_eq!(
        preferred_backend_id().expect("GPU backend should be selectable"),
        "cuda",
        "parser release path must select CUDA"
    );

    let summary = parse_source(source)
        .expect("GPU C spine should parse a local ordinary declaration that shadows a typedef");

    assert_eq!(summary.source_bytes, source.len() as u64);
    assert!(summary.token_count > 0, "expected nonzero token count");
    assert!(summary.ast_bytes > 0, "expected AST evidence bytes");
    assert!(summary.vast_bytes > 0, "expected typed VAST evidence bytes");
    assert!(
        summary.semantic_node_bytes > 0,
        "expected semantic node evidence bytes"
    );
    assert!(
        summary.semantic_edge_bytes > 0,
        "expected semantic edge evidence bytes"
    );
    assert!(
        summary.sema_scope_bytes > 0,
        "expected semantic scope evidence bytes"
    );
}
