//! External contract tests for LR table shape, parser errors, and reentrancy.

use std::thread;

use vyre_libs::parsing::lr_tables::{
    parse_lr, Action, ParseError, C11_EXPR, TOK_EOF, TOK_ID, TOK_LPAREN, TOK_MINUS, TOK_NUM,
    TOK_PLUS, TOK_RPAREN, TOK_STAR,
};

#[test]
fn action_table_length_matches_dimensions() {
    assert_eq!(
        C11_EXPR.action.len() as u32,
        C11_EXPR.num_states * C11_EXPR.num_tokens,
        "ACTION table size must equal num_states * num_tokens"
    );
}

#[test]
fn goto_table_length_matches_dimensions() {
    assert_eq!(
        C11_EXPR.goto.len() as u32,
        C11_EXPR.num_states * C11_EXPR.num_nonterminals,
        "GOTO table size must equal num_states * num_nonterminals"
    );
}

#[test]
fn every_action_is_valid_encoding() {
    for (idx, &word) in C11_EXPR.action.iter().enumerate() {
        let tag = word >> 28;
        assert!(
            tag <= 3,
            "ACTION[{idx}] has invalid tag {tag}: word={word:#010x}"
        );
    }
}

#[test]
fn goto_entries_are_valid_states_or_sentinel() {
    for (idx, &word) in C11_EXPR.goto.iter().enumerate() {
        assert!(
            word == u32::MAX || word < C11_EXPR.num_states,
            "GOTO[{idx}] = {word} is neither u32::MAX nor < num_states"
        );
    }
}

#[test]
fn action_pack_roundtrip() {
    let cases = [
        Action::Shift(0),
        Action::Shift(42),
        Action::Shift(0x0FFF_FFFF),
        Action::Reduce(0),
        Action::Reduce(99),
        Action::Reduce(0x0FFF_FFFF),
        Action::Accept,
        Action::Error,
    ];
    for action in &cases {
        assert_eq!(
            Action::unpack(action.pack()),
            *action,
            "roundtrip failed for {action:?}"
        );
    }
}

#[test]
fn single_id() {
    let toks = [TOK_ID, TOK_EOF];
    let red = parse_lr(&C11_EXPR, &toks).expect("id is a valid expression");
    assert_eq!(red, vec![8, 6, 3]);
}

#[test]
fn id_plus_num() {
    let toks = [TOK_ID, TOK_PLUS, TOK_NUM, TOK_EOF];
    let red = parse_lr(&C11_EXPR, &toks).expect("id+num is valid");
    assert_eq!(red, vec![8, 6, 3, 9, 6, 1]);
}

#[test]
fn id_star_num() {
    let toks = [TOK_ID, TOK_STAR, TOK_NUM, TOK_EOF];
    let red = parse_lr(&C11_EXPR, &toks).expect("id*num is valid");
    assert_eq!(red, vec![8, 6, 9, 4, 3]);
}

#[test]
fn paren_expr() {
    let toks = [TOK_LPAREN, TOK_ID, TOK_PLUS, TOK_NUM, TOK_RPAREN, TOK_EOF];
    let red = parse_lr(&C11_EXPR, &toks).expect("(id+num) is valid");
    assert_eq!(red, vec![8, 6, 3, 9, 6, 1, 7, 6, 3]);
}

#[test]
fn mixed_precedence() {
    let toks = [TOK_ID, TOK_PLUS, TOK_NUM, TOK_STAR, TOK_ID, TOK_EOF];
    let red = parse_lr(&C11_EXPR, &toks).expect("mixed precedence is valid");
    assert_eq!(red, vec![8, 6, 3, 9, 6, 8, 4, 1]);
}

#[test]
fn chained_left_associative() {
    let toks = [TOK_ID, TOK_MINUS, TOK_NUM, TOK_MINUS, TOK_NUM, TOK_EOF];
    let red = parse_lr(&C11_EXPR, &toks).expect("chained minus is valid");
    assert_eq!(red, vec![8, 6, 3, 9, 6, 2, 9, 6, 2]);
}

#[test]
fn empty_input_is_error() {
    let toks = [TOK_EOF];
    let err = parse_lr(&C11_EXPR, &toks).expect_err("empty input should error");
    assert!(
        matches!(
            err,
            ParseError::UnexpectedToken {
                state: 0,
                token: TOK_EOF,
                pos: 0
            }
        ),
        "unexpected error variant: {err}"
    );
}

#[test]
fn double_operator_is_error() {
    let toks = [TOK_ID, TOK_PLUS, TOK_PLUS, TOK_NUM, TOK_EOF];
    let err = parse_lr(&C11_EXPR, &toks).expect_err("double operator should error");
    assert!(
        matches!(
            err,
            ParseError::UnexpectedToken {
                state: 7,
                token: TOK_PLUS,
                pos: 2
            }
        ),
        "unexpected error variant: {err}"
    );
}

#[test]
fn unmatched_lparen_is_error() {
    let toks = [TOK_LPAREN, TOK_ID, TOK_EOF];
    let err = parse_lr(&C11_EXPR, &toks).expect_err("unmatched lparen should error");
    assert!(
        matches!(
            err,
            ParseError::UnexpectedToken {
                state: 11,
                token: TOK_EOF,
                pos: 2
            }
        ),
        "unexpected error variant: {err}"
    );
}

#[test]
fn unmatched_rparen_is_error() {
    let toks = [TOK_RPAREN, TOK_ID, TOK_EOF];
    let err = parse_lr(&C11_EXPR, &toks).expect_err("unmatched rparen should error");
    assert!(
        matches!(
            err,
            ParseError::UnexpectedToken {
                state: 0,
                token: TOK_RPAREN,
                pos: 0
            }
        ),
        "unexpected error variant: {err}"
    );
}

#[test]
fn trailing_operator_is_error() {
    let toks = [TOK_ID, TOK_PLUS, TOK_EOF];
    let err = parse_lr(&C11_EXPR, &toks).expect_err("trailing operator should error");
    assert!(
        matches!(
            err,
            ParseError::UnexpectedToken {
                state: 7,
                token: TOK_EOF,
                pos: 2
            }
        ),
        "unexpected error variant: {err}"
    );
}

#[test]
fn unknown_token_is_structured_error_not_panic() {
    let toks = [TOK_ID, u32::MAX, TOK_EOF];
    let err = parse_lr(&C11_EXPR, &toks).expect_err("unknown token should error");
    assert!(
        matches!(
            err,
            ParseError::UnexpectedToken {
                state: 4,
                token: u32::MAX,
                pos: 1
            }
        ),
        "unexpected error variant: {err}"
    );
}

#[test]
fn concurrent_parsing_does_not_corrupt_tables() {
    let handles: Vec<_> = (0..64)
        .map(|i| {
            thread::spawn(move || {
                if i % 2 == 0 {
                    let toks = [TOK_ID, TOK_PLUS, TOK_NUM, TOK_EOF];
                    parse_lr(&C11_EXPR, &toks).unwrap();
                } else {
                    let toks = [TOK_ID, TOK_PLUS, TOK_PLUS, TOK_NUM, TOK_EOF];
                    parse_lr(&C11_EXPR, &toks).unwrap_err();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("parser thread must not panic");
    }

    assert_eq!(C11_EXPR.num_states, 17);
    assert_eq!(Action::unpack(C11_EXPR.action[0]), Action::Shift(4));
}
