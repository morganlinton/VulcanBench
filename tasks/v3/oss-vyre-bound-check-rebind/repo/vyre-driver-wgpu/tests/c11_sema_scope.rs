//! Reference, property, and GPU parity tests for C11 scope semantics.

#![cfg(feature = "c-parser")]
#![allow(deprecated)]

use proptest::prelude::*;
use std::sync::OnceLock;
use vyre::ir::{Expr, Program};
use vyre::{validate, DispatchConfig, VyreBackend};
use vyre_driver_wgpu::WgpuBackend;
use vyre_emit_naga::program as naga_emit;
use vyre_libs::parsing::c::lex::tokens::*;
use vyre_libs::parsing::c::sema::{c_sema_scope, reference_scope_tree};

const TEST_WORKGROUP_SIZE: [u32; 3] = [1, 1, 1];

#[derive(Clone)]
enum Atom {
    Tok(u32),
    Ident(String),
}

#[derive(Clone)]
struct Fixture {
    name: String,
    tok_types: Vec<u32>,
    tok_starts: Vec<u32>,
    tok_lens: Vec<u32>,
    haystack: Vec<u8>,
}

fn emit_u32_bytes(words: &[u32]) -> Vec<u8> {
    vyre_primitives::wire::pack_u32_slice(words)
}

fn haystack_words(bytes: &[u8]) -> Vec<u32> {
    bytes.iter().copied().map(u32::from).collect()
}

fn reference_values(inputs: &[Vec<u8>]) -> Vec<vyre_reference::value::Value> {
    let owned_inputs;
    let inputs = if inputs.iter().any(Vec::is_empty) {
        owned_inputs = inputs
            .iter()
            .map(|input| {
                if input.is_empty() {
                    vec![0; 4]
                } else {
                    input.clone()
                }
            })
            .collect::<Vec<_>>();
        owned_inputs.as_slice()
    } else {
        inputs
    };
    let mut values = inputs
        .iter()
        .map(|input| input.as_slice().into())
        .collect::<Vec<_>>();
    if inputs.len() == 4 {
        let token_words = inputs[0].len() / 4;
        values.push(vec![0; token_words.saturating_mul(4).max(1) * 4].into());
    }
    values
}

fn tok(t: u32) -> Atom {
    Atom::Tok(t)
}

fn ident(name: &str) -> Atom {
    Atom::Ident(name.to_string())
}

fn pack_fixture(atoms: &[Atom]) -> Fixture {
    let mut tok_types = Vec::<u32>::new();
    let mut tok_starts = Vec::<u32>::new();
    let mut tok_lens = Vec::<u32>::new();
    let mut haystack = Vec::<u8>::new();
    let mut cursor = 0u32;

    for atom in atoms {
        match atom {
            Atom::Tok(token) => {
                tok_types.push(*token);
                tok_starts.push(0);
                tok_lens.push(0);
            }
            Atom::Ident(name) => {
                tok_types.push(TOK_IDENTIFIER);
                tok_starts.push(cursor);
                tok_lens.push(name.len() as u32);
                haystack.extend_from_slice(name.as_bytes());
                cursor += name.len() as u32;
            }
        }
    }

    Fixture {
        name: String::new(),
        tok_types,
        tok_starts,
        tok_lens,
        haystack,
    }
}

fn fixture(name: &str, atoms: Vec<Atom>) -> Fixture {
    let mut fix = pack_fixture(&atoms);
    fix.name = name.to_string();
    fix
}

fn program_for(num_tokens: u32, haystack_len: usize) -> Program {
    c_sema_scope(
        "tok_types",
        "tok_starts",
        "tok_lens",
        "haystack",
        Expr::u32(haystack_len as u32),
        Expr::u32(num_tokens),
        "out_scope_tree",
    )
}

fn reference_case(fix: &Fixture) -> Vec<u32> {
    let haystack = haystack_words(&fix.haystack);
    reference_scope_tree(&fix.tok_types, &fix.tok_starts, &fix.tok_lens, &haystack)
}

