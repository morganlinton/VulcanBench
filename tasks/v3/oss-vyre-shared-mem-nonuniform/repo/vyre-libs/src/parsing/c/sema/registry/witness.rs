#![allow(deprecated)]

use super::reference::{
    brace_scope_id_for_node, brace_scope_parent_id_for_node, function_parameter_scope,
    reference_scope_tree,
};
use super::*;
use crate::parsing::c::lex::tokens::*;

use crate::scan::dispatch_io::pack_u32_slice as pack_u32;

const WITNESS_TOKEN_COUNT: u32 = 14;

#[derive(Clone, Copy)]
struct FixtureAtom {
    token: u32,
    start: u32,
    len: u32,
}

fn witness_fixture() -> (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>) {
    let atoms = [
        FixtureAtom {
            token: TOK_INT,
            start: 0,
            len: 0,
        },
        FixtureAtom {
            token: TOK_IDENTIFIER,
            start: 0,
            len: 4,
        },
        FixtureAtom {
            token: TOK_LPAREN,
            start: 0,
            len: 0,
        },
        FixtureAtom {
            token: TOK_RPAREN,
            start: 0,
            len: 0,
        },
        FixtureAtom {
            token: TOK_LBRACE,
            start: 0,
            len: 0,
        },
        FixtureAtom {
            token: TOK_INT,
            start: 4,
            len: 0,
        },
        FixtureAtom {
            token: TOK_IDENTIFIER,
            start: 4,
            len: 1,
        },
        FixtureAtom {
            token: TOK_SEMICOLON,
            start: 0,
            len: 0,
        },
        FixtureAtom {
            token: TOK_RBRACE,
            start: 0,
            len: 0,
        },
        FixtureAtom {
            token: TOK_IDENTIFIER,
            start: 5,
            len: 5,
        },
        FixtureAtom {
            token: TOK_COLON,
            start: 0,
            len: 0,
        },
        FixtureAtom {
            token: TOK_GOTO,
            start: 0,
            len: 0,
        },
        FixtureAtom {
            token: TOK_IDENTIFIER,
            start: 10,
            len: 5,
        },
        FixtureAtom {
            token: TOK_SEMICOLON,
            start: 0,
            len: 0,
        },
    ];

    let mut tokens = Vec::with_capacity(atoms.len());
    let mut starts = Vec::with_capacity(atoms.len());
    let mut lens = Vec::with_capacity(atoms.len());
    let mut max_end = 0usize;
    for atom in atoms {
        if let Some(end) = atom
            .start
            .checked_add(atom.len)
            .and_then(|value| usize::try_from(value).ok())
        {
            max_end = max_end.max(end);
        }
        tokens.push(atom.token);
        starts.push(atom.start);
        lens.push(atom.len);
    }
    let mut haystack = vec![0u32; max_end.max(16)];
    haystack[0..4].copy_from_slice(&[b'm', b'a', b'i', b'n'].map(u32::from));
    haystack[4] = u32::from(b'x');
    haystack[5..10].copy_from_slice(&[b'l', b'a', b'b', b'e', b'l'].map(u32::from));
    haystack[10..15].copy_from_slice(&[b'l', b'a', b'b', b'e', b'l'].map(u32::from));
    (tokens, starts, lens, haystack)
}

fn witness_inputs() -> Vec<Vec<Vec<u8>>> {
    let (tokens, starts, lens, haystack) = witness_fixture();
    vec![vec![
        pack_u32(&tokens),
        pack_u32(&starts),
        pack_u32(&lens),
        pack_u32(&haystack),
        vec![0; tokens.len() * 4 * 4],
    ]]
}

fn witness_expected() -> Vec<Vec<Vec<u8>>> {
    let (tokens, starts, lens, haystack) = witness_fixture();
    let outputs = reference_scope_tree(&tokens, &starts, &lens, &haystack);
    vec![vec![pack_u32(&outputs)]]
}

fn witness_expected_phase(phase: CScopePhase) -> Vec<Vec<Vec<u8>>> {
    let (tokens, starts, lens, haystack) = witness_fixture();
    let full = reference_scope_tree(&tokens, &starts, &lens, &haystack);
    let mut outputs = vec![0u32; full.len()];
    let column = match phase {
        CScopePhase::Scope => None,
        CScopePhase::ScopeBrace => {
            for row in 0..tokens.len() {
                let scope_id = brace_scope_id_for_node(&tokens, row);
                outputs[row * 4] = scope_id;
                outputs[row * 4 + 1] = brace_scope_parent_id_for_node(&tokens, row, scope_id);
            }
            return vec![vec![pack_u32(&outputs)]];
        }
        CScopePhase::ScopeFunctionParameters => {
            for row in 0..tokens.len() {
                if let Some((scope_id, parent_id)) = function_parameter_scope(&tokens, row) {
                    outputs[row * 4] = scope_id;
                    outputs[row * 4 + 1] = parent_id;
                }
            }
            return vec![vec![pack_u32(&outputs)]];
        }
        CScopePhase::Decl => Some(2usize),
        CScopePhase::IdentifierIntern => Some(3usize),
    };
    match column {
        Some(offset) => {
            for row in 0..tokens.len() {
                outputs[row * 4 + offset] = full[row * 4 + offset];
            }
        }
        None => {
            for row in 0..tokens.len() {
                outputs[row * 4] = full[row * 4];
                outputs[row * 4 + 1] = full[row * 4 + 1];
            }
        }
    }
    vec![vec![pack_u32(&outputs)]]
}

