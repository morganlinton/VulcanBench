use super::*;

pub(crate) fn semantic_resolution_nodes(
    vast_nodes: &str,
    num_nodes: &Expr,
    node_idx: Expr,
) -> Vec<Node> {
    let mut nodes = vec![
        Node::let_bind("resolved_goto_target_idx", Expr::u32(u32::MAX)),
        Node::let_bind("switch_selector_idx", Expr::u32(u32::MAX)),
        Node::let_bind("case_value_idx", Expr::u32(u32::MAX)),
        Node::let_bind("enclosing_switch_idx", Expr::u32(u32::MAX)),
        Node::let_bind("enclosing_switch_distance", Expr::u32(u32::MAX)),
        Node::let_bind("goto_target_hash", Expr::u32(0)),
    ];

    nodes.extend([
        Node::let_bind("current_root_idx", node_idx.clone()),
        Node::let_bind("current_root_parent_idx", Expr::u32(u32::MAX)),
    ]);

    let mut goto_setup = assign_root_nodes(
        vast_nodes,
        num_nodes,
        node_idx.clone(),
        "current_root_idx",
        "current_root_parent_idx",
        "current_root_step",
    );
    goto_setup.push(Node::if_then(
        valid_node_idx(Expr::var("next_sibling_idx"), num_nodes),
        vec![Node::assign(
            "goto_target_hash",
            vast_field(vast_nodes, Expr::var("next_sibling_idx"), IDX_SYMBOL_HASH),
        )],
    ));
    nodes.push(Node::if_then(
        expr_is_kind(Expr::var("kind"), C_AST_KIND_GOTO_STMT),
        goto_setup,
    ));

    let mut label_scan_body = vec![
        Node::let_bind("label_scan_kind", Expr::u32(0)),
        Node::let_bind("label_scan_hash", Expr::u32(0)),
    ];
    label_scan_body.push(Node::if_then(
        valid_node_idx(Expr::var("label_scan_idx"), num_nodes),
        vec![
            Node::assign(
                "label_scan_kind",
                vast_field(vast_nodes, Expr::var("label_scan_idx"), IDX_KIND),
            ),
            Node::assign(
                "label_scan_hash",
                vast_field(vast_nodes, Expr::var("label_scan_idx"), IDX_SYMBOL_HASH),
            ),
        ],
    ));
    let mut label_match_body = resolve_root_nodes(
        vast_nodes,
        num_nodes,
        Expr::var("label_scan_idx"),
        "label_scan_root_idx",
        "label_scan_parent_idx",
        "label_scan_root_step",
    );
    label_match_body.push(Node::if_then(
        Expr::and(
            Expr::eq(Expr::var("resolved_goto_target_idx"), Expr::u32(u32::MAX)),
            Expr::and(
                Expr::eq(Expr::var("label_scan_hash"), Expr::var("goto_target_hash")),
                Expr::eq(
                    Expr::var("label_scan_root_idx"),
                    Expr::var("current_root_idx"),
                ),
            ),
        ),
        vec![Node::assign(
            "resolved_goto_target_idx",
            Expr::var("label_scan_idx"),
        )],
    ));
    label_scan_body.push(Node::if_then(
        Expr::and(
            Expr::eq(
                Expr::var("label_scan_kind"),
                Expr::u32(C_AST_KIND_LABEL_STMT),
            ),
            Expr::and(
                Expr::ne(Expr::var("goto_target_hash"), Expr::u32(0)),
                Expr::eq(Expr::var("resolved_goto_target_idx"), Expr::u32(u32::MAX)),
            ),
        ),
        label_match_body,
    ));
    nodes.push(Node::if_then(
        expr_is_kind(Expr::var("kind"), C_AST_KIND_GOTO_STMT),
        vec![Node::loop_for(
            "label_scan_idx",
            Expr::u32(0),
            num_nodes.clone(),
            label_scan_body,
        )],
    ));

    nodes.push(Node::if_then(
        Expr::and(
            expr_is_kind(Expr::var("kind"), C_AST_KIND_SWITCH_STMT),
            valid_node_idx(Expr::var("next_sibling_idx"), num_nodes),
        ),
        vec![
            Node::let_bind(
                "switch_selector_candidate",
                vast_field(vast_nodes, Expr::var("next_sibling_idx"), IDX_FIRST_CHILD),
            ),
            Node::let_bind("switch_selector_parent", Expr::u32(u32::MAX)),
            Node::if_then(
                valid_node_idx(Expr::var("switch_selector_candidate"), num_nodes),
                vec![Node::assign(
                    "switch_selector_parent",
                    vast_field(
                        vast_nodes,
                        Expr::var("switch_selector_candidate"),
                        IDX_PARENT,
                    ),
                )],
            ),
            Node::if_then(
                Expr::and(
                    valid_node_idx(Expr::var("switch_selector_candidate"), num_nodes),
                    Expr::eq(
                        Expr::var("switch_selector_parent"),
                        Expr::var("next_sibling_idx"),
                    ),
                ),
                vec![Node::assign(
                    "switch_selector_idx",
                    Expr::var("switch_selector_candidate"),
                )],
            ),
        ],
    ));

    nodes.push(Node::if_then(
        Expr::and(
            expr_is_kind(Expr::var("kind"), C_AST_KIND_CASE_STMT),
            Expr::and(
                valid_node_idx(Expr::var("next_sibling_idx"), num_nodes),
                Expr::eq(
                    vast_field(vast_nodes, Expr::var("next_sibling_idx"), IDX_PARENT),
                    Expr::var("parent_idx"),
                ),
            ),
        ),
        vec![Node::assign(
            "case_value_idx",
            Expr::var("next_sibling_idx"),
        )],
    ));

    nodes.push(Node::if_then(
        Expr::or(
            expr_is_kind(Expr::var("kind"), C_AST_KIND_CASE_STMT),
            expr_is_kind(Expr::var("kind"), C_AST_KIND_DEFAULT_STMT),
        ),
        vec![
            Node::let_bind("switch_ancestor_idx", Expr::var("parent_idx")),
            Node::loop_for(
                "switch_ancestor_step",
                Expr::u32(0),
                common_parent_walk_bound(num_nodes),
                vec![Node::if_then(
                    Expr::and(
                        Expr::eq(Expr::var("enclosing_switch_idx"), Expr::u32(u32::MAX)),
                        valid_node_idx(Expr::var("switch_ancestor_idx"), num_nodes),
                    ),
                    vec![
                        Node::let_bind(
                            "switch_ancestor_kind",
                            vast_field(vast_nodes, Expr::var("switch_ancestor_idx"), IDX_KIND),
                        ),
                        Node::if_then(
                            Expr::and(
                                Expr::eq(Expr::var("enclosing_switch_idx"), Expr::u32(u32::MAX)),
                                Expr::eq(
                                    Expr::var("switch_ancestor_kind"),
                                    Expr::u32(C_AST_KIND_SWITCH_STMT),
                                ),
                            ),
                            vec![
                                Node::assign(
                                    "enclosing_switch_idx",
                                    Expr::var("switch_ancestor_idx"),
                                ),
                                Node::assign(
                                    "enclosing_switch_distance",
                                    Expr::var("switch_ancestor_step"),
                                ),
                            ],
                        ),
                        Node::assign(
                            "switch_ancestor_idx",
                            vast_field(vast_nodes, Expr::var("switch_ancestor_idx"), IDX_PARENT),
                        ),
                    ],
                )],
            ),
        ],
    ));

    let switch_scan_limit = common_parent_walk_bound(num_nodes);
    let switch_scan_body = vec![
        Node::let_bind(
            "switch_scan_idx",
            Expr::add(
                Expr::var("switch_scan_start_idx"),
                Expr::var("switch_scan_offset"),
            ),
        ),
        Node::let_bind("switch_scan_kind", Expr::u32(0)),
        Node::let_bind("switch_scan_condition_group_idx", Expr::u32(u32::MAX)),
        Node::let_bind("switch_scan_body_idx", Expr::u32(u32::MAX)),
        Node::if_then(
            valid_node_idx(Expr::var("switch_scan_idx"), num_nodes),
            vec![
                Node::assign(
                    "switch_scan_kind",
                    vast_field(vast_nodes, Expr::var("switch_scan_idx"), IDX_KIND),
                ),
                Node::assign(
                    "switch_scan_condition_group_idx",
                    vast_field(vast_nodes, Expr::var("switch_scan_idx"), IDX_NEXT_SIBLING),
                ),
            ],
        ),
        Node::if_then(
            Expr::and(
                Expr::eq(
                    Expr::var("switch_scan_kind"),
                    Expr::u32(C_AST_KIND_SWITCH_STMT),
                ),
                valid_node_idx(Expr::var("switch_scan_condition_group_idx"), num_nodes),
            ),
            vec![Node::assign(
                "switch_scan_body_idx",
                vast_field(
                    vast_nodes,
                    Expr::var("switch_scan_condition_group_idx"),
                    IDX_NEXT_SIBLING,
                ),
            )],
        ),
        Node::if_then(
            Expr::and(
                valid_node_idx(Expr::var("switch_scan_idx"), num_nodes),
                valid_node_idx(Expr::var("switch_scan_body_idx"), num_nodes),
            ),
            vec![
                Node::let_bind("switch_body_ancestor_idx", Expr::var("parent_idx")),
                Node::let_bind("switch_body_found", Expr::bool(false)),
                Node::let_bind("switch_body_distance", Expr::u32(u32::MAX)),
                Node::loop_for(
                    "switch_body_step",
                    Expr::u32(0),
                    common_parent_walk_bound(num_nodes),
                    vec![
                        Node::if_then(
                            Expr::eq(
                                Expr::var("switch_body_ancestor_idx"),
                                Expr::var("switch_scan_body_idx"),
                            ),
                            vec![Node::if_then(
                                Expr::not(Expr::var("switch_body_found")),
                                vec![
                                    Node::assign("switch_body_found", Expr::bool(true)),
                                    Node::assign(
                                        "switch_body_distance",
                                        Expr::var("switch_body_step"),
                                    ),
                                ],
                            )],
                        ),
                        Node::if_then(
                            valid_node_idx(Expr::var("switch_body_ancestor_idx"), num_nodes),
                            vec![Node::assign(
                                "switch_body_ancestor_idx",
                                vast_field(
                                    vast_nodes,
                                    Expr::var("switch_body_ancestor_idx"),
                                    IDX_PARENT,
                                ),
                            )],
                        ),
                    ],
                ),
                Node::if_then(
                    Expr::and(
                        Expr::var("switch_body_found"),
                        Expr::lt(
                            Expr::var("switch_body_distance"),
                            Expr::var("enclosing_switch_distance"),
                        ),
                    ),
                    vec![
                        Node::assign("enclosing_switch_idx", Expr::var("switch_scan_idx")),
                        Node::assign(
                            "enclosing_switch_distance",
                            Expr::var("switch_body_distance"),
                        ),
                    ],
                ),
            ],
        ),
    ];
    nodes.push(Node::if_then(
        Expr::and(
            Expr::or(
                expr_is_kind(Expr::var("kind"), C_AST_KIND_CASE_STMT),
                expr_is_kind(Expr::var("kind"), C_AST_KIND_DEFAULT_STMT),
            ),
            Expr::eq(Expr::var("enclosing_switch_idx"), Expr::u32(u32::MAX)),
        ),
        vec![
            Node::let_bind(
                "switch_scan_start_idx",
                Expr::select(
                    Expr::gt(node_idx.clone(), switch_scan_limit.clone()),
                    Expr::sub(node_idx.clone(), switch_scan_limit.clone()),
                    Expr::u32(0),
                ),
            ),
            Node::let_bind(
                "switch_scan_stop_idx",
                Expr::select(
                    Expr::lt(
                        Expr::add(node_idx.clone(), switch_scan_limit.clone()),
                        num_nodes.clone(),
                    ),
                    Expr::add(node_idx.clone(), switch_scan_limit.clone()),
                    num_nodes.clone(),
                ),
            ),
            Node::loop_for(
                "switch_scan_offset",
                Expr::u32(0),
                Expr::sub(
                    Expr::var("switch_scan_stop_idx"),
                    Expr::var("switch_scan_start_idx"),
                ),
                switch_scan_body,
            ),
        ],
    ));

    nodes.extend(vec![
        Node::let_bind("semantic_edge3_has", Expr::bool(false)),
        Node::let_bind("semantic_edge3_kind", Expr::u32(C_AST_PG_EDGE_NONE)),
        Node::let_bind("semantic_edge3_src", Expr::u32(u32::MAX)),
        Node::let_bind("semantic_edge3_dst", Expr::u32(u32::MAX)),
        Node::let_bind("semantic_edge4_has", Expr::bool(false)),
        Node::let_bind("semantic_edge4_kind", Expr::u32(C_AST_PG_EDGE_NONE)),
        Node::let_bind("semantic_edge4_src", Expr::u32(u32::MAX)),
        Node::let_bind("semantic_edge4_dst", Expr::u32(u32::MAX)),
    ]);

    nodes.push(Node::if_then(
        Expr::and(
            expr_is_kind(Expr::var("kind"), C_AST_KIND_GOTO_STMT),
            valid_node_idx(Expr::var("resolved_goto_target_idx"), num_nodes),
        ),
        vec![
            Node::assign("semantic_edge3_has", Expr::bool(true)),
            Node::assign("semantic_edge3_kind", Expr::u32(C_AST_PG_EDGE_GOTO_TARGET)),
            Node::assign("semantic_edge3_src", node_idx.clone()),
            Node::assign("semantic_edge3_dst", Expr::var("resolved_goto_target_idx")),
        ],
    ));
    nodes.push(Node::if_then(
        Expr::and(
            expr_is_kind(Expr::var("kind"), C_AST_KIND_SWITCH_STMT),
            valid_node_idx(Expr::var("switch_selector_idx"), num_nodes),
        ),
        vec![
            Node::assign("semantic_edge3_has", Expr::bool(true)),
            Node::assign(
                "semantic_edge3_kind",
                Expr::u32(C_AST_PG_EDGE_SWITCH_SELECTOR),
            ),
            Node::assign("semantic_edge3_src", node_idx.clone()),
            Node::assign("semantic_edge3_dst", Expr::var("switch_selector_idx")),
        ],
    ));
    nodes.push(Node::if_then(
        Expr::and(
            expr_is_kind(Expr::var("kind"), C_AST_KIND_CASE_STMT),
            valid_node_idx(Expr::var("case_value_idx"), num_nodes),
        ),
        vec![
            Node::assign("semantic_edge3_has", Expr::bool(true)),
            Node::assign("semantic_edge3_kind", Expr::u32(C_AST_PG_EDGE_CASE_VALUE)),
            Node::assign("semantic_edge3_src", node_idx.clone()),
            Node::assign("semantic_edge3_dst", Expr::var("case_value_idx")),
        ],
    ));
    nodes.push(Node::if_then(
        Expr::and(
            expr_is_kind(Expr::var("kind"), C_AST_KIND_DEFAULT_STMT),
            valid_node_idx(Expr::var("enclosing_switch_idx"), num_nodes),
        ),
        vec![
            Node::assign("semantic_edge3_has", Expr::bool(true)),
            Node::assign(
                "semantic_edge3_kind",
                Expr::u32(C_AST_PG_EDGE_SWITCH_DEFAULT),
            ),
            Node::assign("semantic_edge3_src", Expr::var("enclosing_switch_idx")),
            Node::assign("semantic_edge3_dst", node_idx.clone()),
        ],
    ));
    nodes.push(Node::if_then(
        Expr::and(
            expr_is_kind(Expr::var("kind"), C_AST_KIND_CASE_STMT),
            valid_node_idx(Expr::var("enclosing_switch_idx"), num_nodes),
        ),
        vec![
            Node::assign("semantic_edge4_has", Expr::bool(true)),
            Node::assign("semantic_edge4_kind", Expr::u32(C_AST_PG_EDGE_SWITCH_CASE)),
            Node::assign("semantic_edge4_src", Expr::var("enclosing_switch_idx")),
            Node::assign("semantic_edge4_dst", node_idx),
        ],
    ));

    nodes
}
