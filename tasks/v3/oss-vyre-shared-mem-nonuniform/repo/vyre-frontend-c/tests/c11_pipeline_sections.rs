//! C11 object pipeline: embedded lex, VAST, ProgramGraph, and semantic graph sections.
#![allow(deprecated)]

mod support;

use support::*;
use vyre_libs::parsing::c::lex::tokens::TOK_IDENTIFIER;
use vyre_libs::parsing::c::sema::registry::reference_scope_tree;
use vyre_primitives::reduce::multi_block_prefix_scan::BLOCK_LANES;

#[test]
fn compile_c11_embeds_sema_scope_section() {
    let (object, resident) =
        compile_source_with_resident("sema_scope", SOURCE, Vec::new(), Vec::new());
    object.assert_elf();
    assert_eq!(object.version(), 7);

    let lex = object.lex();
    let haystack: Vec<u32> = resident.bytes().map(u32::from).collect();
    let expected = reference_scope_tree(&lex.tok_types, &lex.starts, &lex.lens, &haystack);
    assert_eq!(expected.len(), lex.tok_types.len() * 4);
    let mut expected_object = Vec::with_capacity(lex.tok_types.len() * SEMA_STRIDE_U32);
    for row in 0..lex.tok_types.len() {
        let base = row * 4;
        expected_object.extend_from_slice(&expected[base..base + 4]);
        expected_object.push(lex.starts[row]);
        expected_object.push(lex.lens[row]);
    }
    assert_eq!(
        object.section(SECTION_SEMA_SCOPE),
        u32_words_to_bytes(&expected_object)
    );
}

#[test]
fn compile_kernel_libc_shaped_translation_unit_reaches_all_pipeline_sections() {
    let (object, prepared_source) = compile_source_with_resident(
        "kernel_libc_shaped",
        KERNEL_LIBC_SHAPED_SOURCE,
        vec![("CLI_DEFINED".to_string(), Some("13".to_string()))],
        Vec::new(),
    );
    object.assert_elf();
    for tag in [
        SECTION_LEX,
        SECTION_PAREN_PAIRS,
        SECTION_BRACE_PAIRS,
        SECTION_FUNCTIONS,
        SECTION_CALLS,
        SECTION_PREPROC_MASK,
        SECTION_MACRO_TYPES,
        SECTION_AST,
        SECTION_CFG,
        SECTION_VAST,
        SECTION_EXPRESSION_SHAPE,
        SECTION_PROGRAM_GRAPH,
        SECTION_SEMANTIC_PROGRAM_GRAPH_NODES,
        SECTION_SEMANTIC_PROGRAM_GRAPH_EDGES,
        SECTION_SEMA_SCOPE,
    ] {
        assert!(
            !object.section(tag).is_empty(),
            "VYRECOB2 section {tag} is non-empty"
        );
    }

    let lex = object.lex();
    assert!(
        !lex.tok_types.is_empty(),
        "real translation unit produced a lexed token stream"
    );
    for (idx, ((&token_type, &start), &len)) in lex
        .tok_types
        .iter()
        .zip(&lex.starts)
        .zip(&lex.lens)
        .enumerate()
    {
        let start = start as usize;
        let end = start.saturating_add(len as usize);
        assert!(
            len > 0 && end <= prepared_source.len(),
            "lex section span {idx} stays inside the prepared translation unit: token_type={token_type} start={start} len={len} end={end} source_len={}",
            prepared_source.len()
        );
    }

    assert_eq!(
        object.words(SECTION_MACRO_TYPES),
        lex.tok_types,
        "empty macro expansion table preserves the real token stream"
    );

    let mask = object.words(SECTION_PREPROC_MASK);
    assert_eq!(mask.len(), lex.tok_types.len());
    assert!(
        mask.iter().all(|&word| word == 1),
        "conditional preprocessor baseline keeps every token active"
    );

    let sema_words = object.words(SECTION_SEMA_SCOPE);
    assert_eq!(sema_words.len(), lex.tok_types.len() * SEMA_STRIDE_U32);
    assert_eq!(
        object.words(SECTION_SEMANTIC_PROGRAM_GRAPH_NODES).len(),
        lex.tok_types.len() * SEMANTIC_PG_NODE_STRIDE_U32
    );
    assert_eq!(
        object.words(SECTION_SEMANTIC_PROGRAM_GRAPH_EDGES).len(),
        lex.tok_types.len() * SEMANTIC_PG_EDGE_ROWS_PER_NODE * SEMANTIC_PG_EDGE_STRIDE_U32
    );
    for (idx, &token_type) in lex.tok_types.iter().enumerate() {
        let intern_id = sema_words[idx * SEMA_STRIDE_U32 + 3];
        if token_type == TOK_IDENTIFIER {
            assert_ne!(
                intern_id, 0,
                "semantic scope pass interns emitted identifier token {idx}"
            );
        } else {
            assert_eq!(
                intern_id, 0,
                "semantic scope pass leaves non-identifier token {idx} uninterned"
            );
        }
    }

    assert_eq!(object.words(SECTION_PAREN_PAIRS).len(), lex.tok_types.len());
    assert_eq!(object.words(SECTION_BRACE_PAIRS).len(), lex.tok_types.len());
}

#[test]
fn compile_large_translation_unit_covers_multi_block_sparse_scan() {
    let mut source = String::from("int large_seed = 0;\n");
    for i in 0..512u32 {
        source.push_str(&format!("int large_decl_{i} = {i};\n"));
    }

    let (object, prepared_source) =
        compile_source_with_resident("large_sparse_scan", &source, Vec::new(), Vec::new());
    object.assert_elf();
    assert!(
        prepared_source.len() as u32 > BLOCK_LANES,
        "fixture must exceed one sparse-scan block"
    );

    let lex = object.lex();
    assert!(
        lex.tok_types.len() > BLOCK_LANES as usize,
        "large fixture produces enough sparse rows to exercise multi-block compaction"
    );
    assert_eq!(
        object.words(SECTION_PREPROC_MASK).len(),
        lex.tok_types.len()
    );
    assert_eq!(
        object.words(SECTION_SEMA_SCOPE).len(),
        lex.tok_types.len() * SEMA_STRIDE_U32
    );
}
