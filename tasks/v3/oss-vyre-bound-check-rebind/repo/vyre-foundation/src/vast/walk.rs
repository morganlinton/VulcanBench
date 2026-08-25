//! Host VAST tree walks.

use super::error::VastError;
use super::node::{VastNode, NODE_STRIDE_U32, SENTINEL};

/// Preorder walk (node indices). Uses an explicit stack; `max_stack` bounds work.
///
/// # Errors
///
/// Returns [`VastError`] when the node table is malformed or the stack cap is exceeded.
pub fn walk_preorder_indices(
    node_bytes: &[u8],
    node_count: u32,
    max_stack: usize,
) -> Result<Vec<u32>, VastError> {
    if node_count == 0 {
        return Ok(Vec::new());
    }
    let expected = (node_count as usize)
        .checked_mul(NODE_STRIDE_U32 * 4)
        .ok_or(VastError::LengthMismatch {
            expected: usize::MAX,
            got: node_bytes.len(),
        })?;
    if node_bytes.len() != expected {
        return Err(VastError::NodeTableSize {
            expected,
            got: node_bytes.len(),
        });
    }
    let mut out = Vec::new();
    let mut stack = vec![0u32];
    while let Some(n) = stack.pop() {
        out.push(n);
        let node = VastNode::read_row_bytes(node_bytes, n).ok_or(VastError::NodeTableSize {
            expected,
            got: node_bytes.len(),
        })?;
        let mut children = Vec::new();
        let mut c = node.first_child;
        while c != SENTINEL {
            children.push(c);
            let ch = VastNode::read_row_bytes(node_bytes, c).ok_or(VastError::NodeTableSize {
                expected,
                got: node_bytes.len(),
            })?;
            c = ch.next_sibling;
        }
        for c in children.into_iter().rev() {
            if stack.len() >= max_stack {
                return Err(VastError::StackOverflow { cap: max_stack });
            }
            stack.push(c);
        }
    }
    Ok(out)
}

/// Postorder walk (node indices).
///
/// # Errors
///
/// Returns [`VastError`] when the node table is malformed or the stack cap is exceeded.
pub fn walk_postorder_indices(
    node_bytes: &[u8],
    node_count: u32,
    max_stack: usize,
) -> Result<Vec<u32>, VastError> {
    if node_count == 0 {
        return Ok(Vec::new());
    }
    let expected = (node_count as usize)
        .checked_mul(NODE_STRIDE_U32 * 4)
        .ok_or(VastError::LengthMismatch {
            expected: usize::MAX,
            got: node_bytes.len(),
        })?;
    if node_bytes.len() != expected {
        return Err(VastError::NodeTableSize {
            expected,
            got: node_bytes.len(),
        });
    }
    let mut out = Vec::new();
    let mut stack = vec![(0u32, false)];
    while let Some((n, expanded)) = stack.pop() {
        if expanded {
            out.push(n);
            continue;
        }
        if stack.len() >= max_stack {
            return Err(VastError::StackOverflow { cap: max_stack });
        }
        stack.push((n, true));
        let node = VastNode::read_row_bytes(node_bytes, n).ok_or(VastError::NodeTableSize {
            expected,
            got: node_bytes.len(),
        })?;
        let mut children = Vec::new();
        let mut c = node.first_child;
        while c != SENTINEL {
            children.push(c);
            let ch = VastNode::read_row_bytes(node_bytes, c).ok_or(VastError::NodeTableSize {
                expected,
                got: node_bytes.len(),
            })?;
            c = ch.next_sibling;
        }
        for c in children.into_iter().rev() {
            if stack.len() >= max_stack {
                return Err(VastError::StackOverflow { cap: max_stack });
            }
            stack.push((c, false));
        }
    }
    Ok(out)
}
