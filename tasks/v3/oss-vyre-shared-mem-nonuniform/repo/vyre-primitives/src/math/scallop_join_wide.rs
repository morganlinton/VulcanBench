//! `scallop_join_wide`  -  Multi-word lineage extension of `scallop_join`.
//!
//! Extends `#39 scallop_join` from a 32-rule (single u32) capacity to
//! `W` rules per cell for `W ∈ {2, 4, 8}`. This allows up to 256-rule
//! provenance tracking for large Scallop programs or external analyzer closures.
//!
//! Emits `semiring_gemm_wide`-equivalent Lineage semantics inside a
//! GPU-resident fixpoint kernel. Small matrices use a block-local
//! convergence loop; large matrices expose split-visible GridSync phases for
//! multi-block CUDA dispatch.

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

use crate::math::scallop_persistent::{
    ceil_div_u32, wide_lineage_body, wide_lineage_grid_sync_body,
};

/// Stable registry id for the wide Scallop lineage join primitive.
pub const OP_ID: &str = "vyre-primitives::math::scallop_join_wide";
/// One lane per relation word in the wide lineage fixpoint wrapper.
pub const SCALLOP_JOIN_WIDE_WORKGROUP_SIZE: [u32; 3] = [256, 1, 1];

/// Dispatch grid for the wide Scallop kernel.
#[must_use]
pub const fn scallop_join_wide_dispatch_grid(_n: u32, _w: u32) -> [u32; 3] {
    let cells = _n.saturating_mul(_n);
    let blocks = ceil_div_u32(cells, SCALLOP_JOIN_WIDE_WORKGROUP_SIZE[0]);
    [if blocks == 0 { 1 } else { blocks }, 1, 1]
}

/// Emits a generic `M × K · K × N → M × N` matmul Program for `W`-wide lineage cells.
///
/// A cell has `w` contiguous `u32` words.
/// Under wide lineage, the combine operation is:
///   If ALL words of A are 0 OR ALL words of B are 0 -> result is all 0s.
///   Otherwise -> bitwise OR of A and B words.
/// Accumulate is bitwise OR.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn semiring_gemm_wide(
    a: &str,
    b: &str,
    c: &str,
    seed: Option<&str>,
    m: u32,
    n: u32,
    k: u32,
    w: u32,
) -> Program {
    let cells = m * n;
    let t = Expr::InvocationId { axis: 0 };

    let i_expr = Expr::div(t.clone(), Expr::u32(n));
    let j_expr = Expr::rem(t.clone(), Expr::u32(n));

    let mut body = vec![Node::let_bind("i", i_expr), Node::let_bind("j", j_expr)];

    // Initialize W accumulators. For Datalog fixpoint, we initialize
    // from the seed facts so the state grows monotonically.
    for word_idx in 0..w {
        if let Some(seed_name) = seed {
            let seed_idx = Expr::add(Expr::mul(t.clone(), Expr::u32(w)), Expr::u32(word_idx));
            body.push(Node::let_bind(
                format!("acc_{word_idx}"),
                Expr::load(seed_name, seed_idx),
            ));
        } else {
            body.push(Node::let_bind(format!("acc_{word_idx}"), Expr::u32(0)));
        }
    }

    // Inner loop kk from 0 to k
    let mut inner_loop_body = Vec::new();

    // Check if A cell is zero and B cell is zero (boolean logic)
    let mut a_is_zero = Expr::bool(true);
    let mut b_is_zero = Expr::bool(true);

    for word_idx in 0..w {
        let a_idx = Expr::add(
            Expr::mul(
                Expr::add(Expr::mul(Expr::var("i"), Expr::u32(k)), Expr::var("kk")),
                Expr::u32(w),
            ),
            Expr::u32(word_idx),
        );
        let b_idx = Expr::add(
            Expr::mul(
                Expr::add(Expr::mul(Expr::var("kk"), Expr::u32(n)), Expr::var("j")),
                Expr::u32(w),
            ),
            Expr::u32(word_idx),
        );

        inner_loop_body.push(Node::let_bind(
            format!("a_{word_idx}"),
            Expr::load(a, a_idx),
        ));
        inner_loop_body.push(Node::let_bind(
            format!("b_{word_idx}"),
            Expr::load(b, b_idx),
        ));

        a_is_zero = Expr::and(
            a_is_zero,
            Expr::eq(Expr::var(format!("a_{word_idx}")), Expr::u32(0)),
        );
        b_is_zero = Expr::and(
            b_is_zero,
            Expr::eq(Expr::var(format!("b_{word_idx}")), Expr::u32(0)),
        );
    }

    let either_zero = Expr::or(a_is_zero, b_is_zero);

    let mut combine_and_accumulate = Vec::new();
    for word_idx in 0..w {
        let combined = Expr::select(
            either_zero.clone(),
            Expr::u32(0),
            Expr::bitor(
                Expr::var(format!("a_{word_idx}")),
                Expr::var(format!("b_{word_idx}")),
            ),
        );
        combine_and_accumulate.push(Node::assign(
            format!("acc_{word_idx}"),
            Expr::bitor(Expr::var(format!("acc_{word_idx}")), combined),
        ));
    }

    inner_loop_body.extend(combine_and_accumulate);

    body.push(Node::loop_for(
        "kk",
        Expr::u32(0),
        Expr::u32(k),
        inner_loop_body,
    ));

    for word_idx in 0..w {
        let c_idx = Expr::add(Expr::mul(t.clone(), Expr::u32(w)), Expr::u32(word_idx));
        body.push(Node::store(c, c_idx, Expr::var(format!("acc_{word_idx}"))));
    }

    let if_block = vec![Node::if_then(Expr::lt(t.clone(), Expr::u32(cells)), body)];

    let mut buffers = vec![
        BufferDecl::storage(a, 0, BufferAccess::ReadOnly, DataType::U32).with_count(m * k * w),
        BufferDecl::storage(b, 1, BufferAccess::ReadOnly, DataType::U32).with_count(k * n * w),
        BufferDecl::storage(c, 2, BufferAccess::ReadWrite, DataType::U32).with_count(cells * w),
    ];
    if let Some(seed_name) = seed {
        if seed_name != a && seed_name != b && seed_name != c {
            buffers.push(
                BufferDecl::storage(seed_name, 3, BufferAccess::ReadOnly, DataType::U32)
                    .with_count(cells * w),
            );
        }
    }

    Program::wrapped(
        buffers,
        SCALLOP_JOIN_WIDE_WORKGROUP_SIZE,
        vec![Node::Region {
            generator: Ident::from(format!("anonymous::{OP_ID}::semiring_gemm_wide")),
            source_region: None,
            body: Arc::new(if_block),
        }],
    )
}

