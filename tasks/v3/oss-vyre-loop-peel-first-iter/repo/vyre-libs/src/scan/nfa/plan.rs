//! NFA compiled-plan data model and literal-pattern state budgeting.

use super::alloc::reserve_vec;

/// Compiled plan for a pattern set.
#[derive(Debug, Clone)]
pub struct NfaPlan {
    /// Total NFA state count (across every pattern + the shared entry).
    pub num_states: u32,
    /// Input buffer length the plan was compiled against.
    pub input_len: u32,
    /// One `(pattern_id, pattern_len)` per accept state.
    pub accept_states: Vec<(u32, u32)>,
    /// NFA state id for each entry in [`accept_states`](Self::accept_states).
    pub accept_state_ids: Vec<u32>,
    /// Per-accept flag requiring the match start offset to be zero.
    pub accept_start_anchored: Vec<bool>,
    /// Per-accept flag requiring the match end offset to equal input length.
    pub accept_end_anchored: Vec<bool>,
}

impl NfaPlan {
    /// Attach the expected input length.
    #[must_use]
    pub fn for_input_len(mut self, input_len: u32) -> Self {
        self.input_len = input_len;
        self
    }
}

/// Errors returned by fallible NFA compilation and table construction.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum NfaCompileError {
    /// Pattern count does not fit the GPU ABI's `u32` pattern id field.
    PatternCountOverflow {
        /// Number of patterns supplied by the caller.
        count: usize,
    },
    /// One literal length does not fit the GPU ABI's `u32` length field.
    PatternLengthOverflow {
        /// Index of the oversized pattern.
        pattern_index: usize,
        /// UTF-8 byte length of the oversized pattern.
        len: usize,
    },
    /// Total NFA state count overflowed the GPU ABI's `u32` state id field.
    StateCountOverflow,
    /// Transition or epsilon table word count overflowed host `usize`.
    TableWordCountOverflow {
        /// Table being built.
        table: &'static str,
    },
    /// Compiler staging allocation failed.
    StorageReserveFailed {
        /// Scratch vector being reserved.
        field: &'static str,
        /// Requested target capacity.
        requested: usize,
        /// Allocator failure details.
        message: String,
    },
}

impl std::fmt::Display for NfaCompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PatternCountOverflow { count } => write!(
                f,
                "NFA pattern count {count} exceeds u32 capacity. Fix: shard the pattern set before NFA compilation."
            ),
            Self::PatternLengthOverflow { pattern_index, len } => write!(
                f,
                "NFA pattern {pattern_index} length {len} exceeds u32 capacity. Fix: split or reject oversized literals before NFA compilation."
            ),
            Self::StateCountOverflow => write!(
                f,
                "NFA state count overflows u32. Fix: use plan_shards to split the pattern set before compilation."
            ),
            Self::TableWordCountOverflow { table } => write!(
                f,
                "NFA {table} table word count overflows host usize. Fix: shard the pattern set before table construction."
            ),
            Self::StorageReserveFailed {
                field,
                requested,
                message,
            } => write!(
                f,
                "NFA compilation could not reserve {requested} {field} slot(s): {message}. Fix: shard the pattern set before compilation."
            ),
        }
    }
}

impl std::error::Error for NfaCompileError {}

/// Compile patterns into an [`NfaPlan`]. Literal-only: each pattern
/// contributes `len(p)` states; all patterns share state 0 (entry),
/// so total state count is `1 + sum(len(p))`.
#[must_use]
pub fn compile(patterns: &[&str]) -> NfaPlan {
    match try_compile(patterns) {
        Ok(plan) => plan,
        Err(error) => {
            // Returning a 1-state entry-only plan would build a scanner with no
            // accept states — it silently matches NOTHING, reporting every
            // input as clean. That is a total recall-loss silent fallback
            // (Law 10). Fail closed. Callers that must recover use try_compile.
            panic!(
                "vyre-libs NFA compile failed: {error} — \
                 returning an empty entry-only plan would build a scanner that silently matches nothing; \
                 use try_compile and reduce the pattern set below the state cap."
            )
        }
    }
}

