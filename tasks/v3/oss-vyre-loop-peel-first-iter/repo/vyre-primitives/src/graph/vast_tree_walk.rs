//! VAST first-child / next-sibling tree traversal primitives.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use vyre_foundation::vast::{NODE_STRIDE_U32, SENTINEL};

/// Primitive op id for preorder VAST tree traversal.
pub const PREORDER_OP_ID: &str = "vyre-primitives::graph::vast_walk_preorder";
/// Primitive op id for postorder VAST tree traversal.
pub const POSTORDER_OP_ID: &str = "vyre-primitives::graph::vast_walk_postorder";

/// Traversal order for VAST first-child / next-sibling tree walks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VastWalkOrder {
    /// Emit each node before its descendants.
    Preorder,
    /// Emit each node after its descendants.
    Postorder,
}

impl VastWalkOrder {
    fn op_id(self) -> &'static str {
        match self {
            Self::Preorder => PREORDER_OP_ID,
            Self::Postorder => POSTORDER_OP_ID,
        }
    }
}

/// Primitive-owned VAST traversal programs for passes that need both orders.
#[derive(Debug, Clone)]
pub struct VastTreeWalkProgramPlan {
    /// Top-down walk used by declaration discovery and source-order diagnostics.
    pub preorder: Program,
    /// Bottom-up walk used by expression typing and lowering passes.
    pub postorder: Program,
}

/// Build checked preorder and postorder VAST traversal programs together.
///
/// # Errors
///
/// Returns the same launch-shape diagnostics as the single-order builders.
pub fn try_ast_walk_plan(
    nodes: &str,
    preorder_out: &str,
    postorder_out: &str,
    node_count: u32,
    out_cap: u32,
) -> Result<VastTreeWalkProgramPlan, String> {
    Ok(VastTreeWalkProgramPlan {
        preorder: try_ast_walk_preorder(nodes, preorder_out, node_count, out_cap)?,
        postorder: try_ast_walk_postorder(nodes, postorder_out, node_count, out_cap)?,
    })
}

/// Emit preorder node indices for a VAST first-child / next-sibling tree.
#[must_use]
pub fn ast_walk_preorder(nodes: &str, out: &str, node_count: u32, out_cap: u32) -> Program {
    // Fail fast: an invalid launch shape must NOT silently degrade to an inert
    // empty kernel that walks nothing — that is a silent recall loss with no
    // signal. Callers needing structured handling use `try_ast_walk_preorder`.
    try_ast_walk_preorder(nodes, out, node_count, out_cap)
        .unwrap_or_else(|error| panic!("{error}"))
}

/// Emit preorder node indices for a VAST first-child / next-sibling tree with
/// checked launch-shape validation.
pub fn try_ast_walk_preorder(
    nodes: &str,
    out: &str,
    node_count: u32,
    out_cap: u32,
) -> Result<Program, String> {
    try_ast_walk_order(VastWalkOrder::Preorder, nodes, out, node_count, out_cap)
}

/// Emit postorder node indices for a VAST first-child / next-sibling tree.
#[must_use]
pub fn ast_walk_postorder(nodes: &str, out: &str, node_count: u32, out_cap: u32) -> Program {
    // Fail fast on an invalid launch shape rather than silently degrading to an
    // inert empty kernel (silent recall loss). Use `try_ast_walk_postorder` for
    // structured handling.
    try_ast_walk_postorder(nodes, out, node_count, out_cap)
        .unwrap_or_else(|error| panic!("{error}"))
}

/// Emit postorder node indices for a VAST first-child / next-sibling tree with
/// checked launch-shape validation.
pub fn try_ast_walk_postorder(
    nodes: &str,
    out: &str,
    node_count: u32,
    out_cap: u32,
) -> Result<Program, String> {
    try_ast_walk_order(VastWalkOrder::Postorder, nodes, out, node_count, out_cap)
}

