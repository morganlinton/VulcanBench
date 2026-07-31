//! DFA lexer table compilation.
//!
//! Converts a list of token regexes into a deterministic finite
//! automaton with one transition row per state and per character class.
//! Emission format matches what `vyre-libs::parsing::lexer` expects:
//!
//! ```text
//! dfa_transitions[state * NUM_CLASSES + class] =
//!   (next_state << 16) | action
//!
//! dfa_token_ids[state] = token_kind_emitted_on_enter   // 0 if non-accepting
//! ```
//!
//! `action` values:
//!  - 0 = CONTINUE (advance to next_state, consume the byte)
//!  - 1 = EMIT_TOKEN (emit token of kind `dfa_token_ids[state]`, reset to state 0)
//!  - 2 = PUSH_BACK (emit previous accepting state's token, don't consume this byte)
//!  - 3 = ERROR

use regex_automata::MatchKind;
use serde::{Deserialize, Serialize};

/// DFA action on a transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum Action {
    /// Advance to next_state, consume byte.
    Continue = 0,
    /// Emit a token of `dfa_token_ids[state]`, reset to initial state.
    EmitToken = 1,
    /// Emit previous accepting token, keep the current byte for re-lex.
    PushBack = 2,
    /// Hard error  -  unrecognized input.
    Error = 3,
}

/// Packed 32-bit transition: `(next_state << 16) | action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    /// State to advance to on this (state, class) pair.
    pub next_state: u16,
    /// What to do with the byte.
    pub action: Action,
}

impl Transition {
    /// Pack into a single u32 ready for the GPU.
    #[must_use]
    pub fn pack(self) -> u32 {
        (u32::from(self.next_state) << 16) | (self.action as u32)
    }

    /// Unpack a u32 back to the structured form.
    #[must_use]
    pub fn unpack(word: u32) -> Self {
        let next_state = (word >> 16) as u16;
        let action = match word & 0xFFFF {
            0 => Action::Continue,
            1 => Action::EmitToken,
            2 => Action::PushBack,
            _ => Action::Error,
        };
        Self { next_state, action }
    }
}

/// The compiled DFA: dense row-major transition table + per-state
/// accepting-token id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DfaTable {
    /// Number of states.
    pub num_states: u32,
    /// Number of input character classes.
    pub num_classes: u32,
    /// Flat transitions, row-major: `[state][class] =
    /// transitions[state * num_classes + class]`.
    pub transitions: Vec<u32>,
    /// One u32 per state. `token_ids[state] = 0` means non-accepting.
    pub token_ids: Vec<u32>,
}

impl DfaTable {
    /// Get the transition for `(state, class)`.
    #[must_use]
    pub fn transition(&self, state: u32, class: u32) -> Transition {
        let idx = (state * self.num_classes + class) as usize;
        Transition::unpack(self.transitions[idx])
    }

    /// Set the transition for `(state, class)`.
    pub fn set_transition(&mut self, state: u32, class: u32, t: Transition) {
        let idx = (state * self.num_classes + class) as usize;
        self.transitions[idx] = t.pack();
    }
}

/// Builder-side API for populating a DFA table without the index math.
#[derive(Debug, Clone)]
pub struct DfaBuilder {
    table: DfaTable,
    patterns: Vec<(u32, String)>,
}

impl DfaBuilder {
    /// Allocate a zero-initialized DFA with `num_states × num_classes`
    /// transitions all set to `Action::Error` and every state
    /// non-accepting.
    #[must_use]
    pub fn new(num_states: u32, num_classes: u32) -> Self {
        let size = (num_states * num_classes) as usize;
        let error_word = Transition {
            next_state: 0,
            action: Action::Error,
        }
        .pack();
        Self {
            table: DfaTable {
                num_states,
                num_classes,
                transitions: vec![error_word; size],
                token_ids: vec![0; num_states as usize],
            },
            patterns: Vec::new(),
        }
    }

    /// Add a regex pattern to the builder for the given token_id.
    pub fn add_pattern(&mut self, token_id: u32, pattern: &str) {
        self.patterns.push((token_id, pattern.to_string()));
    }

