use super::c11_expr::TOK_EOF;
use super::{Action, LrTables};

/// Errors emitted by the CPU reference LR parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The current state has no action for the lookahead token.
    UnexpectedToken {
        /// LR state at the error point.
        state: u32,
        /// Lookahead token id that had no action.
        token: u32,
        /// Token stream position.
        pos: usize,
    },
    /// The production id returned by the action table does not exist.
    InvalidProduction {
        /// Invalid production id.
        prod_id: u32,
    },
    /// Tried to pop states from an empty stack.
    StackUnderflow,
    /// The goto table has no entry for `(state, nonterminal)`.
    NoGoto {
        /// LR state after reduction.
        state: u32,
        /// Nonterminal id for the missing goto entry.
        nonterminal: u32,
    },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::UnexpectedToken { state, token, pos } => {
                write!(
                    f,
                    "LR unexpected token: state={state} token={token} pos={pos}. \
                     Fix: validate token stream against grammar or extend action table."
                )
            }
            ParseError::InvalidProduction { prod_id } => {
                write!(
                    f,
                    "LR invalid production id {prod_id} in action table. \
                     Fix: rebuild tables so every reduce action references a valid production."
                )
            }
            ParseError::StackUnderflow => {
                write!(
                    f,
                    "LR stack underflow on reduce. \
                     Fix: verify push/pop balance in grammar and that goto table matches."
                )
            }
            ParseError::NoGoto { state, nonterminal } => {
                write!(
                    f,
                    "LR missing goto: state={state} nt={nonterminal}. \
                     Fix: regenerate goto table from closure sets."
                )
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse a token stream using precomputed `LrTables`.
///
/// Returns the sequence of production ids that were reduced.
///
/// # Errors
///
/// Returns `ParseError` on syntax errors or internal table mismatches.
pub fn parse_lr(tables: &LrTables, tokens: &[u32]) -> Result<Vec<u32>, ParseError> {
    let mut stack: Vec<u32> = Vec::with_capacity(64);
    stack.push(0);
    let mut pos = 0usize;
    let mut reductions: Vec<u32> = Vec::with_capacity(tokens.len());

    loop {
        let state = *stack.last().ok_or(ParseError::StackUnderflow)?;
        let token = if pos < tokens.len() {
            tokens[pos]
        } else {
            TOK_EOF
        };
        if token >= tables.num_tokens {
            return Err(ParseError::UnexpectedToken { state, token, pos });
        }

        match tables.action_at(state, token) {
            Action::Accept => return Ok(reductions),
            Action::Shift(next_state) => {
                stack.push(next_state);
                pos += 1;
            }
            Action::Reduce(prod_id) => {
                let prod = tables
                    .productions
                    .get(prod_id as usize)
                    .ok_or(ParseError::InvalidProduction { prod_id })?;
                if prod_id == 0 {
                    return Ok(reductions);
                }
                if stack.len() <= prod.rhs_len as usize {
                    return Err(ParseError::StackUnderflow);
                }
                for _ in 0..prod.rhs_len {
                    stack.pop();
                }
                let new_state = *stack.last().ok_or(ParseError::StackUnderflow)?;
                if prod.lhs >= tables.num_nonterminals {
                    return Err(ParseError::NoGoto {
                        state: new_state,
                        nonterminal: prod.lhs,
                    });
                }
                let goto_state = tables.goto_at(new_state, prod.lhs);
                if goto_state == u32::MAX {
                    return Err(ParseError::NoGoto {
                        state: new_state,
                        nonterminal: prod.lhs,
                    });
                }
                stack.push(goto_state);
                reductions.push(prod_id);
            }
            Action::Error => {
                return Err(ParseError::UnexpectedToken { state, token, pos });
            }
        }
    }
}