fn assert_exact_mapping(name: &str, expected: &[u32], actual: &[u8]) {
    let actual_words: Vec<u32> = actual
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("u32 chunk")))
        .collect();
    assert_eq!(
        expected.len(),
        actual_words.len(),
        "{name}: scope-tree width mismatch, expected {} words, got {}",
        expected.len(),
        actual_words.len()
    );

    for (node_idx, chunk) in actual_words.chunks_exact(4).enumerate() {
        let expected_chunk = &expected[node_idx * 4..node_idx * 4 + 4];
        assert_eq!(
            chunk, expected_chunk,
            "{name}: exact mapping mismatch at node {node_idx}: expected {expected_chunk:?}, got {chunk:?}"
        );
    }
}

fn backend() -> &'static WgpuBackend {
    static BACKEND: OnceLock<WgpuBackend> = OnceLock::new();
    BACKEND.get_or_init(|| WgpuBackend::acquire().expect("Fix: GPU backend must be available"))
}

fn case_inputs(fix: &Fixture) -> Vec<Vec<u8>> {
    vec![
        emit_u32_bytes(&fix.tok_types),
        emit_u32_bytes(&fix.tok_starts),
        emit_u32_bytes(&fix.tok_lens),
        emit_u32_bytes(&haystack_words(&fix.haystack)),
    ]
}

#[test]
fn c_sema_scope_program_emits_valid_wgsl() {
    let fixture = fixture(
        "wgsl",
        vec![
            tok(TOK_INT),
            ident("main"),
            tok(TOK_LPAREN),
            tok(TOK_RPAREN),
            tok(TOK_LBRACE),
            tok(TOK_RETURN),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ],
    );
    let program = program_for(fixture.tok_types.len() as u32, fixture.haystack.len());
    let errors = validate(&program);
    assert!(errors.is_empty(), "c_sema_scope must validate: {errors:?}");
    let module = naga_emit::emit_module(&program, &DispatchConfig::default(), TEST_WORKGROUP_SIZE)
        .expect("Scope op must lower to a valid Naga module");
    let _info = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .expect("Naga must validate scope op module");
    assert!(
        module
            .entry_points
            .iter()
            .any(|entry| entry.stage == naga::ShaderStage::Compute),
        "Scope op Naga module should define a compute entry"
    );
}

#[test]
fn c_sema_scope_witness_matches_cpu_reference() {
    let fixture = fixture(
        "witness",
        vec![
            tok(TOK_INT),
            ident("main"),
            tok(TOK_LPAREN),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            ident("label"),
            tok(TOK_COLON),
            tok(TOK_GOTO),
            ident("label"),
            tok(TOK_SEMICOLON),
            tok(TOK_LBRACE),
            tok(TOK_RBRACE),
        ],
    );
    let program = program_for(fixture.tok_types.len() as u32, fixture.haystack.len());
    let reference = reference_case(&fixture);
    let reference_bytes = emit_u32_bytes(&reference);
    let inputs = case_inputs(&fixture);
    let result = vyre_reference::reference_eval(&program, &reference_values(&inputs))
        .expect("Reference evaluator must run");
    assert_eq!(result.len(), 1, "Expected one output buffer");
    let actual = result[0].to_bytes().to_vec();
    assert_eq!(actual, reference_bytes);
    assert_exact_mapping("witness", &reference, &actual);
}

