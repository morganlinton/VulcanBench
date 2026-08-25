//! Identifier and GNU/Clang builtin-call parsing helpers for the GPU #if evaluator.

use vyre::ir::{Expr, Node};

use super::super::gpu_source_bytes::SourceByteLayout;
use super::byte_load::safe_load_src_expr;
use vyre_primitives::hash::fnv1a::{fnv1a32, fnv1a32_initial_expr, fnv1a32_update_byte_expr};

fn fnv1a32_bytes(bytes: &[u8]) -> u32 {
    fnv1a32(bytes)
}

pub(super) fn ident_hash_equals(bytes: &'static [u8]) -> Expr {
    Expr::and(
        Expr::eq(Expr::var("ident_len"), Expr::u32(bytes.len() as u32)),
        Expr::eq(Expr::var("ident_hash"), Expr::u32(fnv1a32_bytes(bytes))),
    )
}

fn source_at_equals(
    start_var: &'static str,
    bytes: &'static [u8],
    source_layout: SourceByteLayout,
    source_byte_len: Expr,
) -> Expr {
    let mut expr = Expr::eq(Expr::u32(1), Expr::u32(1));
    for (idx, byte) in bytes.iter().copied().enumerate() {
        expr = Expr::and(
            expr,
            Expr::eq(
                safe_load_src_expr(
                    source_layout,
                    Expr::add(Expr::var(start_var), Expr::u32(idx as u32)),
                    source_byte_len.clone(),
                ),
                Expr::u32(u32::from(byte)),
            ),
        );
    }
    expr
}

fn push_gnu_builtin_hash_lookup(
    nodes: &mut Vec<Node>,
    prefix: &'static str,
    hash_var: &str,
    out_var: &str,
) {
    let slot_var = format!("{prefix}_lookup_slot");
    let value_var = format!("{prefix}_lookup_value");
    nodes.push(Node::let_bind(
        &slot_var,
        Expr::rem(
            Expr::mul(
                Expr::var(hash_var),
                Expr::u32(crate::parsing::c::parse::gnu_builtins::GPU_BUILTIN_HASH_TABLE_SEED),
            ),
            Expr::u32(crate::parsing::c::parse::gnu_builtins::GPU_BUILTIN_HASH_TABLE_SIZE as u32),
        ),
    ));
    nodes.push(Node::let_bind(
        &value_var,
        Expr::load("macro_values", Expr::var(&slot_var)),
    ));
    nodes.push(Node::let_bind(
        out_var,
        Expr::select(
            Expr::and(
                Expr::ne(Expr::var(&value_var), Expr::u32(0)),
                Expr::eq(Expr::var(&value_var), Expr::var(hash_var)),
            ),
            Expr::u32(1),
            Expr::u32(0),
        ),
    ));
}

