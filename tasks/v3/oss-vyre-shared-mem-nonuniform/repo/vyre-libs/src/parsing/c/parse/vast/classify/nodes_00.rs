use super::*;

pub(super) fn extend(
    out: &mut Vec<Node>,
    vast_nodes: &str,
    _out_typed_vast_nodes: &str,
    num_nodes: Expr,
    t: Expr,
    base: Expr,
) {
    out.extend(vec![
        Node::let_bind("raw_kind", Expr::load(vast_nodes, base.clone())),
        Node::let_bind(
            "current_symbol_hash",
            Expr::load(
                vast_nodes,
                Expr::add(base.clone(), Expr::u32(VAST_TYPEDEF_SYMBOL_FIELD)),
            ),
        ),
        Node::let_bind(
            "cur_parent",
            Expr::load(vast_nodes, Expr::add(base.clone(), Expr::u32(1))),
        ),
        Node::let_bind(
            "prev_sibling_idx",
            Expr::load(
                vast_nodes,
                Expr::add(base.clone(), Expr::u32(VAST_PREVIOUS_SIBLING_FIELD)),
            ),
        ),
        Node::let_bind(
            "prev_sibling_valid_direct",
            Expr::lt(Expr::var("prev_sibling_idx"), num_nodes.clone()),
        ),
        Node::let_bind(
            "prev_sibling_base_direct",
            Expr::mul(
                Expr::select(
                    Expr::var("prev_sibling_valid_direct"),
                    Expr::var("prev_sibling_idx"),
                    t.clone(),
                ),
                Expr::u32(VAST_NODE_STRIDE_U32),
            ),
        ),
        Node::let_bind(
            "prev_sibling_kind",
            Expr::select(
                Expr::var("prev_sibling_valid_direct"),
                Expr::load(vast_nodes, Expr::var("prev_sibling_base_direct")),
                Expr::u32(SENTINEL),
            ),
        ),
        Node::let_bind(
            "prev_sibling_symbol_hash",
            Expr::select(
                Expr::var("prev_sibling_valid_direct"),
                Expr::load(
                    vast_nodes,
                    Expr::add(
                        Expr::var("prev_sibling_base_direct"),
                        Expr::u32(VAST_TYPEDEF_SYMBOL_FIELD),
                    ),
                ),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(
            "prev_prev_sibling_idx",
            Expr::select(
                Expr::var("prev_sibling_valid_direct"),
                Expr::load(
                    vast_nodes,
                    Expr::add(
                        Expr::var("prev_sibling_base_direct"),
                        Expr::u32(VAST_PREVIOUS_SIBLING_FIELD),
                    ),
                ),
                Expr::u32(SENTINEL),
            ),
        ),
        Node::let_bind(
            "prev_prev_sibling_valid",
            Expr::lt(Expr::var("prev_prev_sibling_idx"), num_nodes.clone()),
        ),
        Node::let_bind(
            "prev_prev_sibling_kind",
            Expr::select(
                Expr::var("prev_prev_sibling_valid"),
                Expr::load(
                    vast_nodes,
                    Expr::mul(
                        Expr::var("prev_prev_sibling_idx"),
                        Expr::u32(VAST_NODE_STRIDE_U32),
                    ),
                ),
                Expr::u32(SENTINEL),
            ),
        ),
        Node::let_bind(
            "first_child_idx",
            Expr::load(vast_nodes, Expr::add(base.clone(), Expr::u32(2))),
        ),
        Node::let_bind(
            "first_child_valid",
            Expr::lt(Expr::var("first_child_idx"), num_nodes.clone()),
        ),
        Node::let_bind(
            "safe_first_child_idx",
            Expr::select(
                Expr::var("first_child_valid"),
                Expr::var("first_child_idx"),
                t.clone(),
            ),
        ),
        Node::let_bind(
            "first_child_base",
            Expr::mul(
                Expr::var("safe_first_child_idx"),
                Expr::u32(VAST_NODE_STRIDE_U32),
            ),
        ),
        Node::let_bind(
            "first_child_kind",
            Expr::select(
                Expr::var("first_child_valid"),
                Expr::load(vast_nodes, Expr::var("first_child_base")),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(
            "first_child_typedef_flags",
            Expr::select(
                Expr::var("first_child_valid"),
                Expr::load(
                    vast_nodes,
                    Expr::add(
                        Expr::var("first_child_base"),
                        Expr::u32(VAST_TYPEDEF_FLAGS_FIELD),
                    ),
                ),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(
            "first_child_symbol_hash",
            Expr::select(
                Expr::var("first_child_valid"),
                Expr::load(
                    vast_nodes,
                    Expr::add(
                        Expr::var("first_child_base"),
                        Expr::u32(VAST_TYPEDEF_SYMBOL_FIELD),
                    ),
                ),
                Expr::u32(0),
            ),
        ),
        Node::let_bind("raw_next_idx", Expr::add(t.clone(), Expr::u32(1))),
        Node::let_bind(
            "raw_next_valid",
            Expr::lt(Expr::var("raw_next_idx"), num_nodes.clone()),
        ),
        Node::let_bind(
            "raw_next_base",
            Expr::mul(
                Expr::select(
                    Expr::var("raw_next_valid"),
                    Expr::var("raw_next_idx"),
                    t.clone(),
                ),
                Expr::u32(VAST_NODE_STRIDE_U32),
            ),
        ),
        Node::let_bind(
            "raw_next_kind",
            Expr::select(
                Expr::var("raw_next_valid"),
                Expr::load(vast_nodes, Expr::var("raw_next_base")),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(
            "raw_next_typedef_flags",
            Expr::select(
                Expr::var("raw_next_valid"),
                Expr::load(
                    vast_nodes,
                    Expr::add(
                        Expr::var("raw_next_base"),
                        Expr::u32(VAST_TYPEDEF_FLAGS_FIELD),
                    ),
                ),
                Expr::u32(0),
            ),
        ),
        Node::let_bind("raw_after_next_idx", Expr::add(t.clone(), Expr::u32(2))),
        Node::let_bind(
            "raw_after_next_valid",
            Expr::lt(Expr::var("raw_after_next_idx"), num_nodes.clone()),
        ),
        Node::let_bind(
            "raw_after_next_kind",
            Expr::select(
                Expr::var("raw_after_next_valid"),
                Expr::load(
                    vast_nodes,
                    Expr::mul(
                        Expr::var("raw_after_next_idx"),
                        Expr::u32(VAST_NODE_STRIDE_U32),
                    ),
                ),
                Expr::u32(0),
            ),
        ),
        Node::let_bind("raw_after_after_idx", Expr::add(t.clone(), Expr::u32(3))),
        Node::let_bind(
            "raw_after_after_valid",
            Expr::lt(Expr::var("raw_after_after_idx"), num_nodes.clone()),
        ),
        Node::let_bind(
            "raw_after_after_kind",
            Expr::select(
                Expr::var("raw_after_after_valid"),
                Expr::load(
                    vast_nodes,
                    Expr::mul(
                        Expr::var("raw_after_after_idx"),
                        Expr::u32(VAST_NODE_STRIDE_U32),
                    ),
                ),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(
            "next_idx",
            Expr::load(vast_nodes, Expr::add(base.clone(), Expr::u32(3))),
        ),
        Node::let_bind(
            "next_valid",
            Expr::lt(Expr::var("next_idx"), num_nodes.clone()),
        ),
        Node::let_bind(
            "safe_next_idx",
            Expr::select(Expr::var("next_valid"), Expr::var("next_idx"), t.clone()),
        ),
        Node::let_bind(
            "next_base",
            Expr::mul(Expr::var("safe_next_idx"), Expr::u32(VAST_NODE_STRIDE_U32)),
        ),
        Node::let_bind(
            "next_kind",
            Expr::select(
                Expr::var("next_valid"),
                Expr::load(vast_nodes, Expr::var("next_base")),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(
            "after_param_idx",
            Expr::select(
                Expr::var("next_valid"),
                Expr::load(vast_nodes, Expr::add(Expr::var("next_base"), Expr::u32(3))),
                Expr::u32(SENTINEL),
            ),
        ),
        Node::let_bind(
            "after_param_valid",
            Expr::lt(Expr::var("after_param_idx"), num_nodes.clone()),
        ),
        Node::let_bind(
            "after_param_kind",
            Expr::select(
                Expr::var("after_param_valid"),
                Expr::load(
                    vast_nodes,
                    Expr::mul(
                        Expr::var("after_param_idx"),
                        Expr::u32(VAST_NODE_STRIDE_U32),
                    ),
                ),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(
            "prev_sibling_valid",
            Expr::lt(Expr::var("prev_sibling_idx"), num_nodes.clone()),
        ),
        Node::let_bind(
            "safe_prev_sibling_idx",
            Expr::select(
                Expr::var("prev_sibling_valid"),
                Expr::var("prev_sibling_idx"),
                t.clone(),
            ),
        ),
        Node::let_bind(
            "prev_sibling_base",
            Expr::mul(
                Expr::var("safe_prev_sibling_idx"),
                Expr::u32(VAST_NODE_STRIDE_U32),
            ),
        ),
        Node::let_bind(
            "prev_sibling_first_child_idx",
            Expr::load(
                vast_nodes,
                Expr::add(Expr::var("prev_sibling_base"), Expr::u32(2)),
            ),
        ),
        Node::let_bind(
            "prev_sibling_first_child_valid",
            Expr::lt(Expr::var("prev_sibling_first_child_idx"), num_nodes.clone()),
        ),
        Node::let_bind(
            "safe_prev_sibling_first_child_idx",
            Expr::select(
                Expr::var("prev_sibling_first_child_valid"),
                Expr::var("prev_sibling_first_child_idx"),
                t.clone(),
            ),
        ),
        Node::let_bind(
            "prev_sibling_first_child_base",
            Expr::mul(
                Expr::var("safe_prev_sibling_first_child_idx"),
                Expr::u32(VAST_NODE_STRIDE_U32),
            ),
        ),
        Node::let_bind(
            "prev_sibling_first_child_kind",
            Expr::select(
                Expr::var("prev_sibling_first_child_valid"),
                Expr::load(vast_nodes, Expr::var("prev_sibling_first_child_base")),
                Expr::u32(0),
            ),
        ),
        Node::let_bind(
            "prev_sibling_typedef_flags",
            Expr::select(
                Expr::var("prev_sibling_valid"),
                Expr::load(
                    vast_nodes,
                    Expr::add(
                        Expr::var("prev_sibling_base"),
                        Expr::u32(VAST_TYPEDEF_FLAGS_FIELD),
                    ),
                ),
                Expr::u32(0),
            ),
        ),
    ]);
}
