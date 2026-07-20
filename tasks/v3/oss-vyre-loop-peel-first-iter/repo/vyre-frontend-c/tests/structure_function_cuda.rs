//! CUDA regression coverage for C structure function extraction.

use vyre::ir::BufferAccess;
use vyre::ir::Expr;
use vyre::DispatchConfig;
use vyre_driver_cuda::cuda_factory;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::parse::structure::{c11_extract_calls, c11_extract_functions};
use vyre_libs::parsing::c::parse::vast::{
    c11_build_vast_nodes, c11_build_vast_nodes_uses_global_last_child,
    c11_classify_annotated_vast_node_kinds_precomputed_context, c11_precompute_vast_scopes,
    c11_precompute_vast_scopes_uses_global_stack, c11_prehash_vast_identifiers_packed_haystack,
};

fn u32_bytes(words: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(words.len() * 4);
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes
}

fn read_u32(buf: &[u8], word_index: usize) -> u32 {
    let offset = word_index * 4;
    u32::from_le_bytes(
        buf[offset..offset + 4]
            .try_into()
            .expect("u32 word is present"),
    )
}

fn mark_single_output(mut program: vyre::ir::Program, name: &str) -> vyre::ir::Program {
    for buffer in std::sync::Arc::make_mut(&mut program.buffers) {
        if buffer.name.as_ref() == name {
            buffer.access = BufferAccess::ReadWrite;
            buffer.pipeline_live_out = true;
            buffer.is_output = true;
        }
    }
    program
}

#[test]
fn cuda_c11_build_vast_nodes_initializes_every_sequential_row() {
    let tok_types: Vec<u32> = (0..35).map(|idx| TOK_INT + (idx % 3)).collect();
    let starts: Vec<u32> = (0..35).map(|idx| idx * 3).collect();
    let lens: Vec<u32> = vec![2; tok_types.len()];
    let nt = tok_types.len() as u32;
    let program = c11_build_vast_nodes(
        "tok_types",
        "tok_starts",
        "tok_lens",
        Expr::u32(nt),
        "out_vast_nodes",
        "out_count",
    );
    let backend = cuda_factory().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let config = DispatchConfig::default();
    let token_bytes = u32_bytes(&tok_types);
    let start_bytes = u32_bytes(&starts);
    let len_bytes = u32_bytes(&lens);
    let last_child_bytes = vec![0u8; nt as usize * 4];
    let stack_bytes = vec![0u8; nt as usize * 4];
    let inputs3 = [
        token_bytes.as_slice(),
        start_bytes.as_slice(),
        len_bytes.as_slice(),
    ];
    let inputs5 = [
        token_bytes.as_slice(),
        start_bytes.as_slice(),
        len_bytes.as_slice(),
        last_child_bytes.as_slice(),
        stack_bytes.as_slice(),
    ];
    let inputs: &[&[u8]] = if c11_build_vast_nodes_uses_global_last_child(nt) {
        &inputs5
    } else {
        &inputs3
    };
    let mut outputs: Vec<Vec<u8>> = Vec::new();

    backend
        .dispatch_borrowed_into(&program, inputs, &config, &mut outputs)
        .expect("Fix: CUDA raw VAST construction must initialize every row.");

    assert_eq!(outputs.len(), 2);
    for idx in 0..tok_types.len() {
        let base = idx * 10;
        assert_eq!(read_u32(&outputs[0], base), tok_types[idx]);
        assert_eq!(read_u32(&outputs[0], base + 5), starts[idx]);
        assert_eq!(read_u32(&outputs[0], base + 6), lens[idx]);
    }
    assert_eq!(read_u32(&outputs[1], 0), nt);
}

