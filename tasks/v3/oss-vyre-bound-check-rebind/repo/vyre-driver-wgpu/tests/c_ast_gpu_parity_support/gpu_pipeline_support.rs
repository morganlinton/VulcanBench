use super::*;

pub(crate) fn assert_full_pipeline_parity(fix: &Fixture, label: &str) {
    let raw_cpu = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let raw_gpu = run_gpu_vast_builder(fix);
    assert_words_eq(
        &raw_gpu,
        &raw_cpu,
        &format!("{label}: raw VAST GPU/CPU parity"),
    );

    let annotated_cpu = reference_c11_annotate_typedef_names(&raw_cpu, fix.source.as_bytes());
    let annotated_gpu = run_gpu_typedef_annotation(fix, &raw_gpu);
    assert_words_eq(
        &annotated_gpu,
        &annotated_cpu,
        &format!("{label}: annotated VAST GPU/CPU parity"),
    );

    let typed_cpu = reference_c11_classify_vast_node_kinds(&annotated_cpu);
    let typed_gpu = run_gpu_classifier(&annotated_gpu);
    assert_words_eq(
        &typed_gpu,
        &typed_cpu,
        &format!("{label}: typed VAST GPU/CPU parity"),
    );
}

pub(crate) fn classify(fix: &Fixture) -> Vec<u8> {
    let raw = reference_c11_build_vast_nodes(&fix.tok_types, &fix.tok_starts, &fix.tok_lens);
    let annotated = reference_c11_annotate_typedef_names(&raw, fix.source.as_bytes());
    reference_c11_classify_vast_node_kinds(&annotated)
}

pub(crate) fn assert_gpu_pg_parity(fix: &Fixture, typed_vast: &[u8], label: &str) {
    let node_count = node_count_from_vast(typed_vast);
    let cpu_pg = reference_ast_to_pg_nodes(typed_vast);
    let gpu_pg = run_gpu_pg_lower_with_count(typed_vast, node_count);
    assert_eq!(
        gpu_pg,
        cpu_pg,
        "{label}: GPU PG lower output diverged from reference for {} bytes",
        fix.source.len()
    );
}

pub(crate) fn run_gpu_vast_builder_from_parts(
    tok_types: &[u32],
    tok_starts: &[u32],
    tok_lens: &[u32],
) -> Vec<u8> {
    let program = c11_build_vast_nodes(
        "tok_types",
        "tok_starts",
        "tok_lens",
        Expr::u32(tok_types.len() as u32),
        "out_vast_nodes",
        "out_count",
    );
    let tok_type_bytes = bytes(tok_types);
    let tok_start_bytes = bytes(tok_starts);
    let tok_len_bytes = bytes(tok_lens);
    let outputs = dispatch_gpu_program(
        "GPU C VAST builder",
        program,
        vec![tok_type_bytes, tok_start_bytes, tok_len_bytes],
    );
    assert_eq!(outputs.len(), 2, "expected [vast_nodes, count]");
    outputs[0].clone()
}

fn run_gpu_vast_builder(fix: &Fixture) -> Vec<u8> {
    run_gpu_vast_builder_from_parts(&fix.tok_types, &fix.tok_starts, &fix.tok_lens)
}

pub(crate) fn run_gpu_classifier(annotated_vast: &[u8]) -> Vec<u8> {
    run_gpu_classifier_with_count(annotated_vast, node_count_from_vast(annotated_vast))
}

pub(crate) fn run_gpu_classifier_with_count(annotated_vast: &[u8], num_nodes: u32) -> Vec<u8> {
    let program =
        c11_classify_vast_node_kinds("vast_nodes", Expr::u32(num_nodes), "typed_vast_nodes");
    let outputs = dispatch_gpu_program(
        "GPU VAST classifier",
        program,
        vec![annotated_vast.to_vec()],
    );
    assert_eq!(outputs.len(), 1);
    outputs[0].clone()
}

pub(crate) fn run_gpu_expr_shape(raw_vast: &[u8], typed_vast: &[u8]) -> Vec<u8> {
    let program = c11_build_expression_shape_nodes(
        "raw_vast_nodes",
        "typed_vast_nodes",
        Expr::u32(node_count_from_vast(raw_vast)),
        "expr_shape_nodes",
    );
    let outputs = dispatch_gpu_program(
        "GPU expression-shape lower",
        program,
        vec![raw_vast.to_vec(), typed_vast.to_vec()],
    );

    assert_eq!(outputs.len(), 1);
    outputs[0].clone()
}

pub(crate) fn run_gpu_pg_lower(typed_vast: &[u8]) -> Vec<u8> {
    run_gpu_pg_lower_with_count(typed_vast, node_count_from_vast(typed_vast))
}

pub(crate) fn run_gpu_pg_lower_with_count(typed_vast: &[u8], num_nodes: u32) -> Vec<u8> {
    let program = c_lower_ast_to_pg_nodes("vast_nodes", Expr::u32(num_nodes), "out_pg_nodes");
    let outputs = dispatch_gpu_program("GPU AST-to-PG lower", program, vec![typed_vast.to_vec()]);
    assert_eq!(outputs.len(), 1);
    outputs[0].clone()
}

pub(crate) fn run_gpu_semantic_pg_lower(typed_vast: &[u8]) -> (Vec<u8>, Vec<u8>) {
    run_gpu_semantic_pg_lower_with_count(typed_vast, node_count_from_vast(typed_vast))
}

pub(crate) fn run_gpu_semantic_pg_lower_with_count(
    typed_vast: &[u8],
    num_nodes: u32,
) -> (Vec<u8>, Vec<u8>) {
    let program = c_lower_ast_to_pg_semantic_graph(
        "vast_nodes",
        Expr::u32(num_nodes),
        "out_pg_nodes",
        "out_pg_edges",
    );
    let outputs = dispatch_gpu_program(
        "GPU semantic AST-to-PG lower",
        program,
        vec![typed_vast.to_vec()],
    );
    assert_eq!(outputs.len(), 2);
    (outputs[0].clone(), outputs[1].clone())
}

pub(crate) fn run_gpu_c_sema_scope_from_parts(
    tok_types: &[u32],
    tok_starts: &[u32],
    tok_lens: &[u32],
    haystack: &[u8],
) -> Vec<u8> {
    let program = c_sema_scope(
        "tok_types",
        "tok_starts",
        "tok_lens",
        "haystack",
        Expr::u32(haystack.len() as u32),
        Expr::u32(tok_types.len() as u32),
        "out_scope_tree",
    );
    let tok_type_bytes = bytes(tok_types);
    let tok_start_bytes = bytes(tok_starts);
    let tok_len_bytes = bytes(tok_lens);
    let haystack_bytes = haystack_words(haystack);
    let outputs = dispatch_gpu_program(
        "GPU C semantic scope",
        program,
        vec![
            tok_type_bytes,
            tok_start_bytes,
            tok_len_bytes,
            haystack_bytes,
        ],
    );
    assert_eq!(outputs.len(), 1);
    outputs[0].clone()
}
