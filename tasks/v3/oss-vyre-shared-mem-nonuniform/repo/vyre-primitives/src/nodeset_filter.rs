//! Shared u32-per-node to packed-NodeSet filter kernel.
//!
//! Several primitives scan one u32 fact per node, test a compile-time
//! predicate, and atomically set the corresponding bit in a packed
//! NodeSet. Centralizing that skeleton prevents node-kind, label-family,
//! and future tag predicates from drifting at word boundaries.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Compile-time predicate applied to each per-node u32 value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NodeSetFilter {
    /// Match exactly one u32 value.
    Eq(u32),
    /// Match when any bit in the mask is present.
    Intersects(u32),
}

impl NodeSetFilter {
    fn expr(self, value: Expr) -> Expr {
        match self {
            Self::Eq(expected) => Expr::eq(value, Expr::u32(expected)),
            Self::Intersects(mask) => Expr::ne(Expr::bitand(value, Expr::u32(mask)), Expr::u32(0)),
        }
    }

    #[cfg(any(test, feature = "cpu-parity"))]
    fn matches(self, value: u32) -> bool {
        match self {
            Self::Eq(expected) => value == expected,
            Self::Intersects(mask) => (value & mask) != 0,
        }
    }
}

/// Build `nodeset_out = { v : filter(values[v]) }`.
#[must_use]
pub(crate) fn nodeset_filter_program(
    op_id: &'static str,
    values: &str,
    nodeset_out: &str,
    node_count: u32,
    filter: NodeSetFilter,
) -> Program {
    let t = Expr::InvocationId { axis: 0 };
    let words = node_count.div_ceil(32);
    let value = Expr::load(values, t.clone());
    let body = vec![Node::if_then(
        filter.expr(value),
        vec![
            Node::let_bind("word_idx", Expr::shr(t.clone(), Expr::u32(5))),
            Node::let_bind(
                "bit",
                Expr::shl(Expr::u32(1), Expr::bitand(t.clone(), Expr::u32(31))),
            ),
            Node::let_bind(
                "_",
                Expr::atomic_or(nodeset_out, Expr::var("word_idx"), Expr::var("bit")),
            ),
        ],
    )];
    Program::wrapped(
        vec![
            BufferDecl::storage(values, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(node_count),
            BufferDecl::storage(nodeset_out, 1, BufferAccess::ReadWrite, DataType::U32)
                .with_count(words),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(op_id),
            source_region: None,
            body: Arc::new(vec![Node::if_then(
                Expr::lt(t.clone(), Expr::u32(node_count)),
                body,
            )]),
        }],
    )
}

/// CPU reference for `nodeset_filter_program`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) fn nodeset_filter_cpu_ref(values: &[u32], filter: NodeSetFilter) -> Vec<u32> {
    let mut out = Vec::new();
    nodeset_filter_cpu_ref_into(values, filter, &mut out);
    out
}

/// CPU reference using a caller-owned output buffer.
#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) fn nodeset_filter_cpu_ref_into(
    values: &[u32],
    filter: NodeSetFilter,
    out: &mut Vec<u32>,
) {
    if let Err(error) = try_nodeset_filter_cpu_ref_into(values, filter, out) {
        // A parity oracle that clears to empty on failure makes the GPU-vs-CPU
        // assertion pass on empty==empty, silently masking a divergence
        // (Law 10 / Law 6). Fail loud; callers use the try_ variant.
        panic!("vyre-primitives nodeset_filter CPU reference failed: {error}");
    }
}

/// Fallible CPU reference using a caller-owned output buffer.
///
/// `out` is not cleared until the target storage has been reserved.
#[cfg(any(test, feature = "cpu-parity"))]
pub(crate) fn try_nodeset_filter_cpu_ref_into(
    values: &[u32],
    filter: NodeSetFilter,
    out: &mut Vec<u32>,
) -> Result<(), String> {
    let words = values.len().div_ceil(32);
    let additional = words.saturating_sub(out.capacity());
    out.try_reserve_exact(additional)
        .map_err(|err| format!("failed to reserve nodeset filter output: {err}"))?;
    out.clear();
    out.resize(words, 0);
    for (node, value) in values.iter().copied().enumerate() {
        if filter.matches(value) {
            out[node / 32] |= 1_u32 << (node % 32);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar_ref(values: &[u32], filter: NodeSetFilter) -> Vec<u32> {
        let mut out = vec![0_u32; values.len().div_ceil(32)];
        for (node, value) in values.iter().copied().enumerate() {
            if filter.matches(value) {
                out[node / 32] |= 1_u32 << (node % 32);
            }
        }
        out
    }

    #[test]
    fn generated_filters_match_scalar_reference() {
        let mut state = 0xF117_EA5E_u32;
        for case in 0..4096_u32 {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let len = (state as usize % 257).min(case as usize % 257);
            let exact = state.rotate_left(case & 31);
            let mask = 1_u32 << (case & 31);
            let filters = [NodeSetFilter::Eq(exact), NodeSetFilter::Intersects(mask)];
            let mut values = Vec::with_capacity(len);
            for index in 0..len {
                state = state.rotate_left(9) ^ (index as u32).wrapping_mul(0x9E37_79B9);
                let value = match index % 5 {
                    0 => exact,
                    1 => mask,
                    2 => exact ^ mask,
                    3 => !mask,
                    _ => state,
                };
                values.push(value);
            }
            for filter in filters {
                assert_eq!(
                    nodeset_filter_cpu_ref(&values, filter),
                    scalar_ref(&values, filter),
                    "case {case} filter {filter:?}"
                );
            }
        }
    }

    #[test]
    fn cpu_ref_into_reuses_output_and_clears_stale_tail() {
        let values = [1_u32, 2, 3, 4, 5, 6, 7, 8];
        let mut out = Vec::with_capacity(4);
        out.extend([u32::MAX; 4]);
        let ptr = out.as_ptr();
        try_nodeset_filter_cpu_ref_into(&values, NodeSetFilter::Intersects(0b1), &mut out).unwrap();
        assert_eq!(out, vec![0b0101_0101]);
        assert_eq!(out.as_ptr(), ptr);

        try_nodeset_filter_cpu_ref_into(&[], NodeSetFilter::Eq(1), &mut out).unwrap();
        assert!(out.is_empty());
        assert_eq!(out.as_ptr(), ptr);
    }

    #[test]
    fn compatibility_wrapper_matches_fallible_reference() {
        let values = [1_u32, 2, 3, 4, 5, 6, 7, 8];
        let filter = NodeSetFilter::Intersects(0b1);
        let mut compat = Vec::with_capacity(4);
        let mut fallible = Vec::with_capacity(4);

        nodeset_filter_cpu_ref_into(&values, filter, &mut compat);
        try_nodeset_filter_cpu_ref_into(&values, filter, &mut fallible)
            .expect("Fix: small nodeset filter CPU reference must reserve");

        assert_eq!(compat, fallible);
        assert_eq!(nodeset_filter_cpu_ref(&values, filter), fallible);
    }

    #[test]
    fn production_wrapper_has_no_raw_panic_path() {
        let production = include_str!("nodeset_filter.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: nodeset_filter.rs must contain production section");

        assert!(
            !production.contains(".expect(") && !production.contains(".unwrap("),
            "Fix: nodeset filter CPU wrapper must not panic in production."
        );
    }
}