/// Fused Datalog-fixpoint Program for `W`-wide lineage.
#[must_use]
pub fn scallop_join_wide(
    state: &str,
    next: &str,
    join_rules: &str,
    changed: &str,
    n: u32,
    w: u32,
    max_iterations: u32,
) -> Program {
    if n == 0 {
        return crate::invalid_output_program(
            OP_ID,
            state,
            DataType::U32,
            "Fix: scallop_join_wide requires n > 0, got 0.".to_string(),
        );
    }
    if w == 0 {
        return crate::invalid_output_program(
            OP_ID,
            state,
            DataType::U32,
            "Fix: scallop_join_wide requires w > 0, got 0.".to_string(),
        );
    }
    if max_iterations == 0 {
        return crate::invalid_output_program(
            OP_ID,
            state,
            DataType::U32,
            "Fix: scallop_join_wide requires max_iterations > 0, got 0.".to_string(),
        );
    }

    let cells = n.saturating_mul(n);
    let words = cells.saturating_mul(w);

    let body = if cells <= SCALLOP_JOIN_WIDE_WORKGROUP_SIZE[0] {
        wide_lineage_body(
            state,
            next,
            join_rules,
            changed,
            n,
            w,
            cells,
            max_iterations,
            SCALLOP_JOIN_WIDE_WORKGROUP_SIZE[0],
        )
    } else {
        wide_lineage_grid_sync_body(
            state,
            next,
            join_rules,
            changed,
            n,
            w,
            cells,
            max_iterations,
        )
    };

    let entry: Vec<Node> = vec![Node::Region {
        generator: Ident::from(OP_ID),
        source_region: None,
        body: Arc::new(body),
    }];

    Program::wrapped(
        vec![
            BufferDecl::storage(state, 0, BufferAccess::ReadWrite, DataType::U32).with_count(words),
            BufferDecl::storage(next, 1, BufferAccess::ReadWrite, DataType::U32).with_count(words),
            BufferDecl::storage(changed, 2, BufferAccess::ReadWrite, DataType::U32).with_count(1),
            BufferDecl::storage(join_rules, 3, BufferAccess::ReadOnly, DataType::U32)
                .with_count(words),
        ],
        SCALLOP_JOIN_WIDE_WORKGROUP_SIZE,
        entry,
    )
}