fn adversarial_fixtures() -> Vec<Fixture> {
    let mut cases = Vec::new();
    for depth in 1..=12 {
        let mut atoms = Vec::new();
        for idx in 0..depth {
            atoms.push(tok(TOK_LBRACE));
            atoms.push(tok(TOK_INT));
            atoms.push(ident(&format!("outer_{depth}_{idx}")));
            atoms.push(tok(TOK_SEMICOLON));
        }
        for _ in 0..depth {
            atoms.push(tok(TOK_RBRACE));
        }
        cases.push(fixture(&format!("nested_blocks_depth_{depth}"), atoms));
    }

    for depth in 1..=10 {
        let mut atoms = vec![tok(TOK_LBRACE)];
        for idx in 0..depth {
            atoms.push(tok(TOK_INT));
            atoms.push(ident(&format!("x_{idx}")));
            atoms.push(tok(TOK_SEMICOLON));
            atoms.push(tok(TOK_LBRACE));
        }
        for _ in 0..=depth {
            atoms.push(tok(TOK_RBRACE));
        }
        cases.push(fixture(&format!("shadowing_levels_{depth}"), atoms));
    }

    for idx in 0..8 {
        let label = format!("lbl_{idx}");
        let atoms = vec![
            ident(&label),
            tok(TOK_COLON),
            tok(TOK_INT),
            ident("x"),
            tok(TOK_SEMICOLON),
            tok(TOK_GOTO),
            ident(&label),
            tok(TOK_SEMICOLON),
        ];
        cases.push(fixture(&format!("label_goto_{idx}"), atoms));
    }

    for idx in 0..8 {
        let fname = format!("kr_{idx}");
        let atoms = vec![
            tok(TOK_INT),
            ident(&fname),
            tok(TOK_LPAREN),
            ident("a"),
            tok(TOK_COMMA),
            ident("b"),
            tok(TOK_RPAREN),
            tok(TOK_INT),
            ident("a"),
            tok(TOK_SEMICOLON),
            tok(TOK_INT),
            ident("b"),
            tok(TOK_SEMICOLON),
            tok(TOK_LBRACE),
            tok(TOK_RETURN),
            ident("a"),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
        ];
        cases.push(fixture(&format!("kr_style_{idx}"), atoms));
    }

    for idx in 0..8 {
        let atoms = vec![
            ident(&format!("__extension__{idx}")),
            tok(TOK_LPAREN),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident(&format!("ext_{idx}")),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_RPAREN),
        ];
        cases.push(fixture(&format!("gnu_extension_{idx}"), atoms));
    }

    for idx in 0..8 {
        let atoms = vec![
            ident("_Generic"),
            tok(TOK_LPAREN),
            ident("x"),
            tok(TOK_COMMA),
            tok(TOK_INT),
            ident(&format!("generic_{idx}")),
            tok(TOK_RPAREN),
            tok(TOK_SEMICOLON),
            ident(&format!("x{idx}")),
            tok(TOK_PLUS),
            ident(&format!("y{idx}")),
            tok(TOK_SEMICOLON),
        ];
        cases.push(fixture(&format!("generic_{idx}"), atoms));
    }

    for idx in 0..8 {
        let atoms = vec![
            tok(TOK_LPAREN),
            tok(TOK_LBRACE),
            tok(TOK_INT),
            ident(&format!("sx_{idx}")),
            tok(TOK_SEMICOLON),
            tok(TOK_RBRACE),
            tok(TOK_RPAREN),
        ];
        cases.push(fixture(&format!("statement_expr_{idx}"), atoms));
    }

    cases
}