/// Emit node indices for a VAST first-child / next-sibling tree in the selected
/// traversal order with checked launch-shape validation.
pub fn try_ast_walk_order(
    order: VastWalkOrder,
    nodes: &str,
    out: &str,
    node_count: u32,
    out_cap: u32,
) -> Result<Program, String> {
    let op_id = order.op_id();
    let (stride, node_words, out_words) = checked_tree_walk_shape(node_count, out_cap, op_id)?;
    let body = match order {
        VastWalkOrder::Preorder => preorder_body(nodes, out, node_count, out_cap, stride),
        VastWalkOrder::Postorder => postorder_body(nodes, out, node_count, out_cap, stride),
    };

    Ok(tree_walk_program(
        op_id, nodes, out, node_words, out_words, body,
    ))
}

fn preorder_body(nodes: &str, out: &str, node_count: u32, out_cap: u32, stride: u32) -> Vec<Node> {
    let valid_node = |expr: Expr| valid_node_expr(expr, node_count);

    vec![
        Node::let_bind("oi", Expr::u32(0)),
        Node::let_bind("n", Expr::u32(0)),
        Node::loop_for(
            "step",
            Expr::u32(0),
            Expr::u32(node_count),
            vec![
                Node::if_then(
                    Expr::eq(Expr::u32(node_count), Expr::u32(0)),
                    vec![Node::return_()],
                ),
                Node::if_then(
                    Expr::ge(Expr::var("oi"), Expr::u32(out_cap)),
                    vec![Node::return_()],
                ),
                Node::if_then(
                    Expr::ge(Expr::var("n"), Expr::u32(node_count)),
                    vec![Node::return_()],
                ),
                Node::let_bind("base", Expr::mul(Expr::var("n"), Expr::u32(stride))),
                Node::let_bind(
                    "fc",
                    Expr::load(nodes, Expr::add(Expr::var("base"), Expr::u32(2))),
                ),
                Node::store(out, Expr::var("oi"), Expr::var("n")),
                Node::assign("oi", Expr::add(Expr::var("oi"), Expr::u32(1))),
                Node::if_then(
                    valid_node(Expr::var("fc")),
                    vec![Node::assign("n", Expr::var("fc"))],
                ),
                Node::if_then(
                    Expr::not(valid_node(Expr::var("fc"))),
                    vec![
                        Node::let_bind("next", Expr::u32(SENTINEL)),
                        Node::let_bind("walk", Expr::var("n")),
                        Node::loop_for(
                            "climb",
                            Expr::u32(0),
                            Expr::u32(node_count),
                            vec![Node::if_then(
                                Expr::and(
                                    Expr::eq(Expr::var("next"), Expr::u32(SENTINEL)),
                                    valid_node(Expr::var("walk")),
                                ),
                                vec![
                                    Node::let_bind(
                                        "walk_base",
                                        Expr::mul(Expr::var("walk"), Expr::u32(stride)),
                                    ),
                                    Node::let_bind(
                                        "sib",
                                        Expr::load(
                                            nodes,
                                            Expr::add(Expr::var("walk_base"), Expr::u32(3)),
                                        ),
                                    ),
                                    Node::if_then(
                                        valid_node(Expr::var("sib")),
                                        vec![Node::assign("next", Expr::var("sib"))],
                                    ),
                                    Node::if_then(
                                        Expr::not(valid_node(Expr::var("sib"))),
                                        vec![
                                            Node::let_bind(
                                                "parent",
                                                Expr::load(
                                                    nodes,
                                                    Expr::add(Expr::var("walk_base"), Expr::u32(1)),
                                                ),
                                            ),
                                            Node::assign("walk", Expr::var("parent")),
                                        ],
                                    ),
                                ],
                            )],
                        ),
                        Node::if_then(
                            Expr::eq(Expr::var("next"), Expr::u32(SENTINEL)),
                            vec![Node::return_()],
                        ),
                        Node::assign("n", Expr::var("next")),
                    ],
                ),
            ],
        ),
    ]
}

