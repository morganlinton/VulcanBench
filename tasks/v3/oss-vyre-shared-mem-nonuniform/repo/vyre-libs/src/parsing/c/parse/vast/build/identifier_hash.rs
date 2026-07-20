use super::*;

pub(crate) fn emit_identifier_hash_for_row(
    vast_nodes: &str,
    haystack: &str,
    haystack_len: &Expr,
    row_base: Expr,
    prefix: &str,
    packed_haystack: bool,
) -> Vec<Node> {
    let start = format!("{prefix}_start");
    let len = format!("{prefix}_len");
    let hash = format!("{prefix}_hash");
    let i = format!("{prefix}_i");
    let byte = format!("{prefix}_byte");

    vec![
        Node::let_bind(
            &start,
            Expr::load(vast_nodes, Expr::add(row_base.clone(), Expr::u32(5))),
        ),
        Node::let_bind(
            &len,
            Expr::load(vast_nodes, Expr::add(row_base.clone(), Expr::u32(6))),
        ),
        Node::let_bind(
            &hash,
            Expr::load(
                vast_nodes,
                Expr::add(row_base, Expr::u32(VAST_TYPEDEF_SYMBOL_FIELD)),
            ),
        ),
        Node::if_then(
            Expr::eq(Expr::var(&hash), Expr::u32(0)),
            vec![
                Node::assign(&hash, Expr::u32(0x811c9dc5)),
                Node::loop_for(
                    &i,
                    Expr::u32(0),
                    Expr::var(&len),
                    vec![Node::if_then(
                        Expr::lt(
                            Expr::add(Expr::var(&start), Expr::var(&i)),
                            haystack_len.clone(),
                        ),
                        vec![
                            Node::let_bind(
                                &byte,
                                load_source_byte(
                                    haystack,
                                    Expr::add(Expr::var(&start), Expr::var(&i)),
                                    packed_haystack,
                                ),
                            ),
                            Node::assign(&hash, Expr::bitxor(Expr::var(&hash), Expr::var(&byte))),
                            Node::assign(&hash, Expr::mul(Expr::var(&hash), Expr::u32(0x01000193))),
                        ],
                    )],
                ),
            ],
        ),
    ]
}
