//! CPU reference for the **chunk-parallel** lexer algorithm used by
//! `vyre-libs::parsing::c::lex::lexer::c11_lexer` (see that module’s
//! `chunk_size` / lane walk). This is **not** a standards-complete
//! max-munch lexer; it matches the GPU kernel’s per-lane “longest
//! accept within this chunk” behaviour for differential tests.

/// Count emitted **valid** tokens (non-zero token id, excluding
/// whitespace / comment meta-tokens 200 / 201) across all lanes,
/// matching the GPU `c11_lexer` compaction filter.
#[must_use]
pub fn count_chunked_valid_tokens(
    transitions: &[u32],
    token_ids: &[u32],
    haystack: &[u8],
    haystack_len: u32,
    state_count: u32,
    chunk_size: u32,
    num_classes: u32,
) -> u32 {
    let expected = (state_count as usize).saturating_mul(num_classes as usize);
    if transitions.len() != expected || token_ids.len() != state_count as usize {
        return 0;
    }

    let mut total = 0u32;
    for lane in 0u32..256 {
        let chunk_base = lane.saturating_mul(chunk_size);
        if chunk_base >= haystack_len {
            continue;
        }

        let mut state = 0u32;
        let mut last_accept_state = 0u32;

        for k in 0..chunk_size {
            let pos = chunk_base.saturating_add(k);
            if pos >= haystack_len {
                break;
            }
            let byte = u32::from(haystack[pos as usize]);
            let idx = (state * num_classes).saturating_add(byte) as usize;
            if idx >= transitions.len() {
                break;
            }
            let packed = transitions[idx];
            state = packed / 65536;
            let candidate = token_ids.get(state as usize).copied().unwrap_or(0);
            if candidate != 0 {
                last_accept_state = state;
            }
        }

        let emit_token = token_ids
            .get(last_accept_state as usize)
            .copied()
            .unwrap_or(0);
        let valid = emit_token != 0 && emit_token != 200 && emit_token != 201;
        if valid {
            total = total.saturating_add(1);
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dfa::DfaBuilder;

    #[test]
    fn chunk_count_matches_trivial_two_state_dfa() {
        let mut b = DfaBuilder::new(2, 256);
        for c in 0u32..256 {
            b.continue_to(0, c, 1).expect("state 1 fits in u16");
            b.continue_to(1, c, 1).expect("state 1 fits in u16");
        }
        b.accept(1, 42);
        let dfa = b.build().expect("empty pattern set must succeed");
        let n = count_chunked_valid_tokens(
            &dfa.transitions,
            &dfa.token_ids,
            b"a",
            1,
            dfa.num_states,
            64,
            dfa.num_classes,
        );
        assert!(n >= 1, "at least one lane should emit token 42, got {n}");
    }
}
