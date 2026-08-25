use crate::parsing::c::lex::tokens::TOK_IDENTIFIER;
use crate::parsing::c::source_bytes::load_source_byte;
use vyre::ir::{Expr, Node};
use vyre_primitives::hash::fnv1a::{fnv1a32_initial_expr, fnv1a32_update_byte_node};

/// Emit IR that interns an identifier token by hashing its source bytes.
pub fn emit_identifier_intern(
    tok_starts: &str,
    tok_lens: &str,
    haystack: &str,
    node_idx: Expr,
    packed_haystack: bool,
) -> Vec<Node> {
    vec![
        Node::let_bind("identifier_intern_id", Expr::u32(0)),
        Node::if_then(
            Expr::and(
                Expr::eq(Expr::var("tok_type"), Expr::u32(TOK_IDENTIFIER)),
                Expr::gt(Expr::load(tok_lens, node_idx.clone()), Expr::u32(0)),
            ),
            vec![
                Node::let_bind("start", Expr::load(tok_starts, node_idx.clone())),
                Node::let_bind("len", Expr::load(tok_lens, node_idx)),
                Node::let_bind("hash", fnv1a32_initial_expr()),
                Node::loop_for(
                    "intern_scan",
                    Expr::u32(0),
                    Expr::var("len"),
                    vec![
                        Node::let_bind(
                            "byte",
                            load_source_byte(
                                haystack,
                                Expr::add(Expr::var("start"), Expr::var("intern_scan")),
                                packed_haystack,
                            ),
                        ),
                        fnv1a32_update_byte_node("hash", Expr::var("byte")),
                    ],
                ),
                Node::assign("identifier_intern_id", Expr::var("hash")),
            ],
        ),
    ]
}
