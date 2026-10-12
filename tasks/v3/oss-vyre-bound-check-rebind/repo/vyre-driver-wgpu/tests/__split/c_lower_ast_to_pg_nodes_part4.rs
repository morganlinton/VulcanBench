use super::*;

#[test]
fn gnu_c_stress_fixture_flows_from_lexer_to_typed_program_graph() {
    let (source, raw_kinds, tok_starts, tok_lens) = gnu_c_stress_fixture_source_and_tokens();
    let lexed_raw = lex_c11_max_munch_kinds(source.as_bytes())
        .expect("Fix: general GNU-C stress fixture must be accepted by the C11 lexer");
    let lexed_non_ws = lexed_raw
        .into_iter()
        .filter(|kind| *kind != TOK_WHITESPACE && *kind != TOK_COMMENT)
        .collect::<Vec<_>>();
    assert_eq!(
        lexed_non_ws, raw_kinds,
        "Fix: fixture token stream must be produced by the lexer before keyword promotion"
    );

    let tok_types =
        reference_c_keyword_types(&raw_kinds, &tok_starts, &tok_lens, source.as_bytes());
    let raw_vast = reference_c11_build_vast_nodes(&tok_types, &tok_starts, &tok_lens);
    let typed_vast = reference_c11_classify_vast_node_kinds(&raw_vast);
    let expected_pg = reference_ast_to_pg_nodes(&typed_vast);
    let program = c_lower_ast_to_pg_nodes(
        "vast_nodes",
        Expr::u32(node_count_from_vast(&typed_vast)),
        "pg_nodes",
    );
    let actual = run_reference_eval(&program, std::slice::from_ref(&typed_vast));
    assert_eq!(
        actual,
        vec![expected_pg.clone()],
        "Fix: lowered program graph must match the CPU oracle"
    );

    let typed_functions = row_indices(
        &typed_vast,
        VAST_STRIDE_U32 as usize,
        node_kind::FUNCTION_DECL,
    );
    let typed_function_defs = row_indices(
        &typed_vast,
        VAST_STRIDE_U32 as usize,
        C_AST_KIND_FUNCTION_DEFINITION,
    );
    let typed_calls = row_indices(&typed_vast, VAST_STRIDE_U32 as usize, node_kind::CALL);
    let typed_blocks = row_indices(
        &typed_vast,
        VAST_STRIDE_U32 as usize,
        node_kind::BASIC_BLOCK,
    );
    let typed_literals = row_indices(&typed_vast, VAST_STRIDE_U32 as usize, node_kind::LITERAL);
    let typed_attributes = row_indices(
        &typed_vast,
        VAST_STRIDE_U32 as usize,
        C_AST_KIND_GNU_ATTRIBUTE,
    );
    let typed_inline_asm =
        row_indices(&typed_vast, VAST_STRIDE_U32 as usize, C_AST_KIND_INLINE_ASM);
    let typed_asm_templates = row_indices(
        &typed_vast,
        VAST_STRIDE_U32 as usize,
        C_AST_KIND_ASM_TEMPLATE,
    );
    let typed_asm_clobbers = row_indices(
        &typed_vast,
        VAST_STRIDE_U32 as usize,
        C_AST_KIND_ASM_CLOBBERS_LIST,
    );
    let typed_if = row_indices(&typed_vast, VAST_STRIDE_U32 as usize, C_AST_KIND_IF_STMT);
    let typed_returns = row_indices(
        &typed_vast,
        VAST_STRIDE_U32 as usize,
        C_AST_KIND_RETURN_STMT,
    );
    let pg_functions = row_indices(
        &expected_pg,
        PG_STRIDE_U32 as usize,
        node_kind::FUNCTION_DECL,
    );
    let pg_function_defs = row_indices(
        &expected_pg,
        PG_STRIDE_U32 as usize,
        C_AST_KIND_FUNCTION_DEFINITION,
    );
    let pg_calls = row_indices(&expected_pg, PG_STRIDE_U32 as usize, node_kind::CALL);
    let pg_attributes = row_indices(
        &expected_pg,
        PG_STRIDE_U32 as usize,
        C_AST_KIND_GNU_ATTRIBUTE,
    );
    let pg_inline_asm = row_indices(&expected_pg, PG_STRIDE_U32 as usize, C_AST_KIND_INLINE_ASM);
    let pg_asm_templates = row_indices(
        &expected_pg,
        PG_STRIDE_U32 as usize,
        C_AST_KIND_ASM_TEMPLATE,
    );
    let pg_asm_clobbers = row_indices(
        &expected_pg,
        PG_STRIDE_U32 as usize,
        C_AST_KIND_ASM_CLOBBERS_LIST,
    );
    let pg_if = row_indices(&expected_pg, PG_STRIDE_U32 as usize, C_AST_KIND_IF_STMT);
    let pg_returns = row_indices(&expected_pg, PG_STRIDE_U32 as usize, C_AST_KIND_RETURN_STMT);

    assert_eq!(
        typed_functions,
        vec![3],
        "Fix: typedef prototype must remain a generic function declaration"
    );
    assert_eq!(
        typed_function_defs,
        vec![17],
        "Fix: attributed GNU-C-style definition must type as a first-class function definition"
    );
    assert_eq!(
        pg_functions, typed_functions,
        "Fix: typed function declaration rows must lower without kind drift"
    );
    assert_eq!(
        pg_function_defs, typed_function_defs,
        "Fix: typed function definition rows must lower without kind drift"
    );
    assert!(
        typed_calls.len() >= 3,
        "Fix: nested trace_fault, likely, and do_fault invocations must type as calls; got {typed_calls:?}"
    );
    assert_eq!(
        pg_calls, typed_calls,
        "Fix: typed call rows must lower without kind drift"
    );
    assert!(
        typed_blocks.len() >= 3,
        "Fix: outer body, nested compound block, and if body must type as basic blocks"
    );
    assert!(
        typed_literals.len() >= 2,
        "Fix: integer literals must remain typed"
    );
    assert_eq!(
        typed_attributes,
        vec![32],
        "Fix: GNU attributes must lower as first-class VAST nodes, not calls"
    );
    assert_eq!(
        pg_attributes, typed_attributes,
        "Fix: GNU attribute node kind must survive PG lowering"
    );
    assert_eq!(
        typed_inline_asm,
        vec![56],
        "Fix: inline asm must lower as a first-class VAST node, not a generic call"
    );
    assert_eq!(
        pg_inline_asm, typed_inline_asm,
        "Fix: inline asm node kind must survive PG lowering"
    );
    assert_eq!(
        typed_asm_templates,
        vec![59],
        "Fix: inline asm template string must become a first-class asm template node"
    );
    assert_eq!(
        typed_asm_clobbers,
        vec![63],
        "Fix: inline asm clobber string must become a first-class asm clobber node"
    );
    assert_eq!(
        pg_asm_templates, typed_asm_templates,
        "Fix: asm template node kind must survive PG lowering"
    );
    assert_eq!(
        pg_asm_clobbers, typed_asm_clobbers,
        "Fix: asm clobber node kind must survive PG lowering"
    );
    assert_eq!(
        typed_if,
        vec![46],
        "Fix: C if statements must be first-class typed VAST nodes"
    );
    assert_eq!(
        pg_if, typed_if,
        "Fix: C if statement node kind must survive PG lowering"
    );
    assert_eq!(
        typed_returns,
        vec![66, 75],
        "Fix: C return statements must be first-class typed VAST nodes"
    );
    assert_eq!(
        pg_returns, typed_returns,
        "Fix: C return statement node kinds must survive PG lowering"
    );

    assert_pg_row(
        &expected_pg,
        38,
        node_kind::BASIC_BLOCK,
        u32::MAX,
        39,
        u32::MAX,
    );
    assert_pg_row(&expected_pg, 39, node_kind::BASIC_BLOCK, 38, 40, 46);
    assert_pg_row(&expected_pg, 55, node_kind::BASIC_BLOCK, 38, 56, 75);
    assert_ne!(
        word_at(&expected_pg, 32 * PG_STRIDE_U32 as usize),
        node_kind::FUNCTION_DECL,
        "Fix: GNU attribute suffix must not lower as a fake function declaration"
    );
}