fn postorder_body(nodes: &str, out: &str, node_count: u32, out_cap: u32, stride: u32) -> Vec<Node> {
    let valid_node = |expr: Expr| valid_node_expr(expr, node_count);

    vec![
        Node::if_then(
            Expr::eq(Expr::u32(node_count), Expr::u32(0)),
            vec![Node::return_()],
        ),
        Node::let_bind("oi", Expr::u32(0)),
        Node::let_bind("n", Expr::u32(0)),
        descend_to_leftmost_leaf_node(nodes, node_count, stride),
        Node::loop_for(
            "emit",
            Expr::u32(0),
            Expr::u32(node_count),
            vec![
                Node::if_then(
                    Expr::ge(Expr::var("oi"), Expr::u32(out_cap)),
                    vec![Node::return_()],
                ),
                Node::if_then(
                    Expr::ge(Expr::var("n"), Expr::u32(node_count)),
                    vec![Node::return_()],
                ),
                Node::store(out, Expr::var("oi"), Expr::var("n")),
                Node::assign("oi", Expr::add(Expr::var("oi"), Expr::u32(1))),
                Node::if_then(
                    Expr::eq(Expr::var("n"), Expr::u32(0)),
                    vec![Node::return_()],
                ),
                Node::let_bind("base", Expr::mul(Expr::var("n"), Expr::u32(stride))),
                Node::let_bind(
                    "sib",
                    Expr::load(nodes, Expr::add(Expr::var("base"), Expr::u32(3))),
                ),
                Node::if_then(
                    valid_node(Expr::var("sib")),
                    vec![
                        Node::assign("n", Expr::var("sib")),
                        descend_to_leftmost_leaf_node(nodes, node_count, stride),
                    ],
                ),
                Node::if_then(
                    Expr::not(valid_node(Expr::var("sib"))),
                    vec![
                        Node::let_bind(
                            "parent",
                            Expr::load(nodes, Expr::add(Expr::var("base"), Expr::u32(1))),
                        ),
                        Node::if_then(
                            Expr::not(valid_node(Expr::var("parent"))),
                            vec![Node::return_()],
                        ),
                        Node::assign("n", Expr::var("parent")),
                    ],
                ),
            ],
        ),
    ]
}

fn checked_tree_walk_shape(
    node_count: u32,
    out_cap: u32,
    op_id: &'static str,
) -> Result<(u32, u32, u32), String> {
    let stride = NODE_STRIDE_U32 as u32;
    let node_words = checked_node_words(node_count, stride, op_id)?;
    let out_words = checked_out_words(out_cap, op_id)?;
    Ok((stride, node_words, out_words))
}

fn valid_node_expr(expr: Expr, node_count: u32) -> Expr {
    Expr::and(
        Expr::ne(expr.clone(), Expr::u32(SENTINEL)),
        Expr::lt(expr, Expr::u32(node_count)),
    )
}

fn descend_to_leftmost_leaf_node(nodes_name: &str, node_count: u32, stride: u32) -> Node {
    Node::loop_for(
        "descend",
        Expr::u32(0),
        Expr::u32(node_count),
        vec![Node::if_then(
            valid_node_expr(Expr::var("n"), node_count),
            vec![
                Node::let_bind(
                    "fc_idx",
                    Expr::add(Expr::mul(Expr::var("n"), Expr::u32(stride)), Expr::u32(2)),
                ),
                Node::let_bind("fc", Expr::load(nodes_name, Expr::var("fc_idx"))),
                Node::if_then(
                    valid_node_expr(Expr::var("fc"), node_count),
                    vec![Node::assign("n", Expr::var("fc"))],
                ),
            ],
        )],
    )
}

fn checked_node_words(node_count: u32, stride: u32, op_id: &'static str) -> Result<u32, String> {
    if node_count == 0 {
        return Ok(1);
    }
    node_count.checked_mul(stride).ok_or_else(|| {
        format!(
            "{op_id} node_count={node_count} stride={stride} overflows VAST node buffer words. Fix: shard the tree before GPU dispatch."
        )
    })
}

fn checked_out_words(out_cap: u32, op_id: &'static str) -> Result<u32, String> {
    if out_cap == 0 {
        Err(format!(
            "{op_id} requires out_cap > 0. Fix: allocate traversal output capacity before GPU dispatch."
        ))
    } else {
        Ok(out_cap)
    }
}

