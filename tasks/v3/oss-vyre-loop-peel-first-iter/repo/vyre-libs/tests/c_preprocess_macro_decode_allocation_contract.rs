//! Structural contracts for macro-expansion output decode allocation.

use std::fs;
use std::path::Path;

fn crate_file(path: &str) -> String {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(manifest.join(path)).unwrap_or_else(|error| {
        panic!("failed to read {path}: {error}");
    })
}

#[test]
fn macro_expansion_output_decode_uses_fallible_reservation() {
    let source =
        crate_file("src/parsing/c/preprocess/gpu_pipeline/macro_expansion/decode_outputs.rs");
    assert!(
        source.contains("fn reserve_macro_decode_vec_capacity"),
        "Fix: macro expansion output decode must centralize allocation failure reporting."
    );
    assert!(
        source.contains("try_reserve_exact(count)"),
        "Fix: decoded macro expansion token columns must reserve fallibly before push."
    );
    assert!(
        source.contains("reserve_macro_decode_vec_capacity(")
            && source.contains("&mut directive_kinds")
            && source.contains("directive_kinds.resize(token_count, 0)"),
        "Fix: generated directive-kind defaults must reserve fallibly before resize."
    );
    assert!(
        !source.contains("Vec::with_capacity(count)") && !source.contains("vec![0; token_count]"),
        "Fix: untrusted macro expansion output counts must not drive infallible allocation."
    );
}