#[test]
fn ast_to_pg_nodes_emits_valid_wgsl() {
    let program = (entry().build)();
    let wgsl = emit_wgsl(&program);
    assert!(
        wgsl.contains("@compute"),
        "Fix: lowered WGSL must include a compute entry point"
    );
}

#[test]
fn ast_to_pg_nodes_gathered_for_parity_certificate() {
    let prog_wire = (entry().build)()
        .to_wire()
        .expect("Fix: build program wire");
    let program_hash = blake3::hash(&prog_wire).to_hex().to_string();
    let witness_inputs = (entry()
        .test_inputs
        .expect("Fix: test_inputs must be pinned"))();
    let witness_outputs = (entry()
        .expected_output
        .expect("Fix: expected_output must be pinned"))();

    let witness_input_bytes: Vec<u8> = witness_inputs
        .iter()
        .flat_map(|case| case.iter().flat_map(|buffer| buffer.iter().copied()))
        .collect();
    let witness_output_bytes: Vec<u8> = witness_outputs
        .iter()
        .flat_map(|case| case.iter().flat_map(|buffer| buffer.iter().copied()))
        .collect();

    let witness_input_hash = blake3::hash(&witness_input_bytes).to_hex().to_string();
    let witness_output_hash = blake3::hash(&witness_output_bytes).to_hex().to_string();

    assert_eq!(program_hash.len(), 64);
    assert_eq!(witness_input_hash.len(), 64);
    assert_eq!(witness_output_hash.len(), 64);
}