fn tree_walk_program(
    op_id: &'static str,
    nodes: &str,
    out: &str,
    node_words: u32,
    out_words: u32,
    body: Vec<Node>,
) -> Program {
    Program::wrapped(
        vec![
            BufferDecl::storage(nodes, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(node_words),
            BufferDecl::storage(out, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(out_words),
        ],
        [1, 1, 1],
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(body),
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_preorder_rejects_zero_output_capacity() {
        let error = try_ast_walk_preorder("nodes", "out", 1, 0)
            .expect_err("checked preorder builder must reject zero output capacity");

        assert!(
            error.contains("out_cap > 0"),
            "error should describe the launch-shape fix: {error}"
        );
    }

    #[test]
    fn checked_postorder_rejects_node_word_overflow() {
        let error = try_ast_walk_postorder("nodes", "out", u32::MAX, 1)
            .expect_err("checked postorder builder must reject node buffer overflow");

        assert!(
            error.contains("overflows VAST node buffer words"),
            "error should describe the VAST buffer overflow: {error}"
        );
    }

    #[test]
    fn checked_plan_builds_both_orders_from_primitive_authority() {
        let plan = try_ast_walk_plan("nodes", "pre", "post", 3, 3)
            .expect("Fix: primitive VAST plan should build both traversal orders");

        assert_eq!(plan.preorder.workgroup_size(), [1, 1, 1]);
        assert_eq!(plan.postorder.workgroup_size(), [1, 1, 1]);
        assert_eq!(plan.preorder.buffers().len(), 2);
        assert_eq!(plan.postorder.buffers().len(), 2);
    }

    #[test]
    fn checked_plan_rejects_shape_before_building_partial_facade_state() {
        let error = try_ast_walk_plan("nodes", "pre", "post", 3, 0)
            .expect_err("Fix: primitive VAST plan should reject invalid shared output capacity");

        assert!(
            error.contains("out_cap > 0"),
            "Fix: VAST plan diagnostic should come from the primitive output-capacity contract: {error}"
        );
    }

    #[test]
    fn legacy_vast_walk_builders_fail_fast_on_invalid_shape() {
        let preorder_panic = std::panic::catch_unwind(|| {
            let _ = ast_walk_preorder("nodes", "out", 1, 0);
        })
        .expect_err("legacy preorder builder must fail fast on zero output capacity");
        let postorder_panic = std::panic::catch_unwind(|| {
            let _ = ast_walk_postorder("nodes", "out", u32::MAX, 1);
        })
        .expect_err("legacy postorder builder must fail fast on node_count overflow");

        let preorder_message = panic_payload_message(preorder_panic);
        let postorder_message = panic_payload_message(postorder_panic);
        assert!(
            preorder_message.contains("out_cap > 0"),
            "error should describe the launch-shape fix: {preorder_message}"
        );
        assert!(
            postorder_message.contains("node_count"),
            "error should describe the node_count overflow: {postorder_message}"
        );
    }

    fn panic_payload_message(payload: Box<dyn std::any::Any + Send>) -> String {
        if let Some(message) = payload.downcast_ref::<&str>() {
            message.to_string()
        } else if let Some(message) = payload.downcast_ref::<String>() {
            message.clone()
        } else {
            format!("{payload:?}")
        }
    }

    #[test]
    fn vast_tree_walk_release_source_has_checked_builders_without_panics() {
        let source = include_str!("vast_tree_walk.rs");
        let production = source
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: VAST tree walk production source must precede tests");

        assert!(
            production.contains("pub fn try_ast_walk_preorder(")
                && production.contains("pub fn try_ast_walk_postorder(")
                && !production.contains("inert_")
                && !production.contains("Err(_) =>"),
            "Fix: VAST traversal builders must expose checked release APIs and must not compile inert no-op kernels."
        );
    }

    // -----------------------------------------------------------------------
    // CPU reference tree walk tests
    // -----------------------------------------------------------------------

    fn fixture_tree() -> Vec<u32> {
        vec![
            1, SENTINEL, 1, SENTINEL, 0, 0, 0, 0, 0,
            0, // node 0 (root): parent=SENTINEL, fc=1, ns=SENTINEL
            2, 0, SENTINEL, 2, 0, 0, 0, 0, 0, 0, // node 1: parent=0, fc=SENTINEL, ns=2
            3, 0, SENTINEL, SENTINEL, 0, 0, 0, 0, 0,
            0, // node 2: parent=0, fc=SENTINEL, ns=SENTINEL
        ]
    }

    fn valid(idx: u32, node_count: u32) -> bool {
        idx != SENTINEL && idx < node_count
    }

    fn cpu_preorder(nodes: &[u32], node_count: u32) -> Vec<u32> {
        if node_count == 0 {
            return Vec::new();
        }
        let stride = NODE_STRIDE_U32 as u32;
        let mut out = Vec::new();
        let mut n: u32 = 0;
        for _ in 0..node_count {
            if !valid(n, node_count) {
                break;
            }
            out.push(n);
            let base = (n * stride) as usize;
            let fc = nodes[base + 2];
            if valid(fc, node_count) {
                n = fc;
            } else {
                // Climb up to find a sibling
                let mut walk = n;
                let mut next = SENTINEL;
                while valid(walk, node_count) && next == SENTINEL {
                    let wb = (walk * stride) as usize;
                    let sib = nodes[wb + 3];
                    if valid(sib, node_count) {
                        next = sib;
                    } else {
                        walk = nodes[wb + 1]; // parent
                    }
                }
                if next == SENTINEL {
                    break;
                }
                n = next;
            }
        }
        out
    }

    fn cpu_postorder(nodes: &[u32], node_count: u32) -> Vec<u32> {
        if node_count == 0 {
            return Vec::new();
        }
        let stride = NODE_STRIDE_U32 as u32;
        let mut out = Vec::new();
        // Descend to leftmost leaf
        let mut n: u32 = 0;
        loop {
            let base = (n * stride) as usize;
            let fc = nodes[base + 2];
            if valid(fc, node_count) {
                n = fc;
            } else {
                break;
            }
        }
        for _ in 0..node_count {
            if !valid(n, node_count) {
                break;
            }
            out.push(n);
            if n == 0 {
                break;
            } // root emitted - done
            let base = (n * stride) as usize;
            let sib = nodes[base + 3];
            if valid(sib, node_count) {
                // Go to sibling's leftmost leaf
                n = sib;
                loop {
                    let sb = (n * stride) as usize;
                    let fc = nodes[sb + 2];
                    if valid(fc, node_count) {
                        n = fc;
                    } else {
                        break;
                    }
                }
            } else {
                n = nodes[base + 1]; // parent
            }
        }
        out
    }

    #[test]
    fn cpu_preorder_matches_inventory() {
        let tree = fixture_tree();
        let result = cpu_preorder(&tree, 3);
        assert_eq!(result, vec![0, 1, 2]);
    }

    #[test]
    fn cpu_postorder_matches_inventory() {
        let tree = fixture_tree();
        let result = cpu_postorder(&tree, 3);
        assert_eq!(result, vec![1, 2, 0]);
    }

    #[test]
    fn cpu_preorder_single_node() {
        let tree = vec![42u32, SENTINEL, SENTINEL, SENTINEL, 0, 0, 0, 0, 0, 0];
        assert_eq!(cpu_preorder(&tree, 1), vec![0]);
    }

    #[test]
    fn cpu_postorder_single_node() {
        let tree = vec![42u32, SENTINEL, SENTINEL, SENTINEL, 0, 0, 0, 0, 0, 0];
        assert_eq!(cpu_postorder(&tree, 1), vec![0]);
    }

    #[test]
    fn cpu_preorder_empty() {
        assert_eq!(cpu_preorder(&[], 0), Vec::<u32>::new());
    }

    #[test]
    fn cpu_postorder_empty() {
        assert_eq!(cpu_postorder(&[], 0), Vec::<u32>::new());
    }

    fn generated_parent(seed: u32, child: u32) -> u32 {
        seed.wrapping_mul(1_664_525)
            .wrapping_add(child.wrapping_mul(1_013_904_223))
            .rotate_left(child % 31)
            % child
    }

    fn generated_valid_tree(seed: u32, node_count: u32) -> Vec<u32> {
        let stride = NODE_STRIDE_U32 as usize;
        let mut nodes = vec![0u32; node_count as usize * stride];
        for node in 0..node_count {
            let base = node as usize * stride;
            nodes[base] = seed ^ node;
            nodes[base + 1] = SENTINEL;
            nodes[base + 2] = SENTINEL;
            nodes[base + 3] = SENTINEL;
        }

        for child in 1..node_count {
            let parent = generated_parent(seed, child);
            let child_base = child as usize * stride;
            let parent_base = parent as usize * stride;
            nodes[child_base + 1] = parent;

            if nodes[parent_base + 2] == SENTINEL {
                nodes[parent_base + 2] = child;
                continue;
            }

            let mut sibling = nodes[parent_base + 2];
            loop {
                let sibling_next = sibling as usize * stride + 3;
                if nodes[sibling_next] == SENTINEL {
                    nodes[sibling_next] = child;
                    break;
                }
                sibling = nodes[sibling_next];
            }
        }

        nodes
    }

    fn positions(order: &[u32], node_count: u32) -> Vec<u32> {
        let mut positions = vec![SENTINEL; node_count as usize];
        for (pos, node) in order.iter().copied().enumerate() {
            assert!(
                valid(node, node_count),
                "generated VAST traversal emitted invalid node {node}"
            );
            assert_eq!(
                positions[node as usize], SENTINEL,
                "generated VAST traversal emitted node {node} twice"
            );
            positions[node as usize] = pos as u32;
        }
        assert!(
            positions.iter().all(|pos| *pos != SENTINEL),
            "generated VAST traversal missed at least one node"
        );
        positions
    }

    #[test]
    fn generated_vast_walk_orders_match_tree_order_contracts() {
        for seed in 0..2048u32 {
            let node_count = seed % 37 + 1;
            let tree = generated_valid_tree(seed, node_count);
            let preorder = cpu_preorder(&tree, node_count);
            let postorder = cpu_postorder(&tree, node_count);

            assert_eq!(
                preorder.len(),
                node_count as usize,
                "preorder must emit every generated VAST node exactly once for seed {seed}"
            );
            assert_eq!(
                postorder.len(),
                node_count as usize,
                "postorder must emit every generated VAST node exactly once for seed {seed}"
            );

            let preorder_positions = positions(&preorder, node_count);
            let postorder_positions = positions(&postorder, node_count);
            let stride = NODE_STRIDE_U32 as usize;

            for child in 1..node_count {
                let parent = tree[child as usize * stride + 1];
                assert!(
                    preorder_positions[parent as usize] < preorder_positions[child as usize],
                    "preorder must emit parent {parent} before child {child} for seed {seed}"
                );
                assert!(
                    postorder_positions[child as usize] < postorder_positions[parent as usize],
                    "postorder must emit child {child} before parent {parent} for seed {seed}"
                );
            }
        }
    }
}

#[cfg(feature = "inventory-registry")]

fn fixture_u32(words: &[u32]) -> Vec<u8> {
    crate::wire::pack_u32_slice(words)
}

#[cfg(feature = "inventory-registry")]
fn fixture_tree_words() -> Vec<u32> {
    vec![
        1, SENTINEL, 1, SENTINEL, 0, 0, 0, 0, 0, 0, // root
        2, 0, SENTINEL, 2, 0, 0, 0, 0, 0, 0, // first child
        3, 0, SENTINEL, SENTINEL, 0, 0, 0, 0, 0, 0, // second child
    ]
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        PREORDER_OP_ID,
        || ast_walk_preorder("nodes", "out", 3, 3),
        Some(|| vec![vec![
            fixture_u32(&fixture_tree_words()),
            fixture_u32(&[SENTINEL, SENTINEL, SENTINEL]),
        ]]),
        Some(|| vec![vec![fixture_u32(&[0, 1, 2])]]),
    )
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        POSTORDER_OP_ID,
        || ast_walk_postorder("nodes", "out", 3, 3),
        Some(|| vec![vec![
            fixture_u32(&fixture_tree_words()),
            fixture_u32(&[SENTINEL, SENTINEL, SENTINEL]),
        ]]),
        Some(|| vec![vec![fixture_u32(&[1, 2, 0])]]),
    )
}