#[test]
fn cuda_c11_prehash_vast_identifiers_preserves_span_columns() {
    let node_count = 35u32;
    let mut vast_words = Vec::with_capacity(node_count as usize * 10);
    for node in 0..node_count {
        vast_words.extend_from_slice(&[
            TOK_INT + node,
            u32::MAX,
            u32::MAX,
            u32::MAX,
            u32::MAX,
            node * 3,
            2,
            0,
            0,
            0,
        ]);
    }
    let program = mark_single_output(
        c11_prehash_vast_identifiers_packed_haystack(
            "vast_nodes",
            "haystack",
            Expr::u32(1),
            Expr::u32(node_count),
            "hashed_vast",
        ),
        "hashed_vast",
    );
    let backend = cuda_factory().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let config = DispatchConfig::default();
    let vast_bytes = u32_bytes(&vast_words);
    let haystack_bytes = vec![0u8; 4];
    let inputs = [vast_bytes.as_slice(), haystack_bytes.as_slice()];
    let mut outputs: Vec<Vec<u8>> = Vec::new();

    backend
        .dispatch_borrowed_into(&program, &inputs, &config, &mut outputs)
        .expect("Fix: CUDA VAST prehash must preserve non-symbol columns.");

    assert_eq!(outputs.len(), 1);
    for node in 0..node_count as usize {
        let base = node * 10;
        assert_eq!(read_u32(&outputs[0], base + 5), node as u32 * 3);
        assert_eq!(read_u32(&outputs[0], base + 6), 2);
    }
}

#[test]
fn cuda_c11_classify_vast_preserves_span_columns() {
    let node_count = 35u32;
    let mut vast_words = Vec::with_capacity(node_count as usize * 10);
    for node in 0..node_count {
        vast_words.extend_from_slice(&[
            TOK_INT + (node % 3),
            u32::MAX,
            u32::MAX,
            u32::MAX,
            u32::MAX,
            node * 3,
            2,
            0,
            u32::MAX,
            0,
        ]);
    }
    let decl_contexts = vec![0u32; node_count as usize * 4];
    let program = mark_single_output(
        c11_classify_annotated_vast_node_kinds_precomputed_context(
            "annotated_vast",
            "decl_contexts",
            Expr::u32(node_count),
            "typed_vast_nodes",
        ),
        "typed_vast_nodes",
    );
    let backend = cuda_factory().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let config = DispatchConfig::default();
    let vast_bytes = u32_bytes(&vast_words);
    let context_bytes = u32_bytes(&decl_contexts);
    let inputs = [vast_bytes.as_slice(), context_bytes.as_slice()];
    let mut outputs: Vec<Vec<u8>> = Vec::new();

    backend
        .dispatch_borrowed_into(&program, &inputs, &config, &mut outputs)
        .expect("Fix: CUDA VAST classification must preserve non-kind columns.");

    assert_eq!(outputs.len(), 1);
    for node in 0..node_count as usize {
        let base = node * 10;
        assert_eq!(read_u32(&outputs[0], base + 5), node as u32 * 3);
        assert_eq!(read_u32(&outputs[0], base + 6), 2);
        assert_eq!(read_u32(&outputs[0], base + 7), 0);
    }
}

#[test]
fn cuda_c11_extract_functions_handles_sparse_function_records() {
    let tok_types = [
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_RBRACE,
    ];
    let paren_pairs = [u32::MAX, u32::MAX, 4, u32::MAX, 2, u32::MAX, u32::MAX];
    let brace_pairs = [u32::MAX, u32::MAX, u32::MAX, u32::MAX, u32::MAX, 6, 5];
    let nt = tok_types.len() as u32;
    let program = c11_extract_functions(
        "tok_types",
        "paren_pairs",
        "brace_pairs",
        Expr::u32(nt),
        "out_functions",
        "out_counts",
    );
    let backend = cuda_factory().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let mut config = DispatchConfig::default();
    config.grid_override = Some([nt.div_ceil(256), 1, 1]);
    config.label = Some("cuda c11 extract functions regression".to_owned());
    let token_bytes = u32_bytes(&tok_types);
    let paren_bytes = u32_bytes(&paren_pairs);
    let brace_bytes = u32_bytes(&brace_pairs);
    let count_bytes = 0u32.to_le_bytes();
    let inputs = [
        token_bytes.as_slice(),
        paren_bytes.as_slice(),
        brace_bytes.as_slice(),
        count_bytes.as_slice(),
    ];
    let mut outputs: Vec<Vec<u8>> = Vec::new();

    backend
        .dispatch_borrowed_into(&program, &inputs, &config, &mut outputs)
        .expect("Fix: CUDA function extraction must not fault on sparse record writes.");

    assert_eq!(outputs.len(), 2);
    assert_eq!(read_u32(&outputs[1], 0), nt * 3);
    assert_eq!(
        (
            read_u32(&outputs[0], 3),
            read_u32(&outputs[0], 4),
            read_u32(&outputs[0], 5),
        ),
        (1, 5, 6)
    );
}

