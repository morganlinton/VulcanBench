use super::*;

fn possible_declarator_follower(kind: u32) -> bool {
    matches!(
        kind,
        TOK_SEMICOLON
            | TOK_COMMA
            | TOK_ASSIGN
            | TOK_LPAREN
            | TOK_LBRACKET
            | TOK_COLON
            | TOK_RPAREN
            | TOK_RBRACKET
    )
}

fn global_typedef_hashes_from_hashed_vast(hashed_vast: &[u8]) -> Vec<u32> {
    let rows = hashed_vast.len() / VAST_STRIDE_BYTES;
    let mut hashes = Vec::new();
    let mut in_typedef_decl = false;
    let mut typedef_brace_depth = 0u32;
    for row in 0..rows {
        let kind = word_at(hashed_vast, row * VAST_STRIDE_U32);
        if kind == TOK_TYPEDEF {
            in_typedef_decl = true;
            typedef_brace_depth = 0;
            continue;
        }
        if in_typedef_decl && kind == TOK_LBRACE {
            typedef_brace_depth = typedef_brace_depth.saturating_add(1);
            continue;
        }
        if in_typedef_decl && kind == TOK_RBRACE {
            typedef_brace_depth = typedef_brace_depth.saturating_sub(1);
            continue;
        }
        if in_typedef_decl && kind == TOK_IDENTIFIER {
            let next_kind = if row + 1 < rows {
                word_at(hashed_vast, (row + 1) * VAST_STRIDE_U32)
            } else {
                TOK_SEMICOLON
            };
            if typedef_brace_depth == 0 && possible_declarator_follower(next_kind) {
                let hash = word_at(
                    hashed_vast,
                    row * VAST_STRIDE_U32 + VAST_TYPEDEF_SYMBOL_FIELD,
                );
                if hash != 0 && !hashes.contains(&hash) {
                    hashes.push(hash);
                }
            }
        }
        if in_typedef_decl && typedef_brace_depth == 0 && kind == TOK_SEMICOLON {
            in_typedef_decl = false;
        }
    }
    if hashes.is_empty() {
        hashes.push(0);
    }
    hashes
}

pub(crate) fn run_gpu_full_typedef_annotation(source: &[u8], raw_vast: &[u8]) -> Vec<u8> {
    let haystack = haystack_words(source);
    let program = c11_annotate_typedef_names(
        "vast_nodes",
        "haystack",
        Expr::u32(source.len() as u32),
        Expr::u32(node_count_from_vast(raw_vast)),
        "annotated_vast",
    );
    let inputs: Vec<&[u8]> = vec![raw_vast, &haystack];
    let outputs = gpu_backend()
        .dispatch_borrowed(&program, &inputs, &Default::default())
        .expect("GPU full typedef annotation dispatch must succeed");
    assert_eq!(outputs.len(), 1);
    outputs[0].clone()
}

pub(crate) fn run_gpu_fast_typedef_annotation(source: &[u8], raw_vast: &[u8]) -> Vec<u8> {
    let haystack = haystack_words(source);
    let node_count_value = node_count_from_vast(raw_vast);
    let node_count = Expr::u32(node_count_value);
    let hashed_program = c11_prehash_vast_identifiers(
        "vast_nodes",
        "haystack",
        Expr::u32(source.len() as u32),
        node_count.clone(),
        "hashed_vast",
    );
    let hashed = dispatch_gpu_program(
        "GPU typedef prehash",
        hashed_program,
        vec![raw_vast.to_vec(), haystack.clone(), raw_vast.to_vec()],
    );
    assert_eq!(hashed.len(), 1);

    let scoped_program =
        c11_precompute_vast_scopes("hashed_vast", node_count.clone(), "scoped_vast");
    let scope_stack = vec![0u8; node_count_value.max(1) as usize * core::mem::size_of::<u32>()];
    let scoped = dispatch_gpu_program(
        "GPU typedef scope precompute",
        scoped_program,
        vec![hashed[0].clone(), hashed[0].clone(), scope_stack],
    );
    let scoped_vast =
        primary_output_with_optional_empty_scratch(scoped, "GPU typedef scope precompute");

    let typedef_hashes = global_typedef_hashes_from_hashed_vast(&scoped_vast);
    let typedef_hash_bytes = bytes(&typedef_hashes);
    let program = c11_annotate_global_typedef_names_fast(
        "vast_nodes",
        "global_typedef_hashes",
        node_count,
        Expr::u32(typedef_hashes.len() as u32),
        "annotated_vast",
    );
    let outputs = dispatch_gpu_program(
        "GPU typedef annotation",
        program,
        vec![scoped_vast.clone(), typedef_hash_bytes, scoped_vast],
    );
    assert_eq!(outputs.len(), 1);
    outputs[0].clone()
}

pub(crate) fn run_gpu_scoped_typedef_annotation(source: &[u8], raw_vast: &[u8]) -> Vec<u8> {
    let haystack = haystack_words(source);
    let node_count_value = node_count_from_vast(raw_vast);
    let node_count = Expr::u32(node_count_value);
    let hashed_program = c11_prehash_vast_identifiers(
        "vast_nodes",
        "haystack",
        Expr::u32(source.len() as u32),
        node_count.clone(),
        "hashed_vast",
    );
    let hashed = dispatch_gpu_program(
        "GPU scoped typedef prehash",
        hashed_program,
        vec![raw_vast.to_vec(), haystack.clone(), raw_vast.to_vec()],
    );
    assert_eq!(hashed.len(), 1);

    let scoped_program =
        c11_precompute_vast_scopes("hashed_vast", node_count.clone(), "scoped_vast");
    let scope_stack = vec![0u8; node_count_value.max(1) as usize * core::mem::size_of::<u32>()];
    let scoped = dispatch_gpu_program(
        "GPU scoped typedef scope precompute",
        scoped_program,
        vec![hashed[0].clone(), hashed[0].clone(), scope_stack],
    );
    let scoped_vast =
        primary_output_with_optional_empty_scratch(scoped, "GPU scoped typedef scope precompute");

    let program = c11_annotate_typedef_names_precomputed_scope(
        "vast_nodes",
        "haystack",
        Expr::u32(source.len() as u32),
        node_count,
        "annotated_vast",
    );
    let outputs = dispatch_gpu_program(
        "GPU scoped typedef annotation",
        program,
        vec![scoped_vast, haystack],
    );
    assert_eq!(outputs.len(), 1);
    outputs[0].clone()
}

pub(crate) fn run_gpu_typedef_annotation(fix: &Fixture, raw_vast: &[u8]) -> Vec<u8> {
    run_gpu_fast_typedef_annotation(fix.source.as_bytes(), raw_vast)
}
