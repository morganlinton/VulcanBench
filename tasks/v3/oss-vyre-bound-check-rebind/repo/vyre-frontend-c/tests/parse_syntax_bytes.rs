//! GPU C syntax parser integration tests.

use std::sync::{Mutex, MutexGuard};

use vyre_driver_cuda as _;
use vyre_driver_wgpu as _;
use vyre_frontend_c::api::{
    parse_prepared_resident_syntax, parse_syntax_batch_bytes, parse_syntax_bytes,
    pipeline_cache_snapshot, prepare_resident_syntax_bytes,
};
use vyre_libs::parsing::c::pipeline::stages::C11_AST_MAX_TOK_SCAN;

fn parse_syntax_guard() -> MutexGuard<'static, ()> {
    static GUARD: Mutex<()> = Mutex::new(());
    GUARD
        .lock()
        .expect("parse_syntax_bytes CUDA integration-test mutex poisoned")
}

#[test]
fn parse_syntax_bytes_accepts_raw_c_source() {
    let _guard = parse_syntax_guard();
    let source = b"int main(void) { return 0; }\n";
    let summary = parse_syntax_bytes(source)
        .expect("raw-byte GPU syntax parse should accept a minimal C translation unit");
    assert!(
        summary.backend_id == "cuda",
        "syntax parser must execute on the CUDA release backend, got {}",
        summary.backend_id
    );
    assert_eq!(summary.source_bytes, source.len() as u64);
    assert!(summary.token_count > 0, "expected nonzero token count");
    assert!(summary.ast_bytes > 0, "expected AST evidence bytes");
    assert!(summary.ast_node_count > 0, "expected emitted AST nodes");
    assert_eq!(
        summary.ast_covered_tokens, summary.token_count,
        "syntax parser must build AST evidence for every token in the raw byte input"
    );
    assert!(
        summary.ast_window_count > 0,
        "expected at least one AST GPU window"
    );
}

#[test]
fn parse_syntax_bytes_preserves_non_utf8_source_length() {
    let _guard = parse_syntax_guard();
    let mut source = b"int x = 0;\n".to_vec();
    source.push(0xFF);
    source.extend_from_slice(b"\n");
    let result = parse_syntax_bytes(&source);
    match result {
        Ok(summary) => {
            assert!(
                summary.backend_id == "cuda",
                "syntax parser must execute on the CUDA release backend, got {}",
                summary.backend_id
            );
            assert_eq!(summary.source_bytes, source.len() as u64);
        }
        Err(message) => assert!(
            message.contains("lexer") || message.contains("parser") || message.contains("dispatch"),
            "raw-byte parser errors should come from GPU parse stages, got: {message}"
        ),
    }
}

#[test]
fn prepared_syntax_bytes_parse_non_utf8_without_lossy_repair() {
    let _guard = parse_syntax_guard();
    let mut source = b"/* raw ".to_vec();
    source.extend_from_slice(&[0xFF, 0xFE, 0x80]);
    source.extend_from_slice(b" bytes */\nint raw_bytes(void) { return 3; }\n");

    let prepared = prepare_resident_syntax_bytes(&source)
        .expect("prepare resident syntax bytes without UTF-8 repair");
    assert_eq!(prepared.source_bytes, source.len() as u64);
    assert!(
        prepared.haystack_len as usize >= source.len(),
        "prepared haystack must preserve raw byte capacity"
    );

    let summary = parse_prepared_resident_syntax(&prepared)
        .expect("GPU syntax parser should accept non-UTF8 bytes inside comments");
    assert_eq!(summary.backend_id, "cuda");
    assert_eq!(summary.source_bytes, source.len() as u64);
    assert!(summary.token_count > 0);
}

#[test]
fn parse_syntax_bytes_covers_multiple_ast_windows() {
    let _guard = parse_syntax_guard();
    let mut source = Vec::new();
    source.resize(C11_AST_MAX_TOK_SCAN as usize + 1024, b';');

    let summary = parse_syntax_bytes(&source)
        .expect("raw-byte GPU syntax parse should tile AST windows for large token streams");

    assert!(
        summary.backend_id == "cuda",
        "syntax parser must execute on the CUDA release backend, got {}",
        summary.backend_id
    );
    assert!(
        summary.token_count > C11_AST_MAX_TOK_SCAN,
        "test must exceed one AST window of {} tokens, got {} tokens",
        C11_AST_MAX_TOK_SCAN,
        summary.token_count
    );
    assert!(
        summary.ast_window_count > 1,
        "expected tiled AST GPU windows"
    );
    assert_eq!(
        summary.ast_covered_tokens, summary.token_count,
        "tiled AST parse must cover every token"
    );
    assert!(summary.ast_node_count > 0, "expected tiled AST nodes");
}

