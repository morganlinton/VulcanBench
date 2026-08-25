use super::*;
pub(super) fn parse_expr_conditional(tokens: &[ExprTok], idx: &mut usize) -> i128 {
    parse_expr_conditional_active(tokens, idx, true)
}

pub(super) fn parse_expr_conditional_active(
    tokens: &[ExprTok],
    idx: &mut usize,
    active: bool,
) -> i128 {
    let cond = parse_expr_or(tokens, idx, active);
    if tokens.get(*idx) != Some(&ExprTok::Question) {
        return cond;
    }
    *idx += 1;
    let if_true = parse_expr_conditional_active(tokens, idx, active && cond != 0);
    if tokens.get(*idx) != Some(&ExprTok::Colon) {
        return if active && cond != 0 { if_true } else { 0 };
    }
    *idx += 1;
    let if_false = parse_expr_conditional_active(tokens, idx, active && cond == 0);
    if !active {
        0
    } else if cond != 0 {
        if_true
    } else {
        if_false
    }
}

pub(super) fn parse_expr_or(tokens: &[ExprTok], idx: &mut usize, active: bool) -> i128 {
    let mut lhs = parse_expr_and(tokens, idx, active);
    while tokens.get(*idx) == Some(&ExprTok::Or) {
        *idx += 1;
        let rhs = parse_expr_and(tokens, idx, active && lhs == 0);
        if active {
            lhs = i128::from(lhs != 0 || rhs != 0);
        }
    }
    lhs
}

pub(super) fn parse_expr_and(tokens: &[ExprTok], idx: &mut usize, active: bool) -> i128 {
    let mut lhs = parse_expr_bit_or(tokens, idx, active);
    while tokens.get(*idx) == Some(&ExprTok::And) {
        *idx += 1;
        let rhs = parse_expr_bit_or(tokens, idx, active && lhs != 0);
        if active {
            lhs = i128::from(lhs != 0 && rhs != 0);
        }
    }
    lhs
}

pub(super) fn parse_expr_bit_or(tokens: &[ExprTok], idx: &mut usize, active: bool) -> i128 {
    let mut lhs = parse_expr_bit_xor(tokens, idx, active);
    while tokens.get(*idx) == Some(&ExprTok::BitOr) {
        *idx += 1;
        let rhs = parse_expr_bit_xor(tokens, idx, active);
        if active {
            lhs |= rhs;
        }
    }
    lhs
}

pub(super) fn parse_expr_bit_xor(tokens: &[ExprTok], idx: &mut usize, active: bool) -> i128 {
    let mut lhs = parse_expr_bit_and(tokens, idx, active);
    while tokens.get(*idx) == Some(&ExprTok::BitXor) {
        *idx += 1;
        let rhs = parse_expr_bit_and(tokens, idx, active);
        if active {
            lhs ^= rhs;
        }
    }
    lhs
}

pub(super) fn parse_expr_bit_and(tokens: &[ExprTok], idx: &mut usize, active: bool) -> i128 {
    let mut lhs = parse_expr_eq(tokens, idx, active);
    while tokens.get(*idx) == Some(&ExprTok::BitAnd) {
        *idx += 1;
        let rhs = parse_expr_eq(tokens, idx, active);
        if active {
            lhs &= rhs;
        }
    }
    lhs
}

pub(super) fn parse_expr_eq(tokens: &[ExprTok], idx: &mut usize, active: bool) -> i128 {
    let mut lhs = parse_expr_rel(tokens, idx, active);
    loop {
        match tokens.get(*idx) {
            Some(ExprTok::Eq) => {
                *idx += 1;
                let rhs = parse_expr_rel(tokens, idx, active);
                if active {
                    lhs = i128::from(lhs == rhs);
                }
            }
            Some(ExprTok::Ne) => {
                *idx += 1;
                let rhs = parse_expr_rel(tokens, idx, active);
                if active {
                    lhs = i128::from(lhs != rhs);
                }
            }
            _ => return lhs,
        }
    }
}

pub(super) fn parse_expr_rel(tokens: &[ExprTok], idx: &mut usize, active: bool) -> i128 {
    let mut lhs = parse_expr_shift(tokens, idx, active);
    loop {
        match tokens.get(*idx) {
            Some(ExprTok::Lt) => {
                *idx += 1;
                let rhs = parse_expr_shift(tokens, idx, active);
                if active {
                    lhs = i128::from(lhs < rhs);
                }
            }
            Some(ExprTok::Le) => {
                *idx += 1;
                let rhs = parse_expr_shift(tokens, idx, active);
                if active {
                    lhs = i128::from(lhs <= rhs);
                }
            }
            Some(ExprTok::Gt) => {
                *idx += 1;
                let rhs = parse_expr_shift(tokens, idx, active);
                if active {
                    lhs = i128::from(lhs > rhs);
                }
            }
            Some(ExprTok::Ge) => {
                *idx += 1;
                let rhs = parse_expr_shift(tokens, idx, active);
                if active {
                    lhs = i128::from(lhs >= rhs);
                }
            }
            _ => return lhs,
        }
    }
}

