//! Visitor walk  -  a bounded post-order tree traversal primitive.
//!
//! Compilers spend most of their time walking trees: AST visitors, scope-tree
//! traversals, dominator walks.  `visitor_walk` gives vyre a first-class
//! primitive for that coordination.  It takes a root node and a CSR child
//! table, then emits a post-order sequence using an explicit stack that lives
//! in workgroup-local SRAM.  The target-text kernel uses the same stack bound and
//! cycle-detection logic, so conform can prove the visit order is identical on
//! CPU and GPU.

use crate::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};
use thiserror::Error;
use vyre_spec::AlgebraicLaw;

/// Registered target-text source for the visitor walk primitive.
#[must_use]
pub fn source() -> Option<&'static str> {
    crate::transform::compiler::shader_provider::source("visitor_walk")
}

/// Sentinel for "no more visits queued" in the visit stack.
pub const VISIT_STACK_EMPTY: u32 = u32::MAX;

/// Build a vyre IR Program that pops one node off the visit stack
/// and appends its children. Callers run this repeatedly until the
/// stack drains; the Program records each popped node in
/// `post_order` in the order of pops, producing a post-order walk
/// when the caller pushes children before parents.
///
/// Buffers:
/// - `child_offsets`: `ReadOnly` u32 array of length `node_count` + 1  -
///   CSR offsets into the child table.
/// - `children`: `ReadOnly` u32 array  -  flat child list.
/// - `stack`: `ReadWrite` u32 array  -  the explicit DFS stack, with
///   `stack[0]` holding the top-of-stack index and `stack[1..]` the contents.
/// - `post_order`: `ReadWrite` u32 array  -  sequence of popped nodes.
/// - `post_count`: `ReadWrite` u32 array of length 1  -  how many
///   entries in `post_order` are populated.
///
/// The IR pops one node per dispatch. The host loop wrapping this
/// Program terminates when `stack` slot 0 equals 0.
#[must_use]
pub fn visit_step_program(
    child_offsets: &str,
    children: &str,
    stack: &str,
    post_order: &str,
    post_count: &str,
) -> Program {
    let body = vec![
        Node::let_bind("top", Expr::load(stack, Expr::u32(0))),
        Node::if_then(
            Expr::ne(Expr::var("top"), Expr::u32(0)),
            vec![
                // Pop: stack[top] is the node we visit next.
                Node::let_bind("node", Expr::load(stack, Expr::var("top"))),
                Node::let_bind("new_top", Expr::sub(Expr::var("top"), Expr::u32(1))),
                Node::store(stack, Expr::u32(0), Expr::var("new_top")),
                // Record in post_order.
                Node::let_bind(
                    "pc",
                    Expr::atomic_add(post_count, Expr::u32(0), Expr::u32(1)),
                ),
                Node::store(post_order, Expr::var("pc"), Expr::var("node")),
                // Push children onto the stack.
                Node::let_bind("cs", Expr::load(child_offsets, Expr::var("node"))),
                Node::let_bind(
                    "ce",
                    Expr::load(child_offsets, Expr::add(Expr::var("node"), Expr::u32(1))),
                ),
                Node::loop_for(
                    "i",
                    Expr::var("cs"),
                    Expr::var("ce"),
                    vec![
                        Node::let_bind("c", Expr::load(children, Expr::var("i"))),
                        Node::let_bind(
                            "t2",
                            Expr::add(Expr::load(stack, Expr::u32(0)), Expr::u32(1)),
                        ),
                        Node::store(stack, Expr::u32(0), Expr::var("t2")),
                        Node::store(stack, Expr::var("t2"), Expr::var("c")),
                    ],
                ),
            ],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(child_offsets, 0, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(children, 1, BufferAccess::ReadOnly, DataType::U32),
            BufferDecl::storage(stack, 2, BufferAccess::ReadWrite, DataType::U32),
            BufferDecl::storage(post_order, 3, BufferAccess::ReadWrite, DataType::U32),
            BufferDecl::storage(post_count, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [1, 1, 1],
        body,
    )
}

impl VisitorWalkOp {}

/// Safely cast a `u32` node id to `usize` for host indexing.
///
/// # Errors
///
/// Returns `VisitorWalkError::IndexOverflow` if the value does not fit.
#[must_use]
pub fn index(value: u32) -> Result<usize, VisitorWalkError> {
    usize::try_from(value).map_err(|_| VisitorWalkError::IndexOverflow)
}

/// Algebraic laws declared by the visitor-walk primitive.
pub const LAWS: &[AlgebraicLaw] = &[AlgebraicLaw::Bounded {
    lo: 0,
    hi: u32::MAX,
}];

/// Produce a post-order tree visit sequence from CSR children.
///
/// # Errors
///
/// Returns `Fix: ...` when the tree is malformed, cyclic, or exceeds the
/// explicit stack/output bounds.
#[must_use]
pub fn postorder(
    root: u32,
    child_offsets: &[u32],
    children: &[u32],
    max_stack: usize,
) -> Result<Vec<u32>, VisitorWalkError> {
    let node_count = child_offsets
        .len()
        .checked_sub(1)
        .ok_or(VisitorWalkError::EmptyOffsets)?;
    let root_index = index(root)?;
    if root_index >= node_count {
        return Err(VisitorWalkError::InvalidRoot { root, node_count });
    }
    validate_tree(node_count, child_offsets, children)?;
    let mut seen = vec![false; node_count];
    let mut sequence = Vec::with_capacity(node_count);
    let mut stack = Vec::with_capacity(max_stack.min(node_count).saturating_add(1));
    stack.push((root, false));
    while let Some((node, expanded)) = stack.pop() {
        let node_index = index(node)?;
        if expanded {
            sequence.push(node);
            continue;
        }
        if seen[node_index] {
            return Err(VisitorWalkError::Cycle { node });
        }
        seen[node_index] = true;
        if stack.len().saturating_add(1) > max_stack {
            return Err(VisitorWalkError::StackOverflow { max_stack });
        }
        stack.push((node, true));
        let start = index(child_offsets[node_index])?;
        let end = index(child_offsets[node_index + 1])?;
        for &child in children[start..end].iter().rev() {
            if stack.len().saturating_add(1) > max_stack {
                return Err(VisitorWalkError::StackOverflow { max_stack });
            }
            stack.push((child, false));
        }
    }
    Ok(sequence)
}

/// Validate that `offsets` and `children` form a well-formed CSR child table.
///
/// # Errors
///
/// Returns `Fix: ...` when offsets are non-monotone, out of range, or when
/// any child id exceeds the node count.
#[must_use]
pub fn validate_tree(
    node_count: usize,
    offsets: &[u32],
    children: &[u32],
) -> Result<(), VisitorWalkError> {
    let mut previous = 0usize;
    for &offset in offsets {
        let current = index(offset)?;
        if current < previous || current > children.len() {
            return Err(VisitorWalkError::InvalidOffset);
        }
        previous = current;
    }
    for &child in children {
        if index(child)? >= node_count {
            return Err(VisitorWalkError::InvalidChild { child, node_count });
        }
    }
    Ok(())
}

/// Visitor-walk validation errors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum VisitorWalkError {
    /// Offset table has no terminal offset.
    #[error(
        "VisitorEmptyOffsets: child_offsets must include node_count + 1 entries. Fix: emit a valid tree CSR table."
    )]
    EmptyOffsets,
    /// Root node is outside the tree.
    #[error(
        "VisitorInvalidRoot: root {root} outside node_count {node_count}. Fix: pass a valid AST root."
    )]
    InvalidRoot {
        /// Invalid root.
        root: u32,
        /// Node count.
        node_count: usize,
    },
    /// Child offsets are not monotone or exceed child length.
    #[error(
        "VisitorInvalidOffset: child offsets must be monotone and within children. Fix: rebuild child_offsets."
    )]
    InvalidOffset,
    /// Node id cannot fit in host index space.
    #[error("VisitorIndexOverflow: node id cannot fit usize. Fix: split the AST before dispatch.")]
    IndexOverflow,
    /// Child node is outside the tree.
    #[error(
        "VisitorInvalidChild: child {child} outside node_count {node_count}. Fix: validate AST child references."
    )]
    InvalidChild {
        /// Invalid child id.
        child: u32,
        /// Node count.
        node_count: usize,
    },
    /// Traversal found a cycle instead of a tree.
    #[error(
        "VisitorCycle: node {node} was reached twice. Fix: pass a tree or DAG-expanded AST, not a cyclic graph."
    )]
    Cycle {
        /// Revisited node.
        node: u32,
    },
    /// Explicit traversal stack exceeded its bound.
    #[error(
        "VisitorStackOverflow: stack exceeded {max_stack} entries. Fix: increase workgroup visitor stack or split the AST."
    )]
    StackOverflow {
        /// Stack capacity.
        max_stack: usize,
    },
}

/// Category C visitor-walk intrinsic.
#[derive(Debug, Default, Clone, Copy)]
pub struct VisitorWalkOp;

/// Workgroup size used by the reference target-text lowering.
pub const WORKGROUP_SIZE: [u32; 3] = [64, 1, 1];

#[cfg(test)]
mod ir_program_tests {
    use super::*;

    #[test]
    fn visit_step_program_validates() {
        let prog = visit_step_program("co", "c", "stack", "po", "pc");
        let errors = crate::validate::validate::validate(&prog);
        assert!(errors.is_empty(), "visitor IR must validate: {errors:?}");
    }

    #[test]
    fn visit_step_program_wire_round_trips() {
        let prog = visit_step_program("co", "c", "stack", "po", "pc");
        let bytes = prog
            .to_wire()
            .expect("Fix: serialize; restore this invariant before continuing.");
        let decoded = Program::from_wire(&bytes)
            .expect("Fix: decode; restore this invariant before continuing.");
        assert_eq!(decoded.buffers().len(), 5);
    }

    #[test]
    fn visit_stack_empty_sentinel_is_u32_max() {
        assert_eq!(VISIT_STACK_EMPTY, u32::MAX);
    }
}
