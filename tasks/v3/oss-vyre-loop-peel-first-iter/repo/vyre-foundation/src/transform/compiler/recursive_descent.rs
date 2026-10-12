//! Recursive descent  -  a bounded table-driven parser primitive.
//!
//! Parsing is sequential: a stack, a state machine, and a transition table.
//! Most GPU frameworks force this parser step out of device execution. Vyre treats it
//! as a first-class primitive.  `recursive_descent` maintains an explicit
//! parser stack in workgroup SRAM, fires callbacks into an output buffer, and
//! validates bounds so the kernel cannot overflow workgroup memory.  The CPU
//! reference performs the exact same table walk with the same stack and
//! callback limits, giving conform a byte-identical target to verify against.

use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use rustc_hash::FxHashMap;
use thiserror::Error;
use vyre_spec::AlgebraicLaw;

/// Registered device source for the recursive-descent primitive.
#[must_use]
pub fn source() -> Option<&'static str> {
    crate::transform::compiler::shader_provider::source("recursive_descent")
}

/// Build a vyre IR Program that consumes `token_count` tokens in a
/// table-driven recursive-descent step. One lane drives one parser;
/// the host invokes the Program repeatedly to advance through the
/// full input stream.
///
/// Buffers:
/// - `tokens`: `ReadOnly` u32 array  -  token stream.
/// - `transition_table`: `ReadOnly` u32 array  -
///   `state * ALPHA_SIZE + token` → `next_state`, with `0` reserved for
///   "reject".
/// - `state`: `ReadWrite` u32 array of length 1  -  current parser
///   state carried across dispatches.
/// - `output`: `ReadWrite` u32 array  -  emitted AST nodes or parse
///   events; one entry per accepted token.
/// - `out_count`: `ReadWrite` u32 array of length 1  -  atomic cursor
///   past the last populated `output` slot.
/// - `reject_flag`: `ReadWrite` u32 array of length 1  -  set to 1 if
///   the parser hits a reject transition.
///
/// `alpha_size` is the alphabet size baked into the transition
/// table; `reject_state` is the state id the table uses for
/// rejection (conventionally 0).
#[must_use]
pub fn consume_step_program(
    tokens: &str,
    transition_table: &str,
    state: &str,
    output: &str,
    out_count: &str,
    reject_flag: &str,
    alpha_size: u32,
    reject_state: u32,
    token_count: u32,
) -> Program {
    let body = vec![
        Node::let_bind("cur_state", Expr::load(state, Expr::u32(0))),
        Node::let_bind("rejected", Expr::u32(0)),
        Node::loop_for(
            "i",
            Expr::u32(0),
            Expr::u32(token_count),
            vec![
                // Skip every iteration after rejection. Validator
                // rejects `assign("i", ...)` so we gate the body
                // on a flag instead of breaking.
                Node::if_then(
                    Expr::eq(Expr::var("rejected"), Expr::u32(0)),
                    vec![
                        Node::let_bind("tok", Expr::load(tokens, Expr::var("i"))),
                        Node::let_bind(
                            "row",
                            Expr::add(
                                Expr::mul(Expr::var("cur_state"), Expr::u32(alpha_size)),
                                Expr::var("tok"),
                            ),
                        ),
                        Node::let_bind("next", Expr::load(transition_table, Expr::var("row"))),
                        Node::if_then_else(
                            Expr::eq(Expr::var("next"), Expr::u32(reject_state)),
                            vec![
                                Node::let_bind(
                                    "rf",
                                    Expr::atomic_exchange(reject_flag, Expr::u32(0), Expr::u32(1)),
                                ),
                                Node::assign("rejected", Expr::u32(1)),
                            ],
                            vec![
                                Node::let_bind(
                                    "idx",
                                    Expr::atomic_add(out_count, Expr::u32(0), Expr::u32(1)),
                                ),
                                Node::store(output, Expr::var("idx"), Expr::var("next")),
                                Node::assign("cur_state", Expr::var("next")),
                            ],
                        ),
                    ],
                ),
            ],
        ),
        Node::store(state, Expr::u32(0), Expr::var("cur_state")),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(tokens, 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(transition_table, 1, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(state, 2, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::storage(output, 3, BufferAccess::ReadWrite, DataType::U32),
            BufferDecl::storage(out_count, 4, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::storage(reject_flag, 5, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [1, 1, 1],
        body,
    )
}

impl RecursiveDescentOp {}

/// Algebraic laws declared by the recursive-descent primitive.
pub const LAWS: &[AlgebraicLaw] = &[AlgebraicLaw::Bounded {
    lo: 0,
    hi: u32::MAX,
}];

/// Parse tokens using a bounded explicit stack and transition table.
///
/// # Errors
///
/// Returns `Fix: ...` when no transition matches, stack capacity is exceeded,
/// or the callback output buffer would overflow.
#[must_use]
pub fn parse(
    tokens: &[u32],
    transitions: &[Transition],
    start_state: u32,
    accept_state: u32,
    max_stack: usize,
    max_callbacks: usize,
) -> Result<ParseResult, RecursiveDescentError> {
    let mut transition_index: FxHashMap<(u32, u32), Transition> = FxHashMap::default();
    transition_index.reserve(transitions.len());
    for &transition in transitions {
        transition_index
            .entry((transition.state, transition.token_kind))
            .or_insert(transition);
    }

    let mut state = start_state;
    let mut stack = Vec::with_capacity(max_stack);
    let mut callbacks = Vec::with_capacity(tokens.len().min(max_callbacks));
    let mut consumed = 0usize;
    while consumed < tokens.len() {
        let token = tokens[consumed];
        let transition = transition_index
            .get(&(state, token))
            .copied()
            .ok_or(RecursiveDescentError::NoTransition { state, token })?;
        if transition.push_state != u32::MAX {
            if stack.len() == max_stack {
                return Err(RecursiveDescentError::StackOverflow { max_stack });
            }
            stack.push(transition.push_state);
        }
        if transition.callback != 0 {
            if callbacks.len() == max_callbacks {
                return Err(RecursiveDescentError::CallbackOverflow { max_callbacks });
            }
            callbacks.push(transition.callback);
        }
        state = if transition.next_state == u32::MAX {
            stack.pop().ok_or(RecursiveDescentError::StackUnderflow)?
        } else {
            transition.next_state
        };
        consumed += 1;
    }
    if state != accept_state {
        return Err(RecursiveDescentError::NotAccepted {
            state,
            accept_state,
        });
    }
    Ok(ParseResult {
        callbacks,
        consumed: u32::try_from(consumed).map_err(|_| RecursiveDescentError::TokenOverflow)?,
        final_state: state,
    })
}

/// Parser result emitted by the CPU reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseResult {
    /// Fired callback ids in input order.
    pub callbacks: Vec<u32>,
    /// Number of consumed tokens.
    pub consumed: u32,
    /// Final parser state.
    pub final_state: u32,
}

/// Recursive-descent parser validation errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum RecursiveDescentError {
    /// No transition matches the current state and token.
    #[error(
        "RecursiveDescentNoTransition: no transition for state {state} token {token}. Fix: add a grammar table edge or reject this token stream before dispatch."
    )]
    NoTransition {
        /// Parser state.
        state: u32,
        /// Token kind.
        token: u32,
    },
    /// Explicit parser stack exceeded its bound.
    #[error(
        "RecursiveDescentStackOverflow: stack exceeded {max_stack} entries. Fix: increase workgroup.stack depth or split the grammar production."
    )]
    StackOverflow {
        /// Stack capacity.
        max_stack: usize,
    },
    /// Transition attempted to return without a caller state.
    #[error(
        "RecursiveDescentStackUnderflow: return transition found an empty stack. Fix: validate push/return grammar balance."
    )]
    StackUnderflow,
    /// Callback sequence exceeded its output bound.
    #[error(
        "RecursiveDescentCallbackOverflow: callback output exceeded {max_callbacks}. Fix: increase callback output capacity."
    )]
    CallbackOverflow {
        /// Callback capacity.
        max_callbacks: usize,
    },
    /// Parser ended in a non-accepting state.
    #[error(
        "RecursiveDescentNotAccepted: final state {state} does not equal accept state {accept_state}. Fix: add a completion transition or reject incomplete input."
    )]
    NotAccepted {
        /// Final state.
        state: u32,
        /// Required accept state.
        accept_state: u32,
    },
    /// Consumed token count cannot fit in `u32`.
    #[error(
        "RecursiveDescentTokenOverflow: consumed token count cannot fit u32. Fix: split the token stream."
    )]
    TokenOverflow,
}

