use crate::parsing::c::lex::tokens::*;
use crate::parsing::core::ast::node::*;
use vyre::ir::Expr;

pub(super) fn is_value_token(token: Expr) -> Expr {
    eq_any2(token, TOK_INTEGER, TOK_IDENTIFIER)
}

pub(super) fn is_assignment_token(token: Expr) -> Expr {
    Expr::or(
        eq_token(&token, TOK_ASSIGN),
        in_closed(&token, TOK_PLUS_EQ, TOK_SLASH_EQ),
    )
}

pub(super) fn precedence(token: Expr) -> Expr {
    Expr::select(
        is_assignment_token_ref(&token),
        Expr::u32(1),
        Expr::select(
            eq_token(&token, TOK_OR),
            Expr::u32(2),
            Expr::select(
                eq_token(&token, TOK_AND),
                Expr::u32(3),
                Expr::select(
                    in_closed(&token, TOK_EQ, TOK_NE),
                    Expr::u32(4),
                    Expr::select(
                        Expr::or(
                            in_closed(&token, TOK_LT, TOK_GT),
                            in_closed(&token, TOK_LE, TOK_GE),
                        ),
                        Expr::u32(5),
                        Expr::select(
                            in_closed(&token, TOK_PLUS, TOK_MINUS),
                            Expr::u32(6),
                            Expr::select(
                                in_closed(&token, TOK_STAR, TOK_PERCENT),
                                Expr::u32(7),
                                Expr::u32(0),
                            ),
                        ),
                    ),
                ),
            ),
        ),
    )
}

pub(super) fn ast_opcode(token: Expr) -> Expr {
    Expr::select(
        is_assignment_token_ref(&token),
        Expr::u32(AST_ASSIGN),
        Expr::select(
            in_closed(&token, TOK_MINUS, TOK_SLASH),
            offset_table(&token, TOK_MINUS, AST_SUB),
            Expr::select(
                eq_token(&token, TOK_PERCENT),
                Expr::u32(AST_MOD),
                Expr::select(
                    in_closed(&token, TOK_EQ, TOK_NE),
                    offset_table(&token, TOK_EQ, AST_EQ),
                    Expr::select(
                        in_closed(&token, TOK_LT, TOK_GT),
                        offset_table(&token, TOK_LT, AST_LT),
                        Expr::select(
                            in_closed(&token, TOK_LE, TOK_GE),
                            offset_table(&token, TOK_LE, AST_LE),
                            Expr::select(
                                in_closed(&token, TOK_AND, TOK_OR),
                                offset_table(&token, TOK_AND, AST_LOGICAL_AND),
                                Expr::u32(AST_ADD),
                            ),
                        ),
                    ),
                ),
            ),
        ),
    )
}

fn eq_token(token: &Expr, expected: u32) -> Expr {
    Expr::eq(token.clone(), Expr::u32(expected))
}

fn eq_any2(token: Expr, a: u32, b: u32) -> Expr {
    Expr::or(eq_token(&token, a), Expr::eq(token, Expr::u32(b)))
}

fn in_closed(token: &Expr, lo: u32, hi: u32) -> Expr {
    Expr::and(
        Expr::ge(token.clone(), Expr::u32(lo)),
        Expr::le(token.clone(), Expr::u32(hi)),
    )
}

fn is_assignment_token_ref(token: &Expr) -> Expr {
    Expr::or(
        eq_token(token, TOK_ASSIGN),
        in_closed(token, TOK_PLUS_EQ, TOK_SLASH_EQ),
    )
}

fn offset_table(token: &Expr, token_base: u32, opcode_base: u32) -> Expr {
    Expr::add(
        Expr::sub(token.clone(), Expr::u32(token_base)),
        Expr::u32(opcode_base),
    )
}

pub(super) fn should_pop_cached(
    top: Expr,
    top_prec: Expr,
    current_prec: Expr,
    current_is_assignment: Expr,
) -> Expr {
    Expr::and(
        Expr::and(
            Expr::ne(top, Expr::u32(TOK_LPAREN)),
            Expr::ne(top_prec.clone(), Expr::u32(0)),
        ),
        Expr::select(
            current_is_assignment,
            Expr::gt(top_prec.clone(), current_prec.clone()),
            Expr::ge(top_prec, current_prec),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precedence_uses_range_checks_instead_of_per_token_selects() {
        let expr = precedence(Expr::var("tok"));
        assert!(
            count_selects(&expr) <= 7,
            "Fix: precedence IR must stay range-grouped instead of rebuilding one select per token."
        );
    }

    #[test]
    fn ast_opcode_uses_offset_tables_instead_of_per_opcode_selects() {
        let expr = ast_opcode(Expr::var("tok"));
        assert!(
            count_selects(&expr) <= 7,
            "Fix: ast opcode IR must stay table/range-built instead of rebuilding one select per opcode."
        );
    }

    fn count_selects(expr: &Expr) -> usize {
        let mut count = 0;
        let mut stack = vec![expr];
        while let Some(expr) = stack.pop() {
            match expr {
                Expr::Select {
                    cond,
                    true_val,
                    false_val,
                } => {
                    count += 1;
                    stack.push(cond);
                    stack.push(true_val);
                    stack.push(false_val);
                }
                Expr::Load { index, .. } | Expr::UnOp { operand: index, .. } => stack.push(index),
                Expr::BinOp { left, right, .. } => {
                    stack.push(left);
                    stack.push(right);
                }
                Expr::Call { args, .. } => stack.extend(args),
                Expr::Cast { value, .. } => stack.push(value),
                Expr::Fma { a, b, c } => {
                    stack.push(a);
                    stack.push(b);
                    stack.push(c);
                }
                Expr::Atomic {
                    index,
                    expected,
                    value,
                    ..
                } => {
                    stack.push(index);
                    if let Some(expected) = expected {
                        stack.push(expected);
                    }
                    stack.push(value);
                }
                Expr::SubgroupBallot { cond } => stack.push(cond),
                Expr::SubgroupShuffle { value, lane } => {
                    stack.push(value);
                    stack.push(lane);
                }
                Expr::SubgroupReduce { value, .. } => stack.push(value),
                Expr::LitU32(_)
                | Expr::LitI32(_)
                | Expr::LitF32(_)
                | Expr::LitBool(_)
                | Expr::Var(_)
                | Expr::BufLen { .. }
                | Expr::InvocationId { .. }
                | Expr::WorkgroupId { .. }
                | Expr::LocalId { .. }
                | Expr::SubgroupLocalId
                | Expr::SubgroupSize
                | Expr::Opaque(_)
                | _ => {}
            }
        }
        count
    }
}
