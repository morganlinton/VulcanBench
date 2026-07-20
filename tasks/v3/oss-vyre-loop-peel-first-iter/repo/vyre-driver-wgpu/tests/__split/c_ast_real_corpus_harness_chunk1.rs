// Real-world C AST corpus harness.
// Inspired by kernel, libc, and sqlite patterns.
// Lexes, builds VAST, annotates, classifies, lowers to PG, and checks reference/GPU parity.

// cfg(feature = "c-parser")  -  moved to parent

#[path = "../c_ast_gpu_parity_support/mod.rs"]
mod c_ast_gpu_parity_support;

use c_ast_gpu_parity_support::{dispatch_gpu_program, run_gpu_fast_typedef_annotation};
use vyre::ir::Expr;
use vyre_libs::parsing::c::lex::keyword::reference_c_keyword_types;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::lower::{c_lower_ast_to_pg_nodes, reference_ast_to_pg_nodes};
use vyre_libs::parsing::c::parse::vast::{
    c11_annotate_typedef_names, c11_build_vast_nodes, c11_classify_vast_node_kinds,
    reference_c11_annotate_typedef_names, reference_c11_build_vast_nodes,
    reference_c11_classify_vast_node_kinds,
};
use vyre_reference::value::Value;

const VAST_STRIDE_U32: usize = 10;

fn bytes(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}

fn haystack_words(bytes: &[u8]) -> Vec<u8> {
    vyre_primitives::wire::pack_bytes_as_u32_slice(bytes)
}

fn word_at(buf: &[u8], word: usize) -> u32 {
    let offset = word * 4;
    u32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap())
}

fn node_count_from_vast(rows: &[u8]) -> u32 {
    (rows.len() / (VAST_STRIDE_U32 * 4)) as u32
}

fn assert_parity(actual: &[u8], expected: &[u8], stage: &str, name: &str) {
    if actual == expected {
        return;
    }
    let actual_words = actual.len() / 4;
    let expected_words = expected.len() / 4;
    let limit = actual_words.min(expected_words);
    for word in 0..limit {
        let actual_word = word_at(actual, word);
        let expected_word = word_at(expected, word);
        if actual_word != expected_word {
            panic!(
                "Parity mismatch in {} stage for {}: word {} differs: actual={}, expected={}, row={}, field={}",
                stage, name, word, actual_word, expected_word, word / VAST_STRIDE_U32, word % VAST_STRIDE_U32
            );
        }
    }
    panic!(
        "Parity mismatch in {} stage for {}: lengths differ: actual={}, expected={}",
        stage,
        name,
        actual.len(),
        expected.len()
    );
}

fn run_reference_eval(program: &vyre::ir::Program, inputs: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let values = inputs.iter().cloned().map(Value::from).collect::<Vec<_>>();
    vyre_reference::reference_eval(program, &values)
        .expect("reference eval failed")
        .into_iter()
        .map(|v| v.to_bytes())
        .collect()
}

