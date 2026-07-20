//! Shared guarded indexed move kernels for gather/scatter.
//!
//! Both operations read `indices[i]`, guard it against the logical
//! element count, and move one u32 between `src` and `dst`. The mode
//! only decides which side is indexed indirectly.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Guarded indexed move direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IndexedMoveKind {
    /// `dst[i] = src[indices[i]]`.
    Gather,
    /// `dst[indices[i]] = src[i]`.
    Scatter,
}

impl IndexedMoveKind {
    fn name(self) -> &'static str {
        match self {
            Self::Gather => "gather",
            Self::Scatter => "scatter",
        }
    }

    fn store_node(self, src: &str, dst: &str, lane: Expr) -> Node {
        match self {
            Self::Gather => Node::store(dst, lane, Expr::load(src, Expr::var("idx"))),
            Self::Scatter => Node::store(dst, Expr::var("idx"), Expr::load(src, lane)),
        }
    }
}

/// Build guarded gather/scatter over `count` u32 lanes.
#[must_use]
pub(crate) fn indexed_move_program(
    op_id: &'static str,
    src: &str,
    indices: &str,
    dst: &str,
    count: u32,
    kind: IndexedMoveKind,
) -> Program {
    if count == 0 {
        return crate::invalid_output_program(
            op_id,
            dst,
            DataType::U32,
            format!("Fix: {} requires count > 0, got {count}.", kind.name()),
        );
    }

    let t = Expr::InvocationId { axis: 0 };
    let body = vec![
        Node::let_bind("idx", Expr::load(indices, t.clone())),
        Node::if_then(
            Expr::lt(Expr::var("idx"), Expr::u32(count)),
            vec![kind.store_node(src, dst, t.clone())],
        ),
    ];

    Program::wrapped(
        vec![
            BufferDecl::storage(src, 0, BufferAccess::ReadOnly, DataType::U32).with_count(count),
            BufferDecl::storage(indices, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(count),
            BufferDecl::storage(dst, 2, BufferAccess::ReadWrite, DataType::U32).with_count(count),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(t.clone(), Expr::u32(count)),
                body,
            )]),
        }],
    )
}

/// CPU oracle for gather/scatter into caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) fn indexed_move_cpu_ref_into(
    kind: IndexedMoveKind,
    src: &[u32],
    indices: &[u32],
    dst_len: usize,
    dst: &mut Vec<u32>,
) {
    if let Err(error) = try_indexed_move_cpu_ref_into(kind, src, indices, dst_len, dst) {
        // A parity oracle that clears to empty on failure makes the GPU-vs-CPU
        // assertion pass on empty==empty, silently masking a divergence
        // (Law 10 / Law 6). Fail loud; callers use the try_ variant.
        panic!("vyre-primitives indexed {kind:?} CPU reference failed: {error}");
    }
}

