//! Explicit CPU oracle VAST construction and decode errors.
//!
//! Production VAST construction must use `c11_build_vast_nodes`. The byte
//! decode helpers remain shared by oracle-only classify/typedef/expression
//! fixtures so malformed parity inputs fail with actionable diagnostics.

#![allow(missing_docs)] // Internal oracle helpers are documented at the owning module boundary.
use crate::parsing::c::lex::tokens::*;

use super::expr_shape::*;
use super::ref_expr_shape::*;
use super::*;

#[deprecated(
    note = "CPU oracle only; production VAST construction must dispatch c11_build_vast_nodes"
)]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn reference_c11_build_vast_nodes(
    tok_types: &[u32],
    tok_starts: &[u32],
    tok_lens: &[u32],
) -> Vec<u8> {
    let n = tok_types.len().min(tok_starts.len()).min(tok_lens.len());
    let mut parent = vec![SENTINEL; n];
    let mut first_child = vec![SENTINEL; n];
    let mut next_sibling = vec![SENTINEL; n];
    let mut previous_sibling = vec![SENTINEL; n];
    let mut stack: Vec<u32> = Vec::new();
    let mut last_child: Vec<Option<u32>> = vec![None; n];
    let mut root_last: Option<u32> = None;

    for i in 0..n {
        let parent_idx = stack.last().copied().unwrap_or(SENTINEL);
        parent[i] = parent_idx;

        if let Some(previous) = if parent_idx == SENTINEL {
            root_last
        } else {
            last_child[parent_idx as usize]
        } {
            previous_sibling[i] = previous;
            next_sibling[previous as usize] = i as u32;
        } else if parent_idx != SENTINEL {
            first_child[parent_idx as usize] = i as u32;
        }

        if parent_idx == SENTINEL {
            root_last = Some(i as u32);
        } else {
            last_child[parent_idx as usize] = Some(i as u32);
        }

        match tok_types[i] {
            TOK_LPAREN | TOK_LBRACE | TOK_LBRACKET => stack.push(i as u32),
            TOK_RPAREN => pop_matching(&mut stack, tok_types, TOK_LPAREN),
            TOK_RBRACE => pop_matching(&mut stack, tok_types, TOK_LBRACE),
            TOK_RBRACKET => pop_matching(&mut stack, tok_types, TOK_LBRACKET),
            _ => {}
        }
    }

    let mut rows = Vec::with_capacity(n.saturating_mul(VAST_NODE_STRIDE_U32 as usize));
    for i in 0..n {
        rows.extend_from_slice(&[
            tok_types[i],
            parent[i],
            first_child[i],
            next_sibling[i],
            previous_sibling[i],
            tok_starts[i],
            tok_lens[i],
            0,
            0,
            0,
        ]);
    }
    u32_words_to_bytes(&rows)
}

/// Malformed byte input for C VAST CPU oracle decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CReferenceDecodeError {
    /// Input byte length is not a whole number of `u32` words.
    MisalignedBytes {
        /// Actual byte length.
        len: usize,
    },
    /// Input word count is not a whole number of VAST rows.
    PartialVastRow {
        /// Actual decoded word count.
        words: usize,
        /// Required row stride.
        stride: usize,
    },
    /// Two VAST streams that must describe the same node set have
    /// different row counts.
    MismatchedVastRows {
        /// Row count in the raw VAST stream.
        raw_rows: usize,
        /// Row count in the typed VAST stream.
        typed_rows: usize,
    },
}

impl std::fmt::Display for CReferenceDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MisalignedBytes { len } => write!(
                formatter,
                "C VAST byte input has {len} bytes, which is not 4-byte aligned. Fix: pass complete u32 rows to the C VAST reference oracle."
            ),
            Self::PartialVastRow { words, stride } => write!(
                formatter,
                "C VAST word input has {words} words, which is not a multiple of row stride {stride}. Fix: pass complete C VAST rows to the reference oracle."
            ),
            Self::MismatchedVastRows {
                raw_rows,
                typed_rows,
            } => write!(
                formatter,
                "C VAST reference oracle received {raw_rows} raw rows but {typed_rows} typed rows. Fix: pass matching raw and typed VAST streams from the same translation unit."
            ),
        }
    }
}

impl std::error::Error for CReferenceDecodeError {}

fn try_u32_words_from_bytes(bytes: &[u8]) -> Result<Vec<u32>, CReferenceDecodeError> {
    if bytes.len() % 4 != 0 {
        return Err(CReferenceDecodeError::MisalignedBytes { len: bytes.len() });
    }
    Ok(vyre_primitives::wire::decode_u32_le_bytes_all(bytes))
}

pub(super) fn try_vast_words_from_bytes(bytes: &[u8]) -> Result<Vec<u32>, CReferenceDecodeError> {
    let words = try_u32_words_from_bytes(bytes)?;
    if words.len() % VAST_NODE_STRIDE_U32 as usize != 0 {
        return Err(CReferenceDecodeError::PartialVastRow {
            words: words.len(),
            stride: VAST_NODE_STRIDE_U32 as usize,
        });
    }
    Ok(words)
}