pub(super) fn parse_expr_shift(tokens: &[ExprTok], idx: &mut usize, active: bool) -> i128 {
    let mut lhs = parse_expr_add(tokens, idx, active);
    loop {
        match tokens.get(*idx) {
            Some(ExprTok::Shl) => {
                *idx += 1;
                let rhs = parse_expr_add(tokens, idx, active);
                if active {
                    if rhs < 0 || rhs >= 128 {
                        panic!("exceeding the i128 evaluator width: left shift by {rhs}");
                    }
                    lhs = lhs.checked_shl(rhs as u32).unwrap_or(0);
                }
            }
            Some(ExprTok::Shr) => {
                *idx += 1;
                let rhs = parse_expr_add(tokens, idx, active);
                if active {
                    if rhs < 0 || rhs >= 128 {
                        panic!("exceeding the i128 evaluator width: right shift by {rhs}");
                    }
                    lhs = lhs.checked_shr(rhs as u32).unwrap_or(0);
                }
            }
            _ => return lhs,
        }
    }
}

pub(super) fn parse_expr_add(tokens: &[ExprTok], idx: &mut usize, active: bool) -> i128 {
    let mut lhs = parse_expr_mul(tokens, idx, active);
    loop {
        match tokens.get(*idx) {
            Some(ExprTok::Plus) => {
                *idx += 1;
                let rhs = parse_expr_mul(tokens, idx, active);
                if active {
                    lhs = lhs.wrapping_add(rhs);
                }
            }
            Some(ExprTok::Minus) => {
                *idx += 1;
                let rhs = parse_expr_mul(tokens, idx, active);
                if active {
                    lhs = lhs.wrapping_sub(rhs);
                }
            }
            _ => return lhs,
        }
    }
}

pub(super) fn parse_expr_mul(tokens: &[ExprTok], idx: &mut usize, active: bool) -> i128 {
    let mut lhs = parse_expr_unary(tokens, idx, active);
    loop {
        match tokens.get(*idx) {
            Some(ExprTok::Star) => {
                *idx += 1;
                let rhs = parse_expr_unary(tokens, idx, active);
                if active {
                    lhs = lhs.wrapping_mul(rhs);
                }
            }
            Some(ExprTok::Slash) => {
                *idx += 1;
                let rhs = parse_expr_unary(tokens, idx, active);
                if active && rhs == 0 {
                    lhs = 0;
                    continue;
                }
                if active {
                    lhs /= rhs;
                }
            }
            Some(ExprTok::Percent) => {
                *idx += 1;
                let rhs = parse_expr_unary(tokens, idx, active);
                if active && rhs == 0 {
                    lhs = 0;
                    continue;
                }
                if active {
                    lhs %= rhs;
                }
            }
            _ => return lhs,
        }
    }
}


pub(super) fn parse_expr_unary(tokens: &[ExprTok], idx: &mut usize, active: bool) -> i128 {
    match tokens.get(*idx) {
        Some(ExprTok::Not) => {
            *idx += 1;
            let value = parse_expr_unary(tokens, idx, active);
            if active {
                i128::from(value == 0)
            } else {
                0
            }
        }
        Some(ExprTok::Plus) => {
            *idx += 1;
            parse_expr_unary(tokens, idx, active)
        }
        Some(ExprTok::Minus) => {
            *idx += 1;
            let value = parse_expr_unary(tokens, idx, active);
            if active {
                value.wrapping_neg()
            } else {
                0
            }
        }
        Some(ExprTok::BitNot) => {
            *idx += 1;
            let value = parse_expr_unary(tokens, idx, active);
            if active {
                !value
            } else {
                0
            }
        }
        Some(ExprTok::LParen) => {
            *idx += 1;
            let value = parse_expr_conditional_active(tokens, idx, active);
            if tokens.get(*idx) == Some(&ExprTok::RParen) {
                *idx += 1;
            }
            value
        }
        Some(ExprTok::Num(value)) => {
            *idx += 1;
            *value
        }
        _ => 0,
    }
}

pub(in crate::tu_host::preprocess) fn eval_preproc_expr(
    expr: &str,
    macros: &HashMap<String, MacroDef>,
) -> bool {
    let tokens = tokenize_preproc_expr(expr, macros);
    let mut idx = 0usize;
    let value = parse_expr_conditional(&tokens, &mut idx);
    if idx != tokens.len() {
        let _ = expr;
        return false;
    }
    value != 0
}