/// Fallible CPU oracle for gather/scatter into caller-owned output storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) fn try_indexed_move_cpu_ref_into(
    kind: IndexedMoveKind,
    src: &[u32],
    indices: &[u32],
    dst_len: usize,
    dst: &mut Vec<u32>,
) -> Result<(), String> {
    match kind {
        IndexedMoveKind::Gather => {
            if indices.len() > dst.capacity() {
                dst.try_reserve_exact(indices.len() - dst.capacity())
                    .map_err(|err| {
                        format!(
                            "gather CPU reference could not reserve {} output words: {err}",
                            indices.len()
                        )
                    })?;
            }
            dst.clear();
            for &idx in indices {
                let value = usize::try_from(idx)
                    .ok()
                    .and_then(|index| src.get(index))
                    .copied()
                    .unwrap_or(0);
                dst.push(value);
            }
        }
        IndexedMoveKind::Scatter => {
            if dst_len > dst.capacity() {
                dst.try_reserve_exact(dst_len - dst.capacity())
                    .map_err(|err| {
                        format!(
                            "scatter CPU reference could not reserve {dst_len} output words: {err}"
                        )
                    })?;
            }
            dst.clear();
            dst.resize(dst_len, 0);
            for (src_index, &dst_index) in indices.iter().enumerate() {
                if let Ok(dst_index) = usize::try_from(dst_index) {
                    if dst_index >= dst.len() {
                        continue;
                    }
                    if let Some(&value) = src.get(src_index) {
                        dst[dst_index] = value;
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar_ref(kind: IndexedMoveKind, src: &[u32], indices: &[u32], dst_len: usize) -> Vec<u32> {
        match kind {
            IndexedMoveKind::Gather => indices
                .iter()
                .map(|&idx| src.get(idx as usize).copied().unwrap_or(0))
                .collect(),
            IndexedMoveKind::Scatter => {
                let mut dst = vec![0_u32; dst_len];
                for (src_index, &dst_index) in indices.iter().enumerate() {
                    if let Some(slot) = dst.get_mut(dst_index as usize) {
                        if let Some(&value) = src.get(src_index) {
                            *slot = value;
                        }
                    }
                }
                dst
            }
        }
    }

    #[test]
    fn generated_indexed_moves_match_scalar_reference() {
        let mut state = 0x1D15_EA5E_u32;
        for case in 0..4096_u32 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let src_len = (state as usize % 97) + 1;
            let index_len = ((state >> 8) as usize % 101) + 1;
            let dst_len = ((state >> 16) as usize % 103) + 1;
            let mut src = Vec::with_capacity(src_len);
            for src_index in 0..src_len {
                state = state.rotate_left(7) ^ (src_index as u32).wrapping_mul(0x9E37_79B9);
                src.push(state);
            }
            let mut indices = Vec::with_capacity(index_len);
            for index in 0..index_len {
                state = state.rotate_left(11) ^ (index as u32).wrapping_mul(0x85EB_CA6B);
                let value = match index % 7 {
                    0 => 0,
                    1 => (src_len - 1) as u32,
                    2 => dst_len.saturating_sub(1) as u32,
                    3 => src_len as u32,
                    4 => dst_len as u32,
                    5 => u32::MAX,
                    _ => state % (src_len.max(dst_len) as u32 + 3),
                };
                indices.push(value);
            }

            for kind in [IndexedMoveKind::Gather, IndexedMoveKind::Scatter] {
                let mut got = Vec::new();
                try_indexed_move_cpu_ref_into(kind, &src, &indices, dst_len, &mut got).unwrap();
                assert_eq!(
                    got,
                    scalar_ref(kind, &src, &indices, dst_len),
                    "case {case} kind {kind:?}"
                );
            }
        }
    }

    #[test]
    fn indexed_moves_clear_stale_tail_without_reallocating() {
        let src = [10_u32, 20, 30, 40];
        let indices = [3_u32, 0, 99, 1];
        for kind in [IndexedMoveKind::Gather, IndexedMoveKind::Scatter] {
            let mut out = Vec::with_capacity(16);
            out.extend_from_slice(&[u32::MAX; 16]);
            let ptr = out.as_ptr();

            try_indexed_move_cpu_ref_into(kind, &src, &indices, 4, &mut out).unwrap();

            assert_eq!(out, scalar_ref(kind, &src, &indices, 4));
            assert_eq!(out.as_ptr(), ptr);
        }
    }

    #[test]
    fn compatibility_wrapper_matches_fallible_reference() {
        let src = [10_u32, 20, 30, 40];
        let indices = [3_u32, 0, 99, 1];

        for kind in [IndexedMoveKind::Gather, IndexedMoveKind::Scatter] {
            let mut compat = Vec::with_capacity(16);
            let mut fallible = Vec::with_capacity(16);

            indexed_move_cpu_ref_into(kind, &src, &indices, 4, &mut compat);
            try_indexed_move_cpu_ref_into(kind, &src, &indices, 4, &mut fallible)
                .expect("Fix: small indexed move CPU reference must reserve");

            assert_eq!(compat, fallible);
        }
    }

    #[test]
    fn production_indexed_move_wrapper_has_no_raw_panic_path() {
        let production = include_str!("indexed_move.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: indexed_move.rs must contain production section");

        assert!(
            !production.contains(".expect(") && !production.contains(".unwrap("),
            "Fix: indexed move CPU parity wrapper must not panic in production."
        );
    }
}
