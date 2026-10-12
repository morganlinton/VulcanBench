use super::Action;

/// A single grammar production: `lhs nonterminal -> rhs_len symbols`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Production {
    /// Nonterminal index on the left-hand side.
    pub lhs: u32,
    /// Number of symbols to pop from the parser stack on reduce.
    pub rhs_len: u32,
}

/// Precomputed LR tables backed by static slices.
///
/// Clone is `O(1)` because the payload is only scalar metadata plus slice
/// pointers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LrTables {
    /// Number of parser states.
    pub num_states: u32,
    /// Number of terminal symbols, including EOF.
    pub num_tokens: u32,
    /// Number of nonterminal symbols.
    pub num_nonterminals: u32,
    /// Flat action table: `action[state * num_tokens + token]`.
    pub action: &'static [u32],
    /// Flat goto table: `goto[state * num_nonterminals + nt]`.
    /// `u32::MAX` means "no goto".
    pub goto: &'static [u32],
    /// Production rules indexed by production id.
    pub productions: &'static [Production],
}

impl LrTables {
    /// Look up the action for `(state, token)`.
    ///
    /// # Panics
    ///
    /// Panics if `state` or `token` are out of bounds. Public parser entry
    /// points validate hostile token streams before calling this method.
    #[must_use]
    #[inline]
    pub fn action_at(&self, state: u32, token: u32) -> Action {
        let idx = (state * self.num_tokens + token) as usize;
        Action::unpack(self.action[idx])
    }

    /// Look up the goto for `(state, nonterminal)`. `u32::MAX` means no goto.
    ///
    /// # Panics
    ///
    /// Panics if `state` or `nt` are out of bounds.
    #[must_use]
    #[inline]
    pub fn goto_at(&self, state: u32, nt: u32) -> u32 {
        let idx = (state * self.num_nonterminals + nt) as usize;
        self.goto[idx]
    }
}