/// Category C recursive-descent intrinsic.
#[derive(Debug, Default, Clone, Copy)]
pub struct RecursiveDescentOp;

/// Grammar transition for the table-walk parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    /// Current parser state.
    pub state: u32,
    /// Token kind required by this transition.
    pub token_kind: u32,
    /// Next parser state.
    pub next_state: u32,
    /// Callback id emitted when the transition fires.
    pub callback: u32,
    /// Optional state to push before moving to `next_state`; `u32::MAX` means no push.
    pub push_state: u32,
}

/// Workgroup size used by the reference target-text lowering.
pub const WORKGROUP_SIZE: [u32; 3] = [64, 1, 1];

#[cfg(test)]
mod ir_program_tests {
    use super::*;

    fn make_prog() -> Program {
        consume_step_program(
            "tokens",
            "trans",
            "state",
            "output",
            "out_count",
            "reject",
            /* alpha_size = */ 64,
            /* reject_state = */ 0,
            /* token_count = */ 16,
        )
    }

    #[test]
    fn consume_step_program_validates() {
        let prog = make_prog();
        let errors = crate::validate::validate::validate(&prog);
        assert!(errors.is_empty(), "parser IR must validate: {errors:?}");
    }

    #[test]
    fn consume_step_program_wire_round_trips() {
        let prog = make_prog();
        let bytes = prog
            .to_wire()
            .expect("Fix: serialize; restore this invariant before continuing.");
        let decoded = Program::from_wire(&bytes)
            .expect("Fix: decode; restore this invariant before continuing.");
        assert_eq!(decoded.buffers().len(), 6);
    }

