//! Error type + blake3 fixture helpers shared with [`crate::lex_c11_max_munch`].

use blake3::Hash;

/// Lexer failure on the host reference path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexCpuError {
    /// No pattern matched at this offset.
    NoTokenAt {
        /// Byte offset in the input.
        offset: usize,
    },
}

/// Blake3 digest of `kinds` encoded as little-endian `u32` words (stable corpus goldens).
#[must_use]
pub fn kinds_blake3(kinds: &[u32]) -> Hash {
    let mut bytes = Vec::with_capacity(kinds.len() * 4);
    for k in kinds {
        bytes.extend_from_slice(&k.to_le_bytes());
    }
    blake3::hash(&bytes)
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;

    use crate::c11_lexer::build_c11_lexer_dfa;
    use crate::chunk_lexer_cpu::count_chunked_valid_tokens;
    use crate::dfa::DfaTable;
    use crate::host_preprocess::preprocess_c_host;
    use crate::lex_c11_max_munch::lex_c11_max_munch_kinds;

    static C11_GPU_DFA: OnceLock<DfaTable> = OnceLock::new();

    #[test]
    fn max_munch_non_whitespace_count_on_hello() {
        let dfa = C11_GPU_DFA.get_or_init(build_c11_lexer_dfa);
        let src = preprocess_c_host(include_str!("../corpus/hello.c"));
        let kinds = lex_c11_max_munch_kinds(src.as_bytes()).expect("lex hello.c");
        let non_meta = kinds.iter().filter(|&&k| k != 200 && k != 201).count();
        assert!(non_meta > 0, "expected non-meta tokens");
        let chunk = count_chunked_valid_tokens(
            &dfa.transitions,
            &dfa.token_ids,
            src.as_bytes(),
            src.len() as u32,
            dfa.num_states,
            64,
            dfa.num_classes,
        );
        assert!(chunk > 0, "chunk reference should see tokens too");
    }
}