#[test]
fn cuda_c11_extract_calls_reads_three_word_function_records_without_vector_fault() {
    let tok_types = [
        TOK_INT,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_VOID,
        TOK_RPAREN,
        TOK_LBRACE,
        TOK_IDENTIFIER,
        TOK_LPAREN,
        TOK_RPAREN,
        TOK_SEMICOLON,
        TOK_RBRACE,
    ];
    let paren_pairs = [
        u32::MAX,
        u32::MAX,
        4,
        u32::MAX,
        2,
        u32::MAX,
        u32::MAX,
        8,
        7,
        u32::MAX,
        u32::MAX,
    ];
    let function_records = [1, 5, 10];
    let nt = tok_types.len() as u32;
    let program = c11_extract_calls(
        "tok_types",
        "paren_pairs",
        "functions",
        Expr::u32(nt),
        Expr::u32(1),
        "out_calls",
        "out_counts",
    );
    let backend = cuda_factory().expect("Fix: CUDA backend must acquire on the GPU-required host.");
    let mut config = DispatchConfig::default();
    config.grid_override = Some([nt.div_ceil(256), 1, 1]);
    config.label = Some("cuda c11 extract calls regression".to_owned());
    let token_bytes = u32_bytes(&tok_types);
    let paren_bytes = u32_bytes(&paren_pairs);
    let function_bytes = u32_bytes(&function_records);
    let count_bytes = 0u32.to_le_bytes();
    let inputs = [
        token_bytes.as_slice(),
        paren_bytes.as_slice(),
        function_bytes.as_slice(),
        count_bytes.as_slice(),
    ];
    let mut outputs: Vec<Vec<u8>> = Vec::new();

    backend
        .dispatch_borrowed_into(&program, &inputs, &config, &mut outputs)
        .expect("Fix: CUDA call extraction must not fault on three-word function records.");

    assert_eq!(outputs.len(), 2);
    assert_eq!(read_u32(&outputs[1], 0), nt * 4);
    let base = 6 * 4;
    assert_eq!(
        (
            read_u32(&outputs[0], base as usize),
            read_u32(&outputs[0], base as usize + 1),
            read_u32(&outputs[0], base as usize + 2),
            read_u32(&outputs[0], base as usize + 3),
        ),
        (0, 6, 7, 8)
    );
}

#[test]
fn cuda_c11_precompute_vast_scopes_copies_stride_ten_rows_without_vector_fault() {
    for node_count in [35u32, 154u32] {
        let mut vast_words = Vec::with_capacity(node_count as usize * 10);
        for node in 0..node_count {
            let kind = if node == 0 {
                TOK_LBRACE
            } else if node == node_count - 1 {
                TOK_RBRACE
            } else {
                TOK_IDENTIFIER
            };
            vast_words.extend_from_slice(&[kind, 0, 0, 0, 0, 0, 0, 0, u32::MAX, 0]);
        }
        let program = mark_single_output(
            c11_precompute_vast_scopes("vast_nodes", Expr::u32(node_count), "scoped_vast"),
            "scoped_vast",
        );
        let backend =
            cuda_factory().expect("Fix: CUDA backend must acquire on the GPU-required host.");
        let config = DispatchConfig::default();
        let vast_bytes = u32_bytes(&vast_words);
        let stack_bytes = vec![0u8; node_count as usize * 4];
        let inputs1 = [vast_bytes.as_slice()];
        let inputs2 = [vast_bytes.as_slice(), stack_bytes.as_slice()];
        let inputs: &[&[u8]] = if c11_precompute_vast_scopes_uses_global_stack(node_count) {
            &inputs2
        } else {
            &inputs1
        };
        let mut outputs: Vec<Vec<u8>> = Vec::new();

        backend
            .dispatch_borrowed_into(&program, inputs, &config, &mut outputs)
            .expect("Fix: CUDA VAST scope precompute must not fault on stride-ten rows.");
        outputs.retain(|output| !output.is_empty());

        assert_eq!(outputs.len(), 1);
        assert_eq!(read_u32(&outputs[0], 8), u32::MAX);
        assert_eq!(read_u32(&outputs[0], 18), 0);
        assert_eq!(
            read_u32(&outputs[0], ((node_count - 1) * 10 + 8) as usize),
            0
        );
    }
}
