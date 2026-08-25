// Integration test module for the containing Vyre package.

use vyre_libs::parsing::c::lex::keyword::reference_c_keyword_types;

#[derive(Clone, Copy)]
pub(crate) struct FixtureToken {
    pub(crate) lexeme: &'static str,
    pub(crate) raw_kind: u32,
}

impl FixtureToken {
    pub(crate) const fn new(lexeme: &'static str, raw_kind: u32) -> Self {
        Self { lexeme, raw_kind }
    }
}

pub(crate) struct Fixture {
    pub(crate) source: String,
    pub(crate) raw_kinds: Vec<u32>,
    pub(crate) tok_types: Vec<u32>,
    pub(crate) tok_starts: Vec<u32>,
    pub(crate) tok_lens: Vec<u32>,
}

pub(crate) fn build_fixture(tokens: &[FixtureToken]) -> Fixture {
    let mut source = String::new();
    let mut raw_kinds = Vec::with_capacity(tokens.len());
    let mut tok_starts = Vec::with_capacity(tokens.len());
    let mut tok_lens = Vec::with_capacity(tokens.len());

    for token in tokens {
        if !source.is_empty() && !source.ends_with('\n') {
            source.push(' ');
        }
        tok_starts.push(source.len() as u32);
        source.push_str(token.lexeme);
        tok_lens.push(token.lexeme.len() as u32);
        raw_kinds.push(token.raw_kind);
    }

    let promoted = reference_c_keyword_types(&raw_kinds, &tok_starts, &tok_lens, source.as_bytes());

    Fixture {
        source,
        raw_kinds: promoted.clone(),
        tok_types: promoted,
        tok_starts,
        tok_lens,
    }
}

#[allow(unused_macros)]
macro_rules! c_fixture {
    ($(($lexeme:literal, $kind:expr $(,)?)),+ $(,)?) => {
        $crate::c_ast_gpu_parity_support::build_fixture(&[
            $(
                $crate::c_ast_gpu_parity_support::FixtureToken::new($lexeme, $kind),
            )+
        ])
    };
}

#[allow(unused_imports)]
pub(crate) use c_fixture;