#[test]
fn repeated_parse_hits_resident_haystack_cache() {
    let _guard = parse_syntax_guard();
    let source = b"int cached(void) { return 7; }\n";
    let before = pipeline_cache_snapshot();
    let first = parse_syntax_bytes(source).expect("first raw-byte GPU syntax parse should succeed");
    let second =
        parse_syntax_bytes(source).expect("second raw-byte GPU syntax parse should succeed");
    let after = pipeline_cache_snapshot();
    assert_eq!(first.source_bytes, second.source_bytes);
    assert_eq!(first.token_count, second.token_count);
    assert!(
        after.haystack_misses > before.haystack_misses,
        "first parse should populate the resident haystack cache"
    );
    assert!(
        after.haystack_hits > before.haystack_hits,
        "second parse of the same raw bytes should hit the resident haystack cache"
    );
}

#[test]
fn prepared_syntax_source_reuses_packed_haystack() {
    let _guard = parse_syntax_guard();
    let source = b"int prepared(void) { return 11; }\n";
    let prepared = prepare_resident_syntax_bytes(source).expect("prepare resident syntax source");
    assert_eq!(prepared.source_bytes, source.len() as u64);
    assert!(prepared.haystack_len > 0);

    let first = parse_prepared_resident_syntax(&prepared)
        .expect("first prepared resident GPU syntax parse");
    let second = parse_prepared_resident_syntax(&prepared)
        .expect("second prepared resident GPU syntax parse");

    assert_eq!(first.backend_id, "cuda");
    assert_eq!(first.source_bytes, source.len() as u64);
    assert_eq!(first.token_count, second.token_count);
    assert_eq!(first.ast_covered_tokens, first.token_count);
    assert_eq!(second.ast_covered_tokens, second.token_count);
}

#[test]
fn parse_syntax_batch_bytes_amortizes_many_loaded_files_on_cuda() {
    let _guard = parse_syntax_guard();
    let files: [&[u8]; 4] = [
        b"int a(void) { return 1; }\n",
        b"int b(void) { return 2; }\n",
        b"int c(void) { return 3; }\n",
        b"int d(void) { return 4; }\n",
    ];

    let summary =
        parse_syntax_batch_bytes(&files).expect("batched raw-byte GPU syntax parse should succeed");

    assert_eq!(summary.backend_id, "cuda");
    assert_eq!(summary.file_count, files.len() as u32);
    assert_eq!(
        summary.source_bytes,
        files.iter().map(|source| source.len() as u64).sum::<u64>()
    );
    assert_eq!(
        summary.batch_bytes,
        summary.source_bytes + files.len().saturating_sub(1) as u64
    );
    assert!(summary.token_count > 0);
    assert_eq!(summary.ast_covered_tokens, summary.token_count);
    assert!(summary.ast_window_count > 0);
}

#[test]
fn parse_syntax_batch_bytes_chunks_unsafe_loaded_files_on_cuda() {
    let _guard = parse_syntax_guard();
    let mut files = Vec::new();
    for index in 0..18 {
        files.push(format!("int unsafe_{index}(void) {{ return '\\\\n'; }}\n").into_bytes());
    }
    let refs = files.iter().map(Vec::as_slice).collect::<Vec<&[u8]>>();

    let summary = parse_syntax_batch_bytes(&refs)
        .expect("unsafe batched raw-byte GPU syntax parse should shard and succeed");
    let source_bytes = refs.iter().map(|source| source.len() as u64).sum::<u64>();

    assert_eq!(summary.backend_id, "cuda");
    assert_eq!(summary.file_count, refs.len() as u32);
    assert_eq!(summary.source_bytes, source_bytes);
    assert!(
        summary.batch_bytes >= summary.source_bytes,
        "chunked resident bytes must include every source byte"
    );
    assert!(
        summary.batch_bytes <= summary.source_bytes + refs.len().saturating_sub(1) as u64,
        "chunked resident bytes must not exceed a single concatenated batch"
    );
    assert!(summary.token_count > 0);
    assert_eq!(summary.ast_covered_tokens, summary.token_count);
    assert!(summary.ast_window_count > 0);
}