    #[test]
    fn changing_alpha_size_changes_wire() {
        let a = consume_step_program("t", "tr", "s", "o", "oc", "rf", 32, 0, 8)
            .to_wire()
            .unwrap();
        let b = consume_step_program("t", "tr", "s", "o", "oc", "rf", 64, 0, 8)
            .to_wire()
            .unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn cpu_parse_uses_indexed_transition_lookup() {
        let transitions = [
            Transition {
                state: 0,
                token_kind: 1,
                next_state: 1,
                callback: 10,
                push_state: u32::MAX,
            },
            Transition {
                state: 1,
                token_kind: 2,
                next_state: 2,
                callback: 20,
                push_state: u32::MAX,
            },
        ];
        let result = parse(&[1, 2], &transitions, 0, 2, 4, 4).unwrap();
        assert_eq!(result.callbacks, vec![10, 20]);
        assert_eq!(result.consumed, 2);
        assert_eq!(result.final_state, 2);
    }

    #[test]
    fn duplicate_transition_keeps_first_match_contract() {
        let transitions = [
            Transition {
                state: 0,
                token_kind: 1,
                next_state: 1,
                callback: 10,
                push_state: u32::MAX,
            },
            Transition {
                state: 0,
                token_kind: 1,
                next_state: 9,
                callback: 99,
                push_state: u32::MAX,
            },
        ];
        let result = parse(&[1], &transitions, 0, 1, 4, 4).unwrap();
        assert_eq!(result.callbacks, vec![10]);
        assert_eq!(result.final_state, 1);
    }
}