    /// Record `(state, class) -> next_state` with `Action::Continue`.
    ///
    /// # Errors
    ///
    /// Returns an error when `next_state` exceeds [`u16::MAX`]. The packed
    /// 16-bit transition encoding cannot represent more than 65535 states;
    /// callers must keep DFA state counts within that range.
    pub fn continue_to(
        &mut self,
        state: u32,
        class: u32,
        next_state: u32,
    ) -> Result<(), String> {
        let next_state_u16 = u16::try_from(next_state).map_err(|_| {
            format!(
                "Fix: DFA next_state {next_state} exceeds u16::MAX ({}). \
                 The packed 16-bit transition encoding cannot represent this state index; \
                 the DFA has too many states.",
                u16::MAX
            )
        })?;
        self.table.set_transition(
            state,
            class,
            Transition {
                next_state: next_state_u16,
                action: Action::Continue,
            },
        );
        Ok(())
    }

    /// Mark `state` as accepting with token id `token_id`.
    pub fn accept(&mut self, state: u32, token_id: u32) {
        self.table.token_ids[state as usize] = token_id;
    }

    /// Finalize with max-munch lexer semantics.
    ///
    /// # Errors
    ///
    /// Returns an error if the regex patterns fail to compile into a DFA or if
    /// the DFA cannot produce a start state. A failure here means the compiled
    /// DFA would silently reject all input; the error is surfaced so callers
    /// can fail loudly rather than upload a zero-recall table.
    pub fn build(self) -> Result<DfaTable, String> {
        self.build_with_match_kind(MatchKind::LeftmostFirst)
    }