/// CPU reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref(
    state: &[u32],
    join_rules: &[u32],
    n: u32,
    w: u32,
    max_iterations: u32,
) -> (Vec<u32>, u32) {
    let mut current = Vec::new();
    let mut next = Vec::new();
    let iters = cpu_ref_into(
        state,
        join_rules,
        n,
        w,
        max_iterations,
        &mut current,
        &mut next,
    );
    (current, iters)
}

/// CPU reference using caller-owned state and scratch buffers.
///
/// `current` is overwritten with the final wide relation matrix. `next` is
/// reused as the semiring GEMM target across iterations and calls.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn cpu_ref_into(
    state: &[u32],
    join_rules: &[u32],
    n: u32,
    w: u32,
    max_iterations: u32,
    current: &mut Vec<u32>,
    next: &mut Vec<u32>,
) -> u32 {
    let words = n
        .checked_mul(n)
        .and_then(|cells| cells.checked_mul(w))
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_else(|| {
            panic!(
                "scallop_join_wide CPU oracle n={n} w={w} overflows word count. Fix: shard the relation matrix before parity comparison."
            )
        });
    let width = w as usize;
    assert_eq!(
        state.len(),
        words,
        "scallop_join_wide CPU oracle received state_len={} for n={n} w={w}. Fix: pass a complete n*n*w state matrix before parity comparison.",
        state.len()
    );
    assert_eq!(
        join_rules.len(),
        words,
        "scallop_join_wide CPU oracle received join_rules_len={} for n={n} w={w}. Fix: pass a complete n*n*w rule matrix before parity comparison.",
        join_rules.len()
    );
    current.clear();
    current.extend_from_slice(state);
    next.clear();
    next.resize(words, 0);

    let cell_nonzero = |buffer: &[u32], start: usize| {
        let end = start.checked_add(width).unwrap_or_else(|| {
            panic!(
                "scallop_join_wide CPU oracle cell range overflow at start={start} width={width}. Fix: shard the relation matrix before parity comparison."
            )
        });
        buffer
            .get(start..end)
            .map(|cell| cell.iter().any(|&x| x != 0))
            .unwrap_or(false)
    };

    for iter in 0..max_iterations {
        next.fill(0);
        for i in 0..n {
            for j in 0..n {
                let c_idx = ((i * n + j) * w) as usize;
                for kk in 0..n {
                    let a_idx = ((i * n + kk) * w) as usize;
                    let b_idx = ((kk * n + j) * w) as usize;

                    if cell_nonzero(&current, a_idx) && cell_nonzero(join_rules, b_idx) {
                        for word_idx in 0..width {
                            let a_word = current[a_idx + word_idx];
                            let b_word = join_rules[b_idx + word_idx];
                            if let Some(dst) = next.get_mut(c_idx + word_idx) {
                                *dst |= a_word | b_word;
                            }
                        }
                    }
                }
            }
        }

        let mut changed = false;
        for (current_word, next_word) in current.iter_mut().zip(next.iter()) {
            let merged = *current_word | *next_word;
            if merged != *current_word {
                *current_word = merged;
                changed = true;
            }
        }

        if !changed {
            return iter;
        }
    }
    max_iterations
}

#[cfg(feature = "inventory-registry")]
inventory::submit! {
    crate::harness::OpEntry::new(
        OP_ID,
        || scallop_join_wide("state", "next", "join_rules", "changed", 2, 2, 4),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0, 0, 0b01, 0, 0, 0, 0, 0]), // state (2x2 cells, 2 words per cell)
                to_bytes(&[0, 0, 0, 0, 0, 0, 0, 0]), // next
                to_bytes(&[0]), // changed
                to_bytes(&[0, 0, 0, 0, 0, 0, 0, 0b10]), // join_rules
            ]]
        }),
        Some(|| {
            let to_bytes = |w: &[u32]| crate::wire::pack_u32_slice(w);
            vec![vec![
                to_bytes(&[0, 0, 0b01, 0b10, 0, 0, 0, 0]), // state
                to_bytes(&[0, 0, 0b01, 0b10, 0, 0, 0, 0]), // next
                to_bytes(&[0]),                            // changed
            ]]
        }),
    )
}