#[test]
fn c_sema_scope_adversarial_fixtures_have_exact_node_scope_mapping() {
    let backend = backend();
    let lowered = |fix: &Fixture| {
        let n = fix.tok_types.len() as u32;
        c_sema_scope(
            "tok_types",
            "tok_starts",
            "tok_lens",
            "haystack",
            Expr::u32(fix.haystack.len() as u32),
            Expr::u32(n),
            "out_scope_tree",
        )
    };

    for case in adversarial_fixtures() {
        let expected = reference_case(&case);
        let expected_bytes = emit_u32_bytes(&expected);
        let program = lowered(&case);
        let inputs = case_inputs(&case);
        let cpu_result = vyre_reference::reference_eval(&program, &reference_values(&inputs))
            .expect("CPU reference must run");
        assert_eq!(
            cpu_result.len(),
            1,
            "CPU output should expose one RW buffer"
        );
        let cpu_output = cpu_result[0].to_bytes().to_vec();
        assert_exact_mapping(&case.name, &expected, &cpu_output);
        assert_eq!(
            cpu_output, expected_bytes,
            "{} CPU output differs from reference bytes",
            case.name
        );

        let optimized = vyre_foundation::optimizer::pre_lowering::optimize(program.clone());
        let gpu_output = backend
            .dispatch(&optimized, &inputs, &DispatchConfig::default())
            .expect("GPU backend must dispatch");
        assert_eq!(gpu_output.len(), 1);
        assert_eq!(gpu_output[0].len(), expected_bytes.len());
        assert_exact_mapping(&case.name, &expected, &gpu_output[0]);
        assert_eq!(
            gpu_output[0], expected_bytes,
            "{} GPU output differs from reference bytes",
            case.name
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn c_sema_scope_random_fixture_parity(
        tokens in proptest::collection::vec(0u8..10, 1..64),
    ) {
        let mut atoms = Vec::new();
        let names = ["alpha", "beta", "gamma", "delta", "epsilon", "z"];
        for code in tokens {
            if code < 4 {
                atoms.push(Atom::Ident(names[code as usize % names.len()].to_string()));
            } else if code == 4 {
                atoms.push(tok(TOK_LBRACE));
            } else if code == 5 {
                atoms.push(tok(TOK_RBRACE));
            } else if code == 6 {
                atoms.push(tok(TOK_LPAREN));
            } else if code == 7 {
                atoms.push(tok(TOK_RPAREN));
            } else if code == 8 {
                atoms.push(tok(TOK_INT));
            } else {
                atoms.push(tok(TOK_SEMICOLON));
            }
        }
        let fixture = fixture("random", atoms);
        let expected = reference_case(&fixture);
        let expected_bytes = emit_u32_bytes(&expected);
        let program = program_for(fixture.tok_types.len() as u32, fixture.haystack.len());
        let outputs = vyre_reference::reference_eval(
            &program,
            &reference_values(&case_inputs(&fixture)),
        ).expect("Reference evaluator must run for random fixture");
        assert_eq!(outputs.len(), 1, "Random fixture must expose one output buffer");
        let cpu_bytes = outputs[0].to_bytes().to_vec();
        assert_eq!(
            cpu_bytes,
            expected_bytes,
            "CPU reference must match deterministic CPU helper for random fixture"
        );
    }
}

#[test]
fn c_sema_scope_boundary_sizes_do_not_panic() {
    let fixture = fixture(
        "boundary",
        vec![tok(TOK_INT), ident("x"), tok(TOK_SEMICOLON)],
    );
    for n in [0u32, 1, 2, 8, 256, 257] {
        let mut short_tokens = fixture.tok_types.clone();
        let mut short_starts = fixture.tok_starts.clone();
        let mut short_lens = fixture.tok_lens.clone();
        let short_haystack = fixture.haystack.clone();
        if short_tokens.len() > n as usize {
            short_tokens.truncate(n as usize);
            short_starts.truncate(n as usize);
            short_lens.truncate(n as usize);
        } else {
            short_tokens.resize(n as usize, 0);
            short_starts.resize(n as usize, 0);
            short_lens.resize(n as usize, 0);
        }

        let test = Fixture {
            name: "boundary".to_string(),
            tok_types: short_tokens,
            tok_starts: short_starts,
            tok_lens: short_lens,
            haystack: short_haystack.clone(),
        };

        let program = c_sema_scope(
            "tok_types",
            "tok_starts",
            "tok_lens",
            "haystack",
            Expr::u32(test.haystack.len() as u32),
            Expr::u32(n),
            "out_scope_tree",
        );
        assert!(
            validate(&program).is_empty(),
            "Boundary scope program should validate for n={n}"
        );
        let outputs =
            vyre_reference::reference_eval(&program, &reference_values(&case_inputs(&test)))
                .expect("Boundary fixture must execute in CPU reference");
        assert_eq!(outputs.len(), 1);
        let _ = outputs[0].to_bytes();
    }
}
