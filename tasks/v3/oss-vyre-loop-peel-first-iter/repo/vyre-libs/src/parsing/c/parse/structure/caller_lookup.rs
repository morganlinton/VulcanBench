use super::*;

pub(super) fn emit_enclosing_function_lookup(
    functions: &str,
    num_functions: Expr,
    token_idx: Expr,
) -> Vec<Node> {
    vec![
        Node::let_bind("caller_id", Expr::u32(u32::MAX)),
        Node::loop_for(
            "caller_fn_scan",
            Expr::u32(0),
            num_functions,
            vec![
                Node::let_bind(
                    "fn_rec_base",
                    Expr::mul(Expr::var("caller_fn_scan"), Expr::u32(3)),
                ),
                Node::let_bind(
                    "fn_body_end",
                    Expr::load(functions, Expr::add(Expr::var("fn_rec_base"), Expr::u32(2))),
                ),
                Node::let_bind(
                    "fn_body_start",
                    Expr::load(functions, Expr::add(Expr::var("fn_rec_base"), Expr::u32(1))),
                ),
                Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("caller_id"), Expr::u32(u32::MAX)),
                        Expr::and(
                            Expr::ge(token_idx.clone(), Expr::var("fn_body_start")),
                            Expr::le(token_idx.clone(), Expr::var("fn_body_end")),
                        ),
                    ),
                    vec![Node::assign("caller_id", Expr::var("caller_fn_scan"))],
                ),
            ],
        ),
    ]
}

#[cfg(test)]
mod tests {
    #[test]
    fn function_record_body_loads_avoid_adjacent_vector_load_fusion() {
        let source = include_str!("caller_lookup.rs");
        let end_pos = source
            .find("\"fn_body_end\"")
            .expect("Fix: malformed C structure lookup must return parse Err, not panic - function lookup binds body end");
        let start_pos = source
            .find("\"fn_body_start\"")
            .expect("Fix: malformed C structure lookup must return parse Err, not panic - function lookup binds body start");
        assert!(
            end_pos < start_pos,
            "Fix: 3-word function records must not load body_start/body_end in adjacent ascending order because PTX vector-load fusion requires stronger alignment than record stride 3 provides."
        );
    }
}
