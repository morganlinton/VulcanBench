use super::*;

pub(crate) fn emit_precomputed_declaration_kind_for_index(
    vast_nodes: &str,
    decl_contexts: &str,
    idx: Expr,
    out_name: &str,
    prefix: &str,
) -> Vec<Node> {
    let base = format!("{prefix}_base");
    let kind = format!("{prefix}_row_kind");
    let context_base = format!("{prefix}_context_base");
    let prefix_start = format!("{prefix}_prefix_start");
    let prefix_count = format!("{prefix}_prefix_count");
    let prefix_scan = format!("{prefix}_prefix_scan");
    let prefix_idx = format!("{prefix}_prefix_idx");
    let prefix_base = format!("{prefix}_prefix_base");
    let prefix_kind = format!("{prefix}_prefix_kind");
    let has_typedef = format!("{prefix}_has_typedef");
    let has_type = format!("{prefix}_has_type");
    let prev_idx = format!("{prefix}_prev_idx");
    let prev_base = format!("{prefix}_prev_base");
    let prev_kind = format!("{prefix}_prev_kind");
    let next_idx = format!("{prefix}_next_idx");
    let next_base = format!("{prefix}_next_base");
    let next_kind = format!("{prefix}_next_kind");
    let possible_declarator = format!("{prefix}_possible_declarator");

    vec![
        Node::let_bind(out_name, Expr::u32(0)),
        Node::let_bind(&base, vast_row_base_expr(idx.clone())),
        Node::let_bind(&kind, Expr::load(vast_nodes, Expr::var(&base))),
        Node::let_bind(
            &context_base,
            Expr::mul(idx.clone(), Expr::u32(VAST_DECL_CONTEXT_STRIDE_U32)),
        ),
        Node::let_bind(
            &prefix_start,
            Expr::load(
                decl_contexts,
                Expr::add(
                    Expr::var(&context_base),
                    Expr::u32(VAST_DECL_CONTEXT_PREFIX_START_FIELD),
                ),
            ),
        ),
        Node::let_bind(
            &prefix_count,
            Expr::select(
                Expr::gt(idx.clone(), Expr::var(&prefix_start)),
                Expr::sub(idx.clone(), Expr::var(&prefix_start)),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(&has_typedef, Expr::u32(0)),
        Node::let_bind(&has_type, Expr::u32(0)),
        Node::loop_for(
            &prefix_scan,
            Expr::u32(0),
            Expr::var(&prefix_count),
            vec![
                Node::let_bind(
                    &prefix_idx,
                    Expr::add(Expr::var(&prefix_start), Expr::var(&prefix_scan)),
                ),
                Node::let_bind(&prefix_base, vast_row_base_expr(Expr::var(&prefix_idx))),
                Node::let_bind(
                    &prefix_kind,
                    Expr::load(vast_nodes, Expr::var(&prefix_base)),
                ),
                Node::if_then(
                    Expr::eq(Expr::var(&prefix_kind), Expr::u32(TOK_TYPEDEF)),
                    vec![Node::assign(&has_typedef, Expr::u32(1))],
                ),
                Node::if_then(
                    any_token_eq(
                        Expr::var(&prefix_kind),
                        &[
                            TOK_INT,
                            TOK_CHAR_KW,
                            TOK_VOID,
                            TOK_DOUBLE,
                            TOK_FLOAT_KW,
                            TOK_LONG,
                            TOK_SHORT,
                            TOK_SIGNED,
                            TOK_UNSIGNED,
                            TOK_BOOL,
                            TOK_STRUCT,
                            TOK_UNION,
                            TOK_ENUM,
                            TOK_TYPEDEF,
                            TOK_AUTO,
                            TOK_CONST,
                            TOK_VOLATILE,
                            TOK_STATIC,
                            TOK_EXTERN,
                            TOK_REGISTER,
                            TOK_RESTRICT,
                            TOK_INLINE,
                            TOK_ALIGNAS,
                        ],
                    ),
                    vec![Node::assign(&has_type, Expr::u32(1))],
                ),
            ],
        ),
        Node::let_bind(
            &prev_idx,
            Expr::select(
                Expr::gt(idx.clone(), Expr::u32(0)),
                Expr::sub(idx.clone(), Expr::u32(1)),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(&prev_base, vast_row_base_expr(Expr::var(&prev_idx))),
        Node::let_bind(
            &prev_kind,
            Expr::select(
                Expr::gt(idx.clone(), Expr::u32(0)),
                Expr::load(vast_nodes, Expr::var(&prev_base)),
                Expr::u32(SENTINEL),
            ),
        ),
        Node::let_bind(
            &next_idx,
            Expr::select(
                Expr::lt(
                    Expr::add(idx.clone(), Expr::u32(1)),
                    Expr::var("annot_num_nodes"),
                ),
                Expr::add(idx.clone(), Expr::u32(1)),
                idx,
            ),
        ),
        Node::let_bind(&next_base, vast_row_base_expr(Expr::var(&next_idx))),
        Node::let_bind(&next_kind, Expr::load(vast_nodes, Expr::var(&next_base))),
        Node::let_bind(
            &possible_declarator,
            is_declaration_candidate_follower_token(Expr::var(&next_kind)),
        ),
        emit_declaration_kind_result_assignment(
            out_name,
            Expr::eq(Expr::var(&kind), Expr::u32(TOK_IDENTIFIER)),
            Expr::var(&possible_declarator),
            Expr::not(is_precomputed_declaration_previous_disqualifier_token(
                Expr::var(&prev_kind),
            )),
            Expr::ne(Expr::var(&next_kind), Expr::u32(TOK_COLON)),
            Expr::bool(true),
            Expr::eq(Expr::var(&has_typedef), Expr::u32(1)),
            Expr::eq(Expr::var(&has_type), Expr::u32(1)),
        ),
    ]
}