fn witness_expected_scope_phase() -> Vec<Vec<Vec<u8>>> {
    witness_expected_phase(CScopePhase::Scope)
}

fn witness_expected_scope_brace_phase() -> Vec<Vec<Vec<u8>>> {
    witness_expected_phase(CScopePhase::ScopeBrace)
}

fn witness_expected_scope_function_parameters_phase() -> Vec<Vec<Vec<u8>>> {
    witness_expected_phase(CScopePhase::ScopeFunctionParameters)
}

fn witness_expected_decl_phase() -> Vec<Vec<Vec<u8>>> {
    witness_expected_phase(CScopePhase::Decl)
}

fn witness_expected_identifier_intern_phase() -> Vec<Vec<Vec<u8>>> {
    witness_expected_phase(CScopePhase::IdentifierIntern)
}

inventory::submit! {
    crate::harness::OpEntry {
        id: "vyre-libs::parsing::c_sema_scope",
        build: || {
            c_sema_scope(
                "tok_types",
                "tok_starts",
                "tok_lens",
                "haystack",
                Expr::u32(16),
                Expr::u32(WITNESS_TOKEN_COUNT),
                "out_scope_tree",
            )
        },
        test_inputs: Some(witness_inputs),
        expected_output: Some(witness_expected),
        category: Some("parsing"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: SCOPE_PHASE_OP_ID,
        build: || {
            c_sema_scope_phase(
                CScopePhase::Scope,
                SCOPE_PHASE_OP_ID,
                "tok_types",
                "tok_starts",
                "tok_lens",
                "haystack",
                Expr::u32(16),
                Expr::u32(WITNESS_TOKEN_COUNT),
                "out_scope_tree",
            )
        },
        test_inputs: Some(witness_inputs),
        expected_output: Some(witness_expected_scope_phase),
        category: Some("parsing"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: SCOPE_BRACE_PHASE_OP_ID,
        build: || {
            c_sema_scope_phase(
                CScopePhase::ScopeBrace,
                SCOPE_BRACE_PHASE_OP_ID,
                "tok_types",
                "tok_starts",
                "tok_lens",
                "haystack",
                Expr::u32(16),
                Expr::u32(WITNESS_TOKEN_COUNT),
                "out_scope_tree",
            )
        },
        test_inputs: Some(witness_inputs),
        expected_output: Some(witness_expected_scope_brace_phase),
        category: Some("parsing"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: SCOPE_FUNCTION_PARAMS_PHASE_OP_ID,
        build: || {
            c_sema_scope_phase(
                CScopePhase::ScopeFunctionParameters,
                SCOPE_FUNCTION_PARAMS_PHASE_OP_ID,
                "tok_types",
                "tok_starts",
                "tok_lens",
                "haystack",
                Expr::u32(16),
                Expr::u32(WITNESS_TOKEN_COUNT),
                "out_scope_tree",
            )
        },
        test_inputs: Some(witness_inputs),
        expected_output: Some(witness_expected_scope_function_parameters_phase),
        category: Some("parsing"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: DECL_PHASE_OP_ID,
        build: || {
            c_sema_scope_phase(
                CScopePhase::Decl,
                DECL_PHASE_OP_ID,
                "tok_types",
                "tok_starts",
                "tok_lens",
                "haystack",
                Expr::u32(16),
                Expr::u32(WITNESS_TOKEN_COUNT),
                "out_scope_tree",
            )
        },
        test_inputs: Some(witness_inputs),
        expected_output: Some(witness_expected_decl_phase),
        category: Some("parsing"),
    }
}

inventory::submit! {
    crate::harness::OpEntry {
        id: IDENTIFIER_INTERN_PHASE_OP_ID,
        build: || {
            c_sema_scope_phase(
                CScopePhase::IdentifierIntern,
                IDENTIFIER_INTERN_PHASE_OP_ID,
                "tok_types",
                "tok_starts",
                "tok_lens",
                "haystack",
                Expr::u32(16),
                Expr::u32(WITNESS_TOKEN_COUNT),
                "out_scope_tree",
            )
        },
        test_inputs: Some(witness_inputs),
        expected_output: Some(witness_expected_identifier_intern_phase),
        category: Some("parsing"),
    }
}
