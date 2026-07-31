//! DFA / Aho-Corasick sub-dialect: pre-built transition tables + scanner.
mod aho_corasick;
mod cooperative_dfa;
mod dfa_compile;

pub use aho_corasick::{aho_corasick, aho_corasick_bounded};
pub use cooperative_dfa::{cooperative_dfa_scan, cooperative_dfa_scan_body_with_store};
pub use dfa_compile::{
    dfa_compile, dfa_compile_with_budget, CompiledDfa, DfaCompileError, DEFAULT_DFA_BUDGET_BYTES,
};