pub(super) fn push_has_builtin_call_parser(
    nodes: &mut Vec<Node>,
    prefix: &'static str,
    start_var: &'static str,
    tok_end_var: &'static str,
    tok_len_var: &'static str,
    source_layout: SourceByteLayout,
    source_byte_len: Expr,
    scan_out_var: &'static str,
    found_var: &'static str,
    value_var: &'static str,
) {
    let is_builtin = format!("{prefix}_is_builtin");
    let is_constexpr_builtin = format!("{prefix}_is_constexpr_builtin");
    let pos = format!("{prefix}_pos");
    let ws_done = format!("{prefix}_ws_done");
    let ws_loop = format!("{prefix}_ws");
    let ws_b = format!("{prefix}_ws_b");
    let ws_is_ws = format!("{prefix}_ws_is_ws");
    let had_paren = format!("{prefix}_had_paren");
    let ws2_done = format!("{prefix}_ws2_done");
    let ws2_loop = format!("{prefix}_ws2");
    let ws2_b = format!("{prefix}_ws2_b");
    let ws2_is_ws = format!("{prefix}_ws2_is_ws");
    let arg_base = format!("{prefix}_arg_base");
    let arg_len = format!("{prefix}_arg_len");
    let hash = format!("{prefix}_hash");
    let arg_loop = format!("{prefix}_arg_id");
    let arg_pos = format!("{prefix}_arg_pos");
    let arg_b = format!("{prefix}_arg_b");
    let arg_alpha = format!("{prefix}_arg_alpha");
    let arg_digit = format!("{prefix}_arg_digit");
    let arg_under = format!("{prefix}_arg_under");
    let arg_cont = format!("{prefix}_arg_cont");
    let known = format!("{prefix}_known");
    let ws3_done = format!("{prefix}_ws3_done");
    let ws3_loop = format!("{prefix}_ws3");
    let ws3_b = format!("{prefix}_ws3_b");
    let ws3_is_ws = format!("{prefix}_ws3_is_ws");
    let had_close = format!("{prefix}_had_close");

    nodes.push(Node::let_bind(
        &is_builtin,
        Expr::select(
            source_at_equals(
                start_var,
                b"__has_builtin",
                source_layout,
                source_byte_len.clone(),
            ),
            Expr::u32(1),
            Expr::u32(0),
        ),
    ));
    nodes.push(Node::let_bind(
        &is_constexpr_builtin,
        Expr::select(
            source_at_equals(
                start_var,
                b"__has_constexpr_builtin",
                source_layout,
                source_byte_len.clone(),
            ),
            Expr::u32(1),
            Expr::u32(0),
        ),
    ));
    nodes.push(Node::if_then(
        Expr::and(
            Expr::eq(Expr::var(found_var), Expr::u32(0)),
            Expr::or(
                Expr::eq(Expr::var(&is_builtin), Expr::u32(1)),
                Expr::eq(Expr::var(&is_constexpr_builtin), Expr::u32(1)),
            ),
        ),
        {
            let mut call_nodes: Vec<Node> = Vec::new();
            call_nodes.push(Node::let_bind(
                &pos,
                Expr::add(
                    Expr::var(start_var),
                    Expr::select(
                        Expr::eq(Expr::var(&is_builtin), Expr::u32(1)),
                        Expr::u32(13),
                        Expr::u32(23),
                    ),
                ),
            ));
            call_nodes.push(Node::let_bind(&ws_done, Expr::u32(0)));
            call_nodes.push(Node::loop_for(
                &ws_loop,
                Expr::u32(0),
                Expr::var(tok_len_var),
                vec![Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var(&ws_done), Expr::u32(0)),
                        Expr::lt(Expr::var(&pos), Expr::var(tok_end_var)),
                    ),
                    vec![
                        Node::let_bind(
                            &ws_b,
                            safe_load_src_expr(
                                source_layout,
                                Expr::var(&pos),
                                source_byte_len.clone(),
                            ),
                        ),
                        Node::let_bind(
                            &ws_is_ws,
                            Expr::select(
                                Expr::or(
                                    Expr::or(
                                        Expr::eq(Expr::var(&ws_b), Expr::u32(b' ' as u32)),
                                        Expr::eq(Expr::var(&ws_b), Expr::u32(b'\t' as u32)),
                                    ),
                                    Expr::or(
                                        Expr::eq(Expr::var(&ws_b), Expr::u32(0x0B)),
                                        Expr::eq(Expr::var(&ws_b), Expr::u32(0x0C)),
                                    ),
                                ),
                                Expr::u32(1),
                                Expr::u32(0),
                            ),
                        ),
                        Node::if_then_else(
                            Expr::eq(Expr::var(&ws_is_ws), Expr::u32(1)),
                            vec![Node::assign(&pos, Expr::add(Expr::var(&pos), Expr::u32(1)))],
                            vec![Node::assign(&ws_done, Expr::u32(1))],
                        ),
                    ],
                )],
            ));
            call_nodes.push(Node::let_bind(
                &had_paren,
                Expr::select(
                    Expr::eq(
                        safe_load_src_expr(source_layout, Expr::var(&pos), source_byte_len.clone()),
                        Expr::u32(b'(' as u32),
                    ),
                    Expr::u32(1),
                    Expr::u32(0),
                ),
            ));
            call_nodes.push(Node::if_then(
                Expr::eq(Expr::var(&had_paren), Expr::u32(1)),
                {
                    let mut paren_nodes: Vec<Node> = Vec::new();
                    paren_nodes.push(Node::assign(&pos, Expr::add(Expr::var(&pos), Expr::u32(1))));
                    paren_nodes.push(Node::let_bind(&ws2_done, Expr::u32(0)));
                    paren_nodes.push(Node::loop_for(
                        &ws2_loop,
                        Expr::u32(0),
                        Expr::var(tok_len_var),
                        vec![Node::if_then(
                            Expr::and(
                                Expr::eq(Expr::var(&ws2_done), Expr::u32(0)),
                                Expr::lt(Expr::var(&pos), Expr::var(tok_end_var)),
                            ),
                            vec![
                                Node::let_bind(
                                    &ws2_b,
                                    safe_load_src_expr(
                                        source_layout,
                                        Expr::var(&pos),
                                        source_byte_len.clone(),
                                    ),
                                ),
                                Node::let_bind(
                                    &ws2_is_ws,
                                    Expr::select(
                                        Expr::or(
                                            Expr::or(
                                                Expr::eq(Expr::var(&ws2_b), Expr::u32(b' ' as u32)),
                                                Expr::eq(
                                                    Expr::var(&ws2_b),
                                                    Expr::u32(b'\t' as u32),
                                                ),
                                            ),
                                            Expr::or(
                                                Expr::eq(Expr::var(&ws2_b), Expr::u32(0x0B)),
                                                Expr::eq(Expr::var(&ws2_b), Expr::u32(0x0C)),
                                            ),
                                        ),
                                        Expr::u32(1),
                                        Expr::u32(0),
                                    ),
                                ),
                                Node::if_then_else(
                                    Expr::eq(Expr::var(&ws2_is_ws), Expr::u32(1)),
                                    vec![Node::assign(
                                        &pos,
                                        Expr::add(Expr::var(&pos), Expr::u32(1)),
                                    )],
                                    vec![Node::assign(&ws2_done, Expr::u32(1))],
                                ),
                            ],
                        )],
                    ));
                    paren_nodes.push(Node::let_bind(
                        &arg_base,
                        Expr::add(Expr::var(&pos), Expr::u32(0)),
                    ));
                    paren_nodes.push(Node::let_bind(&arg_len, Expr::u32(0)));
                    paren_nodes.push(Node::let_bind(&hash, fnv1a32_initial_expr()));
                    paren_nodes.push(Node::loop_for(
                        &arg_loop,
                        Expr::u32(0),
                        Expr::select(
                            Expr::lt(Expr::var(&arg_base), Expr::var(tok_end_var)),
                            Expr::sub(Expr::var(tok_end_var), Expr::var(&arg_base)),
                            Expr::u32(0),
                        ),
                        vec![Node::if_then(
                            Expr::eq(Expr::var(&arg_len), Expr::var(&arg_loop)),
                            vec![
                                Node::let_bind(
                                    &arg_pos,
                                    Expr::add(Expr::var(&arg_base), Expr::var(&arg_loop)),
                                ),
                                Node::if_then(
                                    Expr::lt(Expr::var(&arg_pos), Expr::var(tok_end_var)),
                                    vec![
                                        Node::let_bind(
                                            &arg_b,
                                            safe_load_src_expr(
                                                source_layout,
                                                Expr::var(&arg_pos),
                                                source_byte_len.clone(),
                                            ),
                                        ),
                                        Node::let_bind(
                                            &arg_alpha,
                                            Expr::select(
                                                Expr::or(
                                                    Expr::and(
                                                        Expr::ge(
                                                            Expr::var(&arg_b),
                                                            Expr::u32(b'a' as u32),
                                                        ),
                                                        Expr::le(
                                                            Expr::var(&arg_b),
                                                            Expr::u32(b'z' as u32),
                                                        ),
                                                    ),
                                                    Expr::and(
                                                        Expr::ge(
                                                            Expr::var(&arg_b),
                                                            Expr::u32(b'A' as u32),
                                                        ),
                                                        Expr::le(
                                                            Expr::var(&arg_b),
                                                            Expr::u32(b'Z' as u32),
                                                        ),
                                                    ),
                                                ),
                                                Expr::u32(1),
                                                Expr::u32(0),
                                            ),
                                        ),
                                        Node::let_bind(
                                            &arg_digit,
                                            Expr::select(
                                                Expr::and(
                                                    Expr::ge(
                                                        Expr::var(&arg_b),
                                                        Expr::u32(b'0' as u32),
                                                    ),
                                                    Expr::le(
                                                        Expr::var(&arg_b),
                                                        Expr::u32(b'9' as u32),
                                                    ),
                                                ),
                                                Expr::u32(1),
                                                Expr::u32(0),
                                            ),
                                        ),
                                        Node::let_bind(
                                            &arg_under,
                                            Expr::select(
                                                Expr::eq(Expr::var(&arg_b), Expr::u32(b'_' as u32)),
                                                Expr::u32(1),
                                                Expr::u32(0),
                                            ),
                                        ),
                                        Node::let_bind(
                                            &arg_cont,
                                            Expr::select(
                                                Expr::or(
                                                    Expr::or(
                                                        Expr::eq(
                                                            Expr::var(&arg_alpha),
                                                            Expr::u32(1),
                                                        ),
                                                        Expr::eq(
                                                            Expr::var(&arg_digit),
                                                            Expr::u32(1),
                                                        ),
                                                    ),
                                                    Expr::eq(Expr::var(&arg_under), Expr::u32(1)),
                                                ),
                                                Expr::u32(1),
                                                Expr::u32(0),
                                            ),
                                        ),
                                        Node::if_then(
                                            Expr::eq(Expr::var(&arg_cont), Expr::u32(1)),
                                            vec![
                                                Node::assign(
                                                    &hash,
                                                    fnv1a32_update_byte_expr(
                                                        Expr::var(&hash),
                                                        Expr::var(&arg_b),
                                                    ),
                                                ),
                                                Node::assign(
                                                    &arg_len,
                                                    Expr::add(Expr::var(&arg_len), Expr::u32(1)),
                                                ),
                                            ],
                                        ),
                                    ],
                                ),
                            ],
                        )],
                    ));
                    paren_nodes.push(Node::assign(
                        &pos,
                        Expr::add(Expr::var(&arg_base), Expr::var(&arg_len)),
                    ));
                    push_gnu_builtin_hash_lookup(&mut paren_nodes, prefix, &hash, &known);
                    paren_nodes.push(Node::let_bind(&ws3_done, Expr::u32(0)));
                    paren_nodes.push(Node::loop_for(
                        &ws3_loop,
                        Expr::u32(0),
                        Expr::var(tok_len_var),
                        vec![Node::if_then(
                            Expr::and(
                                Expr::eq(Expr::var(&ws3_done), Expr::u32(0)),
                                Expr::lt(Expr::var(&pos), Expr::var(tok_end_var)),
                            ),
                            vec![
                                Node::let_bind(
                                    &ws3_b,
                                    safe_load_src_expr(
                                        source_layout,
                                        Expr::var(&pos),
                                        source_byte_len.clone(),
                                    ),
                                ),
                                Node::let_bind(
                                    &ws3_is_ws,
                                    Expr::select(
                                        Expr::or(
                                            Expr::or(
                                                Expr::eq(Expr::var(&ws3_b), Expr::u32(b' ' as u32)),
                                                Expr::eq(
                                                    Expr::var(&ws3_b),
                                                    Expr::u32(b'\t' as u32),
                                                ),
                                            ),
                                            Expr::or(
                                                Expr::eq(Expr::var(&ws3_b), Expr::u32(0x0B)),
                                                Expr::eq(Expr::var(&ws3_b), Expr::u32(0x0C)),
                                            ),
                                        ),
                                        Expr::u32(1),
                                        Expr::u32(0),
                                    ),
                                ),
                                Node::if_then_else(
                                    Expr::eq(Expr::var(&ws3_is_ws), Expr::u32(1)),
                                    vec![Node::assign(
                                        &pos,
                                        Expr::add(Expr::var(&pos), Expr::u32(1)),
                                    )],
                                    vec![Node::assign(&ws3_done, Expr::u32(1))],
                                ),
                            ],
                        )],
                    ));
                    paren_nodes.push(Node::let_bind(
                        &had_close,
                        Expr::select(
                            Expr::eq(
                                safe_load_src_expr(
                                    source_layout,
                                    Expr::var(&pos),
                                    source_byte_len.clone(),
                                ),
                                Expr::u32(b')' as u32),
                            ),
                            Expr::u32(1),
                            Expr::u32(0),
                        ),
                    ));
                    paren_nodes.push(Node::if_then(
                        Expr::eq(Expr::var(&had_close), Expr::u32(1)),
                        vec![
                            Node::assign(scan_out_var, Expr::add(Expr::var(&pos), Expr::u32(1))),
                            Node::assign(found_var, Expr::u32(1)),
                            Node::assign(value_var, Expr::var(&known)),
                        ],
                    ));
                    paren_nodes
                },
            ));
            call_nodes
        },
    ));
}