/// Fallible counterpart of [`compile`].
///
/// # Errors
///
/// Returns [`NfaCompileError`] when pattern ids, pattern lengths, aggregate
/// state counts, or compiler scratch allocation cannot be represented safely.
pub fn try_compile(patterns: &[&str]) -> Result<NfaPlan, NfaCompileError> {
    let _pattern_count =
        u32::try_from(patterns.len()).map_err(|_| NfaCompileError::PatternCountOverflow {
            count: patterns.len(),
        })?;
    let mut accept_states = Vec::new();
    reserve_vec(&mut accept_states, patterns.len(), "accept state")?;
    let mut accept_state_ids = Vec::new();
    reserve_vec(&mut accept_state_ids, patterns.len(), "accept state id")?;
    let mut accept_start_anchored = Vec::new();
    reserve_vec(
        &mut accept_start_anchored,
        patterns.len(),
        "accept start-anchor flag",
    )?;
    accept_start_anchored.resize(patterns.len(), false);
    let mut accept_end_anchored = Vec::new();
    reserve_vec(
        &mut accept_end_anchored,
        patterns.len(),
        "accept end-anchor flag",
    )?;
    accept_end_anchored.resize(patterns.len(), false);
    let mut next_state: u32 = 1;
    for (pid, p) in patterns.iter().enumerate() {
        let pid = u32::try_from(pid).map_err(|_| NfaCompileError::PatternCountOverflow {
            count: patterns.len(),
        })?;
        let len = u32::try_from(p.len()).map_err(|_| NfaCompileError::PatternLengthOverflow {
            pattern_index: pid as usize,
            len: p.len(),
        })?;
        let accept_state_id = if len == 0 {
            0
        } else {
            next_state
                .checked_add(len)
                .and_then(|value| value.checked_sub(1))
                .ok_or(NfaCompileError::StateCountOverflow)?
        };
        accept_states.push((pid, len));
        accept_state_ids.push(accept_state_id);
        next_state = next_state
            .checked_add(len)
            .ok_or(NfaCompileError::StateCountOverflow)?;
    }
    Ok(NfaPlan {
        num_states: next_state,
        input_len: 0,
        accept_states,
        accept_state_ids,
        accept_start_anchored,
        accept_end_anchored,
    })
}

/// The degenerate entry-only plan. Used only by tests to pin the empty-pattern
/// contract; production `compile` no longer falls back to this on error (it
/// fails loud — see the panic arm above), so this is `#[cfg(test)]` to keep it
/// from being mistaken for a usable silent-fallback target.
#[cfg(test)]
fn empty_plan() -> NfaPlan {
    NfaPlan {
        num_states: 1,
        input_len: 0,
        accept_states: Vec::new(),
        accept_state_ids: Vec::new(),
        accept_start_anchored: Vec::new(),
        accept_end_anchored: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{compile, empty_plan, try_compile};

    #[test]
    fn compile_empty_patterns_returns_real_entry_state() {
        let plan = compile(&[]);

        assert_eq!(plan.num_states, 1);
        assert!(plan.accept_states.is_empty());
        assert!(plan.accept_state_ids.is_empty());
    }

    #[test]
    fn empty_plan_matches_try_compile_empty_contract() {
        let fallible = try_compile(&[]).expect("Fix: empty NFA compile must fit ABI");
        let fallback = empty_plan();

        assert_eq!(fallback.num_states, fallible.num_states);
        assert_eq!(fallback.accept_states, fallible.accept_states);
        assert_eq!(fallback.accept_state_ids, fallible.accept_state_ids);
    }

    #[test]
    fn compile_wrapper_fails_loud_not_silent_fallback() {
        let production = include_str!("plan.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: plan.rs must contain production section");

        // No LAZY panics: `.unwrap()`/`.expect()` give the operator no fix hint.
        assert!(
            !production.contains(".expect(") && !production.contains(".unwrap("),
            "Fix: NFA compile wrapper must not use bare .unwrap()/.expect() — use an explicit panic!() with a fix hint."
        );
        // No SILENT fallback: the old `eprintln! + empty_plan()` arm swallowed
        // the error and returned a scanner that matches nothing (Law 10).
        assert!(
            !production.contains("eprintln!(\"vyre-libs NFA compile failed"),
            "Fix: NFA compile must not log-and-return an empty plan on error — fail loud via panic!() so callers use try_compile."
        );
        // Fail-loud is present.
        assert!(
            production.contains("panic!("),
            "Fix: NFA compile must panic!() on an unrepresentable plan, never fabricate an empty entry-only plan."
        );
    }
}