fn run_harness(name: &str, source: &str, tokens: &[(&str, u32)]) {
    let mut tok_starts = Vec::new();
    let mut tok_lens = Vec::new();
    let mut raw_kinds = Vec::new();
    let mut current_source = String::new();

    for (lexeme, kind) in tokens {
        tok_starts.push(current_source.len() as u32);
        tok_lens.push(lexeme.len() as u32);
        raw_kinds.push(*kind);
        current_source.push_str(lexeme);
    }

    assert_eq!(current_source, source, "Source mismatch for {}", name);

    let filtered_indices: Vec<usize> = raw_kinds
        .iter()
        .enumerate()
        .filter(|(_, &k)| k != TOK_WHITESPACE && k != TOK_COMMENT)
        .map(|(i, _)| i)
        .collect();

    let f_tok_starts: Vec<u32> = filtered_indices.iter().map(|&i| tok_starts[i]).collect();
    let f_tok_lens: Vec<u32> = filtered_indices.iter().map(|&i| tok_lens[i]).collect();
    let f_raw_kinds: Vec<u32> = filtered_indices.iter().map(|&i| raw_kinds[i]).collect();

    let tok_types =
        reference_c_keyword_types(&f_raw_kinds, &f_tok_starts, &f_tok_lens, source.as_bytes());

    // 1. Build VAST
    let vast_cpu = reference_c11_build_vast_nodes(&tok_types, &f_tok_starts, &f_tok_lens);
    let prog_vast = c11_build_vast_nodes(
        "types",
        "starts",
        "lens",
        Expr::u32(tok_types.len() as u32),
        "out",
        "count",
    );

    let out_vast_len = tok_types.len() * VAST_STRIDE_U32 * 4;
    let out_count_len = 4;
    let vast_inputs = vec![
        bytes(&tok_types),
        bytes(&f_tok_starts),
        bytes(&f_tok_lens),
        vec![0; out_vast_len],
        vec![0; out_count_len],
    ];

    let vast_gpu_outputs = dispatch_gpu_program("real corpus VAST build", prog_vast.clone(), vast_inputs.clone());
    // Bindings 3 and 4 are ReadWrite. So outputs[0] is binding 3, outputs[1] is binding 4.
    let vast_gpu = vast_gpu_outputs[0].clone();
    assert_parity(&vast_gpu, &vast_cpu, "VAST Build (GPU)", name);

    let vast_reference_eval_outputs = run_reference_eval(&prog_vast, &vast_inputs);
    // reference_eval returns ONLY ReadWrite buffers. So [0] is binding 3, [1] is binding 4.
    let vast_reference_eval = vast_reference_eval_outputs[0].clone();
    assert_parity(&vast_reference_eval, &vast_cpu, "VAST Build (Reference Eval)", name);

    // 2. Annotate
    let annotated_cpu = reference_c11_annotate_typedef_names(&vast_cpu, source.as_bytes());
    let prog_annot = c11_annotate_typedef_names(
        "vast",
        "haystack",
        Expr::u32(source.len() as u32),
        Expr::u32(node_count_from_vast(&vast_cpu)),
        "out",
    );

    let annot_inputs = vec![
        vast_cpu.clone(),
        haystack_words(source.as_bytes()),
        vec![0; vast_cpu.len()],
    ];

    let annotated_gpu = run_gpu_fast_typedef_annotation(source.as_bytes(), &vast_gpu);
    assert_parity(&annotated_gpu, &annotated_cpu, "VAST Annotate (GPU)", name);

    let annotated_reference_eval_outputs = run_reference_eval(&prog_annot, &annot_inputs);
    // [0] is binding 2.
    let annotated_reference_eval = annotated_reference_eval_outputs[0].clone();
    assert_parity(
        &annotated_reference_eval,
        &annotated_cpu,
        "VAST Annotate (Reference Eval)",
        name,
    );

    // 3. Classify
    let classified_cpu = reference_c11_classify_vast_node_kinds(&annotated_cpu);
    let prog_classify = c11_classify_vast_node_kinds(
        "vast",
        Expr::u32(node_count_from_vast(&annotated_cpu)),
        "out",
    );

    let classify_inputs = vec![annotated_cpu.clone(), vec![0; annotated_cpu.len()]];

    let classified_gpu_outputs = dispatch_gpu_program(
        "real corpus VAST classify",
        prog_classify.clone(),
        classify_inputs.clone(),
    );
    // Binding 1 is ReadWrite. [0] is binding 1.
    let classified_gpu = classified_gpu_outputs[0].clone();
    assert_parity(
        &classified_gpu,
        &classified_cpu,
        "VAST Classify (GPU)",
        name,
    );

    let classified_reference_eval_outputs = run_reference_eval(&prog_classify, &classify_inputs);
    let classified_reference_eval = classified_reference_eval_outputs[0].clone();
    assert_parity(
        &classified_reference_eval,
        &classified_cpu,
        "VAST Classify (Reference Eval)",
        name,
    );

    // 4. Lower to PG
    let pg_cpu_ref = reference_ast_to_pg_nodes(&classified_cpu);
    let prog_pg = c_lower_ast_to_pg_nodes(
        "vast",
        Expr::u32(node_count_from_vast(&classified_cpu)),
        "out",
    );

    let out_pg_len = (node_count_from_vast(&classified_cpu) as usize) * 6 * 4;
    let pg_inputs = vec![classified_cpu.clone(), vec![0; out_pg_len]];

    let pg_gpu_outputs = dispatch_gpu_program("real corpus PG lower", prog_pg.clone(), pg_inputs.clone());
    // Binding 1 is ReadWrite. [0] is binding 1.
    let pg_gpu = pg_gpu_outputs[0].clone();
    assert_parity(&pg_gpu, &pg_cpu_ref, "PG Lower (GPU)", name);

    let pg_reference_eval_outputs = run_reference_eval(&prog_pg, &pg_inputs);
    let pg_reference_eval = pg_reference_eval_outputs[0].clone();
    assert_parity(&pg_reference_eval, &pg_cpu_ref, "PG Lower (Reference Eval)", name);
}