    /// Finalize with a given [`MatchKind`].
    ///
    /// Callers that need every overlapping regex match must opt into
    /// [`MatchKind::All`] explicitly; the C lexer path uses leftmost-first
    /// max-munch because each byte position must produce at most one token.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The regex patterns fail to compile into a dense DFA (e.g. NFA size
    ///   limit exceeded, invalid syntax).
    /// - The DFA cannot return a valid start state.
    /// - A DFA state index overflows the packed 16-bit transition encoding.
    ///
    /// Any failure means the compiled DFA would silently reject all input; the
    /// error is surfaced so callers can fail loudly rather than upload a
    /// zero-recall table to the GPU.
    pub fn build_with_match_kind(self, kind: MatchKind) -> Result<DfaTable, String> {
        if self.patterns.is_empty() {
            return Ok(self.table);
        }

        use regex_automata::dfa::{dense, Automaton};
        use regex_automata::Input;

        let anchored_regexes: Vec<String> = self
            .patterns
            .iter()
            .map(|(_, p)| format!("^(?:{p})"))
            .collect();
        let regexes: Vec<&str> = anchored_regexes.iter().map(String::as_str).collect();
        let dfa = dense::Builder::new()
            .configure(dense::Config::new().match_kind(kind))
            .build_many(&regexes)
            .map_err(|e| {
                format!(
                    "Fix: DFA build failed — regex pattern set did not compile: {e:?}. \
                     The GPU would receive a zero-recall table that rejects all input."
                )
            })?;

        let input = Input::new("");
        let start_id = dfa.start_state_forward(&input).map_err(|e| {
            format!(
                "Fix: DFA start state could not be determined: {e:?}. \
                 The GPU would receive a zero-recall table that rejects all input."
            )
        })?;

        let mut state_queue = vec![start_id];
        let mut id_to_idx = std::collections::HashMap::new();
        id_to_idx.insert(start_id, 0u32);

        let mut i = 0;
        while i < state_queue.len() {
            let id = state_queue[i];
            i += 1;
            for byte in 0..=255u8 {
                let next_id = dfa.next_state(id, byte);
                if let std::collections::hash_map::Entry::Vacant(e) = id_to_idx.entry(next_id) {
                    e.insert(state_queue.len() as u32);
                    state_queue.push(next_id);
                }
            }
        }

        let num_states = state_queue.len();
        let num_classes = 256;
        let size = num_states * num_classes;
        let error_word = Transition {
            next_state: 0,
            action: Action::Error,
        }
        .pack();

        let mut table = DfaTable {
            num_states: num_states as u32,
            num_classes: num_classes as u32,
            transitions: vec![error_word; size],
            token_ids: vec![0; num_states],
        };

        for (state_idx, &id) in state_queue.iter().enumerate() {
            let is_match = dfa.is_match_state(id);
            if is_match {
                let match_count = dfa.match_len(id);
                if match_count > 0 {
                    let pat_idx = dfa.match_pattern(id, 0).as_usize();
                    table.token_ids[state_idx] = self.patterns[pat_idx].0;
                }
            }

            for byte in 0..=255u8 {
                let next_id = dfa.next_state(id, byte);
                let next_idx = id_to_idx[&next_id];

                let action = if dfa.is_dead_state(next_id) || dfa.is_quit_state(next_id) {
                    Action::Error
                } else {
                    Action::Continue
                };

                let next_state_u16 = u16::try_from(next_idx).map_err(|_| {
                    format!(
                        "Fix: DFA state index {next_idx} overflows the packed 16-bit \
                         transition encoding (max {}). Reduce the number of patterns or \
                         use a coarser character class map.",
                        u16::MAX
                    )
                })?;

                table.set_transition(
                    state_idx as u32,
                    byte as u32,
                    Transition {
                        next_state: next_state_u16,
                        action,
                    },
                );
            }
        }

        Ok(table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_pack_unpack_preserves_fields() {
        for action in [
            Action::Continue,
            Action::EmitToken,
            Action::PushBack,
            Action::Error,
        ] {
            for &next in &[0u16, 1, 42, 1000, u16::MAX] {
                let t = Transition {
                    next_state: next,
                    action,
                };
                let got = Transition::unpack(t.pack());
                assert_eq!(got.next_state, next);
                assert_eq!(got.action, action);
            }
        }
    }

    #[test]
    fn builder_default_row_is_error() {
        let b = DfaBuilder::new(4, 8);
        let table = b.build().expect("empty pattern set must succeed");
        assert_eq!(table.num_states, 4);
        assert_eq!(table.num_classes, 8);
        for &t in &table.transitions {
            assert_eq!(Transition::unpack(t).action, Action::Error);
        }
    }

    #[test]
    fn builder_continue_populates_cell() {
        let mut b = DfaBuilder::new(4, 8);
        b.continue_to(1, 3, 2).expect("state 2 fits in u16");
        let table = b.build().expect("empty pattern set must succeed");
        let got = table.transition(1, 3);
        assert_eq!(got.next_state, 2);
        assert_eq!(got.action, Action::Continue);
    }

    #[test]
    fn builder_accept_sets_token_id() {
        let mut b = DfaBuilder::new(4, 8);
        b.accept(2, 42);
        let table = b.build().expect("empty pattern set must succeed");
        assert_eq!(table.token_ids[2], 42);
    }

    #[test]
    fn build_with_invalid_regex_returns_err_not_all_error_table() {
        // A deliberately broken regex must produce Err, not a silent all-Error
        // DFA that rejects every input with no diagnostic (dfa-build-silent-fallback).
        let mut b = DfaBuilder::new(0, 0);
        b.add_pattern(1, "[invalid(unclosed");
        let result = b.build();
        let err = result.expect_err(
            "Fix: DFA builder must return Err on regex compile failure, not a silent zero-recall table",
        );
        assert!(
            err.contains("Fix:"),
            "Fix: error message must include a 'Fix:' action hint, got: {err}"
        );
        assert!(
            err.contains("did not compile") || err.contains("DFA build failed"),
            "Fix: error must identify the compile failure, got: {err}"
        );
    }

    #[test]
    fn continue_to_overflow_returns_err_not_clamped_max() {
        // next_state that overflows u16 must return Err, not silently store u16::MAX
        // which points to a non-existent state (dfa-next-state-overflow-silent).
        let mut b = DfaBuilder::new(4, 8);
        let result = b.continue_to(0, 0, u32::from(u16::MAX) + 1);
        let err = result.expect_err(
            "Fix: continue_to must return Err when next_state overflows u16, \
             not silently clamp to u16::MAX",
        );
        assert!(
            err.contains("Fix:"),
            "Fix: error must include a 'Fix:' action hint, got: {err}"
        );
        assert!(
            err.contains("exceeds u16::MAX") || err.contains("overflow"),
            "Fix: error must name the overflow, got: {err}"
        );
        // The transition must not have been written — state (0,0) must still be Error.
        let table = b.build().expect("empty patterns must succeed");
        assert_eq!(
            table.transition(0, 0).action,
            Action::Error,
            "Fix: failed continue_to must not mutate the transition table"
        );
    }
}