#[test]
fn test_kernel_atomic_add_unless_parity() {
    let source = "static inline int atomic_add_unless(atomic_t *v, int a, int u) {\n  int c = atomic_read(v);\n  do {\n    if (unlikely(c == u))\n      break;\n  } while (!atomic_try_cmpxchg(v, &c, c + a));\n  return c != u;\n}";
    let tokens = [
        ("static", TOK_STATIC),
        (" ", TOK_WHITESPACE),
        ("inline", TOK_INLINE),
        (" ", TOK_WHITESPACE),
        ("int", TOK_INT),
        (" ", TOK_WHITESPACE),
        ("atomic_add_unless", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("atomic_t", TOK_IDENTIFIER),
        (" ", TOK_WHITESPACE),
        ("*", TOK_STAR),
        ("v", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        (" ", TOK_WHITESPACE),
        ("int", TOK_INT),
        (" ", TOK_WHITESPACE),
        ("a", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        (" ", TOK_WHITESPACE),
        ("int", TOK_INT),
        (" ", TOK_WHITESPACE),
        ("u", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (" ", TOK_WHITESPACE),
        ("{", TOK_LBRACE),
        ("\n  ", TOK_WHITESPACE),
        ("int", TOK_INT),
        (" ", TOK_WHITESPACE),
        ("c", TOK_IDENTIFIER),
        (" ", TOK_WHITESPACE),
        ("=", TOK_ASSIGN),
        (" ", TOK_WHITESPACE),
        ("atomic_read", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("v", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("\n  ", TOK_WHITESPACE),
        ("do", TOK_DO),
        (" ", TOK_WHITESPACE),
        ("{", TOK_LBRACE),
        ("\n    ", TOK_WHITESPACE),
        ("if", TOK_IF),
        (" ", TOK_WHITESPACE),
        ("(", TOK_LPAREN),
        ("unlikely", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("c", TOK_IDENTIFIER),
        (" ", TOK_WHITESPACE),
        ("==", TOK_EQ),
        (" ", TOK_WHITESPACE),
        ("u", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        ("\n      ", TOK_WHITESPACE),
        ("break", TOK_BREAK),
        (";", TOK_SEMICOLON),
        ("\n  ", TOK_WHITESPACE),
        ("}", TOK_RBRACE),
        (" ", TOK_WHITESPACE),
        ("while", TOK_WHILE),
        (" ", TOK_WHITESPACE),
        ("(", TOK_LPAREN),
        ("!", TOK_BANG),
        ("atomic_try_cmpxchg", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("v", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        (" ", TOK_WHITESPACE),
        ("&", TOK_AMP),
        ("c", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        (" ", TOK_WHITESPACE),
        ("c", TOK_IDENTIFIER),
        (" ", TOK_WHITESPACE),
        ("+", TOK_PLUS),
        (" ", TOK_WHITESPACE),
        ("a", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("\n  ", TOK_WHITESPACE),
        ("return", TOK_RETURN),
        (" ", TOK_WHITESPACE),
        ("c", TOK_IDENTIFIER),
        (" ", TOK_WHITESPACE),
        ("!=", TOK_NE),
        (" ", TOK_WHITESPACE),
        ("u", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("\n", TOK_WHITESPACE),
        ("}", TOK_RBRACE),
    ];
    run_harness("kernel_atomic_add_unless", source, &tokens);
}

#[test]
fn test_libc_qsort_parity() {
    let source = "typedef int (*__compar_fn_t)(const void *, const void *);\nextern void qsort(void *__base, size_t __nmemb, size_t __size, __compar_fn_t __compar);";
    let tokens = [
        ("typedef", TOK_TYPEDEF),
        (" ", TOK_WHITESPACE),
        ("int", TOK_INT),
        (" ", TOK_WHITESPACE),
        ("(", TOK_LPAREN),
        ("*", TOK_STAR),
        ("__compar_fn_t", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        ("(", TOK_LPAREN),
        ("const", TOK_CONST),
        (" ", TOK_WHITESPACE),
        ("void", TOK_VOID),
        (" ", TOK_WHITESPACE),
        ("*", TOK_STAR),
        (",", TOK_COMMA),
        (" ", TOK_WHITESPACE),
        ("const", TOK_CONST),
        (" ", TOK_WHITESPACE),
        ("void", TOK_VOID),
        (" ", TOK_WHITESPACE),
        ("*", TOK_STAR),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("\n", TOK_WHITESPACE),
        ("extern", TOK_EXTERN),
        (" ", TOK_WHITESPACE),
        ("void", TOK_VOID),
        (" ", TOK_WHITESPACE),
        ("qsort", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("void", TOK_VOID),
        (" ", TOK_WHITESPACE),
        ("*", TOK_STAR),
        ("__base", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        (" ", TOK_WHITESPACE),
        ("size_t", TOK_IDENTIFIER),
        (" ", TOK_WHITESPACE),
        ("__nmemb", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        (" ", TOK_WHITESPACE),
        ("size_t", TOK_IDENTIFIER),
        (" ", TOK_WHITESPACE),
        ("__size", TOK_IDENTIFIER),
        (",", TOK_COMMA),
        (" ", TOK_WHITESPACE),
        ("__compar_fn_t", TOK_IDENTIFIER),
        (" ", TOK_WHITESPACE),
        ("__compar", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
    ];
    run_harness("libc_qsort", source, &tokens);
}

#[test]
fn test_sqlite_malloc_parity() {
    let source = "void *sqlite3Malloc(int n) {\n  void *p;\n  if (n <= 0) {\n    p = 0;\n  } else {\n    p = malloc(n);\n  }\n  return p;\n}";
    let tokens = [
        ("void", TOK_VOID),
        (" ", TOK_WHITESPACE),
        ("*", TOK_STAR),
        ("sqlite3Malloc", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("int", TOK_INT),
        (" ", TOK_WHITESPACE),
        ("n", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (" ", TOK_WHITESPACE),
        ("{", TOK_LBRACE),
        ("\n  ", TOK_WHITESPACE),
        ("void", TOK_VOID),
        (" ", TOK_WHITESPACE),
        ("*", TOK_STAR),
        ("p", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("\n  ", TOK_WHITESPACE),
        ("if", TOK_IF),
        (" ", TOK_WHITESPACE),
        ("(", TOK_LPAREN),
        ("n", TOK_IDENTIFIER),
        (" ", TOK_WHITESPACE),
        ("<=", TOK_LE),
        (" ", TOK_WHITESPACE),
        ("0", TOK_INTEGER),
        (")", TOK_RPAREN),
        (" ", TOK_WHITESPACE),
        ("{", TOK_LBRACE),
        ("\n    ", TOK_WHITESPACE),
        ("p", TOK_IDENTIFIER),
        (" ", TOK_WHITESPACE),
        ("=", TOK_ASSIGN),
        (" ", TOK_WHITESPACE),
        ("0", TOK_INTEGER),
        (";", TOK_SEMICOLON),
        ("\n  ", TOK_WHITESPACE),
        ("}", TOK_RBRACE),
        (" ", TOK_WHITESPACE),
        ("else", TOK_ELSE),
        (" ", TOK_WHITESPACE),
        ("{", TOK_LBRACE),
        ("\n    ", TOK_WHITESPACE),
        ("p", TOK_IDENTIFIER),
        (" ", TOK_WHITESPACE),
        ("=", TOK_ASSIGN),
        (" ", TOK_WHITESPACE),
        ("malloc", TOK_IDENTIFIER),
        ("(", TOK_LPAREN),
        ("n", TOK_IDENTIFIER),
        (")", TOK_RPAREN),
        (";", TOK_SEMICOLON),
        ("\n  ", TOK_WHITESPACE),
        ("}", TOK_RBRACE),
        ("\n  ", TOK_WHITESPACE),
        ("return", TOK_RETURN),
        (" ", TOK_WHITESPACE),
        ("p", TOK_IDENTIFIER),
        (";", TOK_SEMICOLON),
        ("\n", TOK_WHITESPACE),
        ("}", TOK_RBRACE),
    ];
    run_harness("sqlite_malloc", source, &tokens);
}
