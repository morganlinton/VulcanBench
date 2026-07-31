//! Pearl's do-calculus  -  graph surgery primitives.
//!
//! Pearl's three rules of do-calculus reduce a do-query `P(Y | do(X))`
//! to an observable-query `P(Y | X)` when the causal graph admits.
//! The Shpitser ID algorithm (2008) automates the rule application;
//! Correa-Bareinboim (2020) extends to multi-treatment identifiability.
//!
//! At the GPU primitive level, do-calculus reduces to **graph
//! surgery**  -  three primitive transformations on the adjacency matrix:
//!
//! 1. **Edge deletion**  -  `do(X = x)` removes incoming edges to X
//!    (parents no longer cause X; X is set externally).
//! 2. **Edge reversal**  -  needed when applying Rule 3 (action /
//!    observation exchange).
//! 3. **Subgraph extraction**  -  restrict to a node subset for backdoor
//!    / frontdoor adjustment.
//!
//! This file ships the **incoming-edge-deletion** primitive  -  the
//! most-used graph surgery, the heart of `do(X = x)`.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | `vyre-libs::causal` consumers | Pearl-style counterfactuals |
//! | `vyre-libs::security::what_if` consumers | "would finding fire under fix X?" counterfactual analysis |
//! | `vyre-foundation::transform` change-impact analysis | `do(rule_X)` on the rule dependency graph predicts which downstream Programs invalidate. Replaces ad-hoc cache-invalidation tracking with formal causal analysis. |

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::graph::do_intervention_delete_incoming";
/// Rule 2 op id.
pub const RULE2_OP_ID: &str = "vyre-primitives::graph::do_rule2_reverse_incoming";

/// Emit a Program that zeros all incoming edges to nodes marked
/// "intervened" in `intervention_mask`. The result is the post-do
/// adjacency matrix.
///
/// Inputs:
/// - `adjacency`: row-major `n × n` u32 buffer (entry `[i, j]` = edge
///   weight or 0/1 for unweighted).
/// - `intervention_mask`: `n` u32 lanes, `1` if node is do-intervened.
///
/// Output:
/// - `out_adjacency`: row-major `n × n` u32 buffer.
///
/// Per-cell rule: `out[i, j] = 0` if `intervention_mask[j] == 1`
/// (column j zeros out  -  incoming edges to j removed). Otherwise
/// `out[i, j] = adjacency[i, j]`.
#[must_use]
pub fn do_intervention_delete_incoming(
    adjacency: &str,
    intervention_mask: &str,
    out_adjacency: &str,
    n: u32,
) -> Program {
    match try_do_intervention_delete_incoming(adjacency, intervention_mask, out_adjacency, n) {
        Ok(program) => program,
        Err(error) => crate::invalid_output_program(OP_ID, out_adjacency, DataType::U32, error),
    }
}

/// Emit an incoming-edge-deletion Program with checked adjacency matrix shape.
pub fn try_do_intervention_delete_incoming(
    adjacency: &str,
    intervention_mask: &str,
    out_adjacency: &str,
    n: u32,
) -> Result<Program, String> {
    if n == 0 {
        return Err(format!(
            "Fix: do_intervention_delete_incoming requires n > 0, got {n}."
        ));
    }

    let cells = checked_square_cells(n, OP_ID)?;
    let t = Expr::InvocationId { axis: 0 };

    // Decode (i, j) from flat invocation t = i*n + j; only j matters.
    let j_expr = Expr::rem(t.clone(), Expr::u32(n));
    let intervened = Expr::load(intervention_mask, j_expr);
    let edge = Expr::load(adjacency, t.clone());
    let value = Expr::select(Expr::eq(intervened, Expr::u32(0)), edge, Expr::u32(0));

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(cells)),
        vec![Node::store(out_adjacency, t, value)],
    )];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(adjacency, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(cells),
            BufferDecl::storage(intervention_mask, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n),
            BufferDecl::storage(out_adjacency, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(cells),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    ))
}

/// CPU reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn do_intervention_delete_incoming_cpu(
    adjacency: &[u32],
    intervention_mask: &[u32],
    n: u32,
) -> Vec<u32> {
    try_do_intervention_delete_incoming_cpu(adjacency, intervention_mask, n).unwrap_or_else(|err| {
        panic!("do_intervention_delete_incoming CPU oracle received malformed input. {err}")
    })
}

/// Fallible CPU reference.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_do_intervention_delete_incoming_cpu(
    adjacency: &[u32],
    intervention_mask: &[u32],
    n: u32,
) -> Result<Vec<u32>, String> {
    let mut out = Vec::new();
    try_do_intervention_delete_incoming_cpu_into(adjacency, intervention_mask, n, &mut out)?;
    Ok(out)
}

/// CPU reference writing into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn do_intervention_delete_incoming_cpu_into(
    adjacency: &[u32],
    intervention_mask: &[u32],
    n: u32,
    out: &mut Vec<u32>,
) {
    try_do_intervention_delete_incoming_cpu_into(adjacency, intervention_mask, n, out)
        .unwrap_or_else(|err| {
            panic!("do_intervention_delete_incoming CPU oracle received malformed input. {err}")
        });
}

/// Fallible CPU reference writing into caller-owned storage.
///
/// On error, `out` is left unchanged so hostile-shape tests and parity
/// harnesses retain their last useful diagnostic matrix.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_do_intervention_delete_incoming_cpu_into(
    adjacency: &[u32],
    intervention_mask: &[u32],
    n: u32,
    out: &mut Vec<u32>,
) -> Result<(), String> {
    let n_us = n as usize;
    let cells = n_us.checked_mul(n_us).ok_or_else(|| {
        format!(
            "Fix: do-calculus intervention n*n overflows usize for n={n}; shard the causal graph before parity comparison."
        )
    })?;
    if adjacency.len() != cells {
        return Err(format!(
            "Fix: do-calculus intervention requires a complete n*n adjacency matrix: adjacency.len() == n*n, got len={} for n={n}.",
            adjacency.len()
        ));
    }
    if intervention_mask.len() != n_us {
        return Err(format!(
            "Fix: do-calculus intervention requires intervention_mask.len() == n, got len={} for n={n}.",
            intervention_mask.len()
        ));
    }
    if cells > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            cells - out.len(),
            "do-calculus intervention CPU oracle",
            "output adjacency",
        )?;
    }
    out.clear();
    out.extend_from_slice(adjacency);
    for j in 0..n_us {
        if intervention_mask[j] != 0 {
            for i in 0..n_us {
                out[i * n_us + j] = 0; // zero column j
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_no_intervention_preserves_adjacency() {
        let a = vec![1, 2, 3, 4];
        let mask = vec![0, 0];
        let out = do_intervention_delete_incoming_cpu(&a, &mask, 2);
        assert_eq!(out, a);
    }

    #[test]
    fn cpu_intervene_node_zero_zeros_column() {
        // 2-node graph, intervene on node 0.
        // Edge [0->0]=1, [0->1]=2, [1->0]=3, [1->1]=4
        // After do(0): incoming-to-0 zeroed → [0->0]=0, [1->0]=0 stay
        // existing: [0->1]=2, [1->1]=4
        let a = vec![1, 2, 3, 4];
        let mask = vec![1, 0];
        let out = do_intervention_delete_incoming_cpu(&a, &mask, 2);
        // column 0: out[0*2+0] = 0, out[1*2+0] = 0
        // column 1: out[0*2+1] = 2, out[1*2+1] = 4
        assert_eq!(out, vec![0, 2, 0, 4]);
    }

    #[test]
    fn cpu_intervene_all_zeros_all() {
        let a = vec![1, 2, 3, 4];
        let mask = vec![1, 1];
        let out = do_intervention_delete_incoming_cpu(&a, &mask, 2);
        assert_eq!(out, vec![0; 4]);
    }

    #[test]
    fn cpu_chain_graph_intervention_breaks_chain() {
        // Chain: 0 -> 1 -> 2.
        // Adjacency (row=from, col=to):
        //   [0,1]=1, [1,2]=1, others=0
        let a = vec![
            0, 1, 0, // row 0: edge to 1
            0, 0, 1, // row 1: edge to 2
            0, 0, 0, // row 2: no edges out
        ];
        // Intervene on node 1: "set node 1 externally" → break 0→1.
        let mask = vec![0, 1, 0];
        let out = do_intervention_delete_incoming_cpu(&a, &mask, 3);
        // column 1 zeroed: [0,1]=0
        // column 2 untouched: [1,2]=1
        assert_eq!(out[0 * 3 + 1], 0);
        assert_eq!(out[1 * 3 + 2], 1);
    }

    #[test]
    #[should_panic(expected = "complete n*n adjacency matrix")]
    fn cpu_malformed_inputs_fail_loudly() {
        let _ = do_intervention_delete_incoming_cpu(&[1], &[1], 2);
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = do_intervention_delete_incoming("a", "m", "out", 4);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["a", "m", "out"]);
        assert_eq!(p.buffers[0].count(), 16); // n*n
        assert_eq!(p.buffers[1].count(), 4); // n
        assert_eq!(p.buffers[2].count(), 16); // n*n
    }

    #[test]
    fn zero_n_traps() {
        let p = do_intervention_delete_incoming("a", "m", "o", 0);
        assert!(p.stats().trap());
    }

    #[test]
    fn checked_delete_incoming_rejects_zero_n() {
        let error = try_do_intervention_delete_incoming("a", "m", "out", 0)
            .expect_err("checked do-intervention builder must reject n=0");

        assert!(
            error.contains("requires n > 0"),
            "error should describe the invalid causal graph shape: {error}"
        );
    }

    #[test]
    fn checked_delete_incoming_rejects_adjacency_cell_overflow() {
        let error = try_do_intervention_delete_incoming("a", "m", "out", u32::MAX)
            .expect_err("checked do-intervention builder must reject n*n overflow");

        assert!(
            error.contains("overflows adjacency cell count"),
            "error should describe the adjacency matrix overflow: {error}"
        );
    }

    #[test]
    fn legacy_delete_incoming_does_not_panic_on_adjacency_cell_overflow() {
        let program = do_intervention_delete_incoming("a", "m", "out", u32::MAX);

        assert!(program.stats().trap());
    }

    #[test]
    fn delete_incoming_builder_source_has_checked_api_without_panics() {
        let source = include_str!("do_calculus.rs");
        let builder_source = source
            .split("/// Emit a Program that zeros all incoming edges")
            .nth(1)
            .expect("Fix: do-intervention builder source must be present")
            .split("/// CPU reference.")
            .next()
            .expect("Fix: do-intervention builder source must precede CPU oracle");

        assert!(
            builder_source.contains("pub fn try_do_intervention_delete_incoming(")
                && !builder_source.contains(concat!("panic", "!("))
                && !builder_source.contains(".unwrap_or_else("),
            "Fix: do_intervention_delete_incoming must expose checked release API and avoid production panics."
        );
    }
}

// ===== P-PRIM-7: Rules 2 and 3 of do-calculus =====================
//
// Pearl's three rules act on a causal graph G with treatment X,
// outcome Y, and conditioning set Z:
//
//   Rule 1 (insertion/deletion of observation): if Z is conditionally
//          independent of Y given X in the mutilated graph, you can
//          drop it from the conditioning set. Implemented via the
//          do_intervention_delete_incoming primitive above + a
//          d-separation check (callers compose these).
//   Rule 2 (action / observation exchange): in the graph with edges
//          INTO X removed, observation Y | Z, X equals
//          Y | Z, do(X). Implemented as edge reversal: a treatment-
//          set's incoming edges are reversed in the working
//          adjacency.
//   Rule 3 (insertion/deletion of action): in the graph with X →·
//          edges removed (downstream of X), do(X) has no effect on
//          ancestors. Implemented as subgraph extraction.
//
// This block ships the CPU references for Rule 2 and Rule 3 so the
// substrate file actually contains all three rules. Rule 1's "remove
// incoming edges" surgery is the existing
// `do_intervention_delete_incoming_cpu`.

/// Rule 2 (do-calculus)  -  edge reversal on incoming edges of treatment
/// nodes. Reverses every edge `i → j` where `treatment_mask[j] != 0`
/// to `j → i`. Pre-existing reverse edges are merged via OR.
///
/// Returns the reversed adjacency matrix.
#[must_use]
pub fn do_rule2_reverse_incoming(
    adjacency: &str,
    treatment_mask: &str,
    out_adjacency: &str,
    n: u32,
) -> Program {
    match try_do_rule2_reverse_incoming(adjacency, treatment_mask, out_adjacency, n) {
        Ok(program) => program,
        Err(error) => {
            crate::invalid_output_program(RULE2_OP_ID, out_adjacency, DataType::U32, error)
        }
    }
}

/// Emit a Rule 2 incoming-edge-reversal Program with checked adjacency matrix
/// shape.
pub fn try_do_rule2_reverse_incoming(
    adjacency: &str,
    treatment_mask: &str,
    out_adjacency: &str,
    n: u32,
) -> Result<Program, String> {
    if n == 0 {
        return Err(format!(
            "Fix: do_rule2_reverse_incoming requires n > 0, got {n}."
        ));
    }

    let cells = checked_square_cells(n, RULE2_OP_ID)?;
    let t = Expr::InvocationId { axis: 0 };
    let row = Expr::div(t.clone(), Expr::u32(n));
    let col = Expr::rem(t.clone(), Expr::u32(n));
    let not_self = Expr::ne(row.clone(), col.clone());
    let original = Expr::load(adjacency, t.clone());
    let col_treated = Expr::ne(Expr::load(treatment_mask, col.clone()), Expr::u32(0));
    let row_treated = Expr::ne(Expr::load(treatment_mask, row.clone()), Expr::u32(0));
    let reverse_idx = Expr::add(Expr::mul(col, Expr::u32(n)), row);
    let kept_original = Expr::select(
        Expr::and(col_treated, not_self.clone()),
        Expr::u32(0),
        original,
    );
    let reversed_in = Expr::select(
        Expr::and(row_treated, not_self),
        Expr::load(adjacency, reverse_idx),
        Expr::u32(0),
    );
    let value = Expr::bitor(kept_original, reversed_in);

    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(cells)),
        vec![Node::store(out_adjacency, t, value)],
    )];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(adjacency, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(cells),
            BufferDecl::storage(treatment_mask, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n),
            BufferDecl::storage(out_adjacency, 2, BufferAccess::ReadWrite, DataType::U32)
                .with_count(cells),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(RULE2_OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    ))
}

fn checked_square_cells(n: u32, op_id: &'static str) -> Result<u32, String> {
    n.checked_mul(n).ok_or_else(|| {
        format!(
            "{op_id} n={n} overflows adjacency cell count. Fix: shard the causal graph before GPU dispatch."
        )
    })
}

/// Rule 2 CPU reference.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn do_rule2_reverse_incoming_cpu(
    adjacency: &[u32],
    treatment_mask: &[u32],
    n: u32,
) -> Vec<u32> {
    try_do_rule2_reverse_incoming_cpu(adjacency, treatment_mask, n).unwrap_or_else(|err| {
        panic!("do_rule2_reverse_incoming CPU oracle received malformed input. {err}")
    })
}

/// Fallible rule-2 CPU reference.
#[cfg(any(test, feature = "cpu-parity"))]

pub fn try_do_rule2_reverse_incoming_cpu(
    adjacency: &[u32],
    treatment_mask: &[u32],
    n: u32,
) -> Result<Vec<u32>, String> {
    let mut out = Vec::new();
    try_do_rule2_reverse_incoming_cpu_into(adjacency, treatment_mask, n, &mut out)?;
    Ok(out)
}

/// Rule 2 CPU reference writing into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn do_rule2_reverse_incoming_cpu_into(
    adjacency: &[u32],
    treatment_mask: &[u32],
    n: u32,
    out: &mut Vec<u32>,
) {
    try_do_rule2_reverse_incoming_cpu_into(adjacency, treatment_mask, n, out).unwrap_or_else(
        |err| panic!("do_rule2_reverse_incoming CPU oracle received malformed input. {err}"),
    );
}

/// Fallible rule-2 CPU reference writing into caller-owned storage.
///
/// On error, `out` is left unchanged so parity harnesses can retain the last
/// useful diagnostic matrix instead of losing it to a shape-preflight failure.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_do_rule2_reverse_incoming_cpu_into(
    adjacency: &[u32],
    treatment_mask: &[u32],
    n: u32,
    out: &mut Vec<u32>,
) -> Result<(), String> {
    let n_us = n as usize;
    let expected_adjacency = n_us
        .checked_mul(n_us)
        .ok_or_else(|| format!("Fix: do-calculus rule2 n*n overflows usize for n={n}."))?;
    if adjacency.len() != expected_adjacency {
        return Err(format!(
            "Fix: do-calculus rule2 requires adjacency.len() == n*n, got len={} for n={n}.",
            adjacency.len()
        ));
    }
    if treatment_mask.len() != n_us {
        return Err(format!(
            "Fix: do-calculus rule2 requires treatment_mask.len() == n, got len={} for n={n}.",
            treatment_mask.len()
        ));
    }
    if expected_adjacency > out.capacity() {
        crate::graph::scratch::reserve_graph_items(
            out,
            expected_adjacency - out.len(),
            "do-calculus rule2 CPU oracle",
            "output adjacency",
        )?;
    }
    out.clear();
    out.resize(expected_adjacency, 0);
    for row in 0..n_us {
        for col in 0..n_us {
            let idx = row * n_us + col;
            if row == col {
                out[idx] = adjacency[idx];
                continue;
            }
            let mut value = 0;
            if treatment_mask[col] == 0 {
                value |= adjacency[idx];
            }
            if treatment_mask[row] != 0 {
                value |= adjacency[col * n_us + row];
            }
            out[idx] = value;
        }
    }
    Ok(())
}

/// Rule 3 (do-calculus)  -  subgraph extraction. Returns the adjacency
/// matrix restricted to nodes whose `keep_mask` bit is set. Edges
/// touching dropped nodes are removed; the result is laid out as
/// `k × k` where `k = popcount(keep_mask)`.
///
/// Returns `(reduced_adjacency, kept_index_to_original_index)`.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn do_rule3_subgraph_cpu(adjacency: &[u32], keep_mask: &[u32], n: u32) -> (Vec<u32>, Vec<u32>) {
    try_do_rule3_subgraph_cpu(adjacency, keep_mask, n).unwrap_or_else(|err| {
        panic!("do_rule3_subgraph CPU oracle received malformed input. {err}")
    })
}

/// Fallible rule-3 CPU reference.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_do_rule3_subgraph_cpu(
    adjacency: &[u32],
    keep_mask: &[u32],
    n: u32,
) -> Result<(Vec<u32>, Vec<u32>), String> {
    let mut reduced = Vec::new();
    let mut kept = Vec::new();
    try_do_rule3_subgraph_cpu_into(adjacency, keep_mask, n, &mut reduced, &mut kept)?;
    Ok((reduced, kept))
}

/// Fallible rule-3 CPU reference writing into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_do_rule3_subgraph_cpu_into(
    adjacency: &[u32],
    keep_mask: &[u32],
    n: u32,
    reduced: &mut Vec<u32>,
    kept: &mut Vec<u32>,
) -> Result<(), String> {
    let n_us = n as usize;
    let expected_adjacency = n_us
        .checked_mul(n_us)
        .ok_or_else(|| format!("Fix: do-calculus rule3 n*n overflows usize for n={n}."))?;
    if adjacency.len() != expected_adjacency {
        return Err(format!(
            "Fix: do-calculus rule3 requires adjacency.len() == n*n, got len={} for n={n}.",
            adjacency.len()
        ));
    }
    if keep_mask.len() != n_us {
        return Err(format!(
            "Fix: do-calculus rule3 requires keep_mask.len() == n, got len={} for n={n}.",
            keep_mask.len()
        ));
    }

    let kept_words = keep_mask.iter().filter(|&&m| m != 0).count();
    let reduced_words = kept_words.checked_mul(kept_words).ok_or_else(|| {
        format!("Fix: do-calculus rule3 reduced k*k overflows usize for k={kept_words}.")
    })?;
    if kept_words > kept.capacity() {
        crate::graph::scratch::reserve_graph_items(
            kept,
            kept_words - kept.len(),
            "do-calculus rule3 CPU oracle",
            "kept index map",
        )?;
    }
    if reduced_words > reduced.capacity() {
        crate::graph::scratch::reserve_graph_items(
            reduced,
            reduced_words - reduced.len(),
            "do-calculus rule3 CPU oracle",
            "reduced adjacency",
        )?;
    }
    kept.clear();
    kept.extend(keep_mask.iter().enumerate().filter_map(|(idx, &m)| {
        if m != 0 {
            Some(idx as u32)
        } else {
            None
        }
    }));
    let k = kept.len();
    reduced.clear();
    reduced.resize(reduced_words, 0);
    for (new_i, &old_i) in kept.iter().enumerate() {
        for (new_j, &old_j) in kept.iter().enumerate() {
            reduced[new_i * k + new_j] = adjacency[(old_i as usize) * n_us + (old_j as usize)];
        }
    }
    Ok(())
}

/// Rule 3 CPU reference writing into caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn do_rule3_subgraph_cpu_into(
    adjacency: &[u32],
    keep_mask: &[u32],
    n: u32,
    reduced: &mut Vec<u32>,
    kept: &mut Vec<u32>,
) {
    try_do_rule3_subgraph_cpu_into(adjacency, keep_mask, n, reduced, kept).unwrap_or_else(|err| {
        panic!("do_rule3_subgraph CPU oracle received malformed input. {err}")
    });
}

#[cfg(test)]
mod fallible_cpu_reference_tests {
    use super::*;

    #[test]
    fn try_intervention_rejects_bad_input_without_clobbering_output() {
        let mut out = vec![42, 7];

        let err = try_do_intervention_delete_incoming_cpu_into(&[1], &[1], 2, &mut out)
            .expect_err("malformed intervention adjacency must return a typed error");

        assert!(
            err.contains("adjacency.len() == n*n"),
            "Fix: intervention shape error must identify the adjacency contract, got: {err}"
        );
        assert_eq!(
            out,
            vec![42, 7],
            "failed intervention preflight must preserve caller-owned diagnostics"
        );
    }

    #[test]
    fn intervention_into_reuses_capacity_and_truncates_stale_tail() {
        let adjacency = vec![1, 2, 3, 4];
        let mask = vec![1, 0];
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&[99, 98, 97, 96, 95, 94, 93, 92]);
        let capacity = out.capacity();

        try_do_intervention_delete_incoming_cpu_into(&adjacency, &mask, 2, &mut out)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid intervention matrix should reuse caller-owned output");

        assert_eq!(out, vec![0, 2, 0, 4]);
        assert_eq!(out.capacity(), capacity);

        try_do_intervention_delete_incoming_cpu_into(&[5], &[1], 1, &mut out)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - smaller intervention matrix should truncate stale output");

        assert_eq!(out, vec![0]);
        assert_eq!(out.capacity(), capacity);
    }

    #[test]
    fn generated_try_intervention_matches_legacy_oracle() {
        for n in 1usize..=6 {
            let adjacency: Vec<u32> = (0..(n * n))
                .map(|idx| u32::from(((idx * 11 + n) % 5) == 0))
                .collect();
            let mask: Vec<u32> = (0..n)
                .map(|idx| u32::from(((idx * 3 + n) % 2) == 0))
                .collect();
            let legacy = do_intervention_delete_incoming_cpu(&adjacency, &mask, n as u32);
            let mut out = vec![u32::MAX];

            try_do_intervention_delete_incoming_cpu_into(&adjacency, &mask, n as u32, &mut out)
                .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - generated valid intervention matrices must pass fallible oracle");

            assert_eq!(
                out, legacy,
                "fallible intervention oracle diverged at n={n}"
            );
        }
    }

    #[test]
    fn try_rule2_rejects_bad_input_without_clobbering_output() {
        let mut out = vec![7, 11, 13];

        let err = try_do_rule2_reverse_incoming_cpu_into(&[1], &[1], 2, &mut out)
            .expect_err("malformed rule2 adjacency must return a typed error");

        assert!(
            err.contains("adjacency.len() == n*n"),
            "Fix: rule2 shape error must identify the adjacency contract, got: {err}"
        );
        assert_eq!(
            out,
            vec![7, 11, 13],
            "failed rule2 preflight must preserve caller-owned diagnostics"
        );
    }

    #[test]
    fn rule2_into_reuses_capacity_and_truncates_stale_tail() {
        let adjacency = vec![0, 1, 0, 0];
        let mask = vec![0, 1];
        let mut out = Vec::with_capacity(8);
        out.extend_from_slice(&[99, 98, 97, 96, 95, 94, 93, 92]);
        let capacity = out.capacity();

        try_do_rule2_reverse_incoming_cpu_into(&adjacency, &mask, 2, &mut out)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid rule2 matrix should reuse caller-owned output");

        assert_eq!(out, vec![0, 0, 1, 0]);
        assert_eq!(out.capacity(), capacity);

        try_do_rule2_reverse_incoming_cpu_into(&[7], &[1], 1, &mut out)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - smaller rule2 matrix should truncate stale output");

        assert_eq!(out, vec![7]);
        assert_eq!(out.capacity(), capacity);
    }

    #[test]
    fn generated_try_rule2_matches_legacy_oracle() {
        for n in 1usize..=6 {
            let mut adjacency = vec![0u32; n * n];
            for row in 0..n {
                for col in 0..n {
                    adjacency[row * n + col] = u32::from(((row * 3 + col * 5 + n) % 4) == 0);
                }
            }
            let treatment_mask: Vec<u32> = (0..n)
                .map(|idx| u32::from(((idx * 7 + n) % 3) == 0))
                .collect();
            let legacy = do_rule2_reverse_incoming_cpu(&adjacency, &treatment_mask, n as u32);
            let mut out = vec![u32::MAX];

            try_do_rule2_reverse_incoming_cpu_into(&adjacency, &treatment_mask, n as u32, &mut out)
                .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - generated valid rule2 matrices must pass fallible oracle");

            assert_eq!(out, legacy, "fallible rule2 oracle diverged at n={n}");
        }
    }

    #[test]
    fn try_rule3_returns_tuple_and_preserves_outputs_on_error() {
        let mut reduced = vec![0xA5, 0x5A];
        let mut kept = vec![3, 1];

        let err = try_do_rule3_subgraph_cpu_into(&[1], &[1, 0], 2, &mut reduced, &mut kept)
            .expect_err("malformed rule3 adjacency must return a typed error");

        assert!(
            err.contains("adjacency.len() == n*n"),
            "Fix: rule3 shape error must identify the adjacency contract, got: {err}"
        );
        assert_eq!(
            reduced,
            vec![0xA5, 0x5A],
            "failed rule3 preflight must preserve reduced adjacency diagnostics"
        );
        assert_eq!(
            kept,
            vec![3, 1],
            "failed rule3 preflight must preserve kept-index diagnostics"
        );

        let (valid_reduced, valid_kept) = try_do_rule3_subgraph_cpu(&[0, 1, 1, 0], &[1, 0], 2)
            .expect("Fix: replace expect with fallible API or document caller precondition; panic only on programmer error - valid rule3 tuple oracle must succeed");
        assert_eq!(valid_reduced, vec![0]);
        assert_eq!(valid_kept, vec![0]);
    }
}

#[cfg(test)]
mod rule2_tests {
    use super::*;

    #[test]
    fn no_treatment_preserves_adjacency() {
        let a = vec![0, 1, 0, 0];
        let mask = vec![0u32, 0];
        let out = do_rule2_reverse_incoming_cpu(&a, &mask, 2);
        assert_eq!(out, a);
    }

    #[test]
    fn single_treatment_reverses_incoming() {
        // 2 nodes; edge 0→1; treat node 1 → reverse to 1→0.
        let a = vec![
            0, 1, // row 0
            0, 0, // row 1
        ];
        let mask = vec![0u32, 1];
        let out = do_rule2_reverse_incoming_cpu(&a, &mask, 2);
        assert_eq!(out, vec![0, 0, 1, 0]);
    }

    #[test]
    fn reversal_or_merges_with_existing_reverse_edge() {
        // Bidirectional 0↔1 (both edges exist).
        // Treat node 1 → 0→1 reversed to 1→0; existing 1→0 stays.
        let a = vec![0, 1, 1, 0];
        let mask = vec![0u32, 1];
        let out = do_rule2_reverse_incoming_cpu(&a, &mask, 2);
        assert_eq!(out, vec![0, 0, 1, 0]);
    }

    #[test]
    fn self_edges_untouched() {
        let a = vec![1, 0, 0, 1];
        let mask = vec![1u32, 1];
        let out = do_rule2_reverse_incoming_cpu(&a, &mask, 2);
        // Self-edges are skipped; still 1 on the diagonal.
        assert_eq!(out, vec![1, 0, 0, 1]);
    }

    #[test]
    fn reversal_is_involution_under_double_treatment() {
        // Reversing twice on the same treatment set yields the
        // original adjacency (when no overlap with reverse edges).
        let a = vec![0, 1, 0, 0, 0, 1, 0, 0, 0];
        let mask = vec![1u32, 1, 1];
        let once = do_rule2_reverse_incoming_cpu(&a, &mask, 3);
        let twice = do_rule2_reverse_incoming_cpu(&once, &mask, 3);
        assert_eq!(twice, a);
    }

    #[test]
    fn bidirectional_fully_treated_preserves_both_edges_without_order_loss() {
        let a = vec![0, 1, 1, 0];
        let mask = vec![1u32, 1];
        let out = do_rule2_reverse_incoming_cpu(&a, &mask, 2);
        assert_eq!(out, a);
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = do_rule2_reverse_incoming("a", "m", "out", 4);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["a", "m", "out"]);
        assert_eq!(p.buffers[0].count(), 16);
        assert_eq!(p.buffers[1].count(), 4);
        assert_eq!(p.buffers[2].count(), 16);
    }

    #[test]
    fn checked_rule2_builder_rejects_adjacency_cell_overflow() {
        let error = try_do_rule2_reverse_incoming("a", "m", "out", u32::MAX)
            .expect_err("checked Rule 2 builder must reject n*n overflow");

        assert!(
            error.contains("overflows adjacency cell count"),
            "error should describe the adjacency matrix overflow: {error}"
        );
    }

    #[test]
    fn legacy_rule2_builder_does_not_panic_on_adjacency_cell_overflow() {
        let program = do_rule2_reverse_incoming("a", "m", "out", u32::MAX);

        assert!(program.stats().trap());
    }

    #[test]
    fn rule2_builder_source_has_checked_api_without_panics() {
        let source = include_str!("do_calculus.rs");
        let builder_source = source
            .split("pub fn do_rule2_reverse_incoming(")
            .nth(1)
            .expect("Fix: Rule 2 builder source must be present")
            .split("/// Rule 2 CPU reference.")
            .next()
            .expect("Fix: Rule 2 builder source must precede CPU oracle");

        assert!(
            builder_source.contains("pub fn try_do_rule2_reverse_incoming(")
                && !builder_source.contains(concat!("panic", "!("))
                && !builder_source.contains(".unwrap_or_else("),
            "Fix: do_rule2_reverse_incoming must expose checked release API and avoid production panics."
        );
    }
}

#[cfg(test)]

mod rule3_tests {
    use super::*;

    #[test]
    fn keep_all_returns_original() {
        let a = vec![0, 1, 1, 0];
        let mask = vec![1u32, 1];
        let (out, kept) = do_rule3_subgraph_cpu(&a, &mask, 2);
        assert_eq!(out, a);
        assert_eq!(kept, vec![0, 1]);
    }

    #[test]
    fn subgraph_into_reuses_buffers() {
        let a = vec![0, 1, 1, 0];
        let mask = vec![1u32, 1];
        let mut out = Vec::with_capacity(8);
        let mut kept = Vec::with_capacity(4);
        let out_capacity = out.capacity();
        let kept_capacity = kept.capacity();
        out.extend_from_slice(&[99, 98, 97, 96, 95, 94, 93, 92]);
        kept.extend_from_slice(&[9, 8, 7, 6]);
        do_rule3_subgraph_cpu_into(&a, &mask, 2, &mut out, &mut kept);
        assert_eq!(out.capacity(), out_capacity);
        assert_eq!(kept.capacity(), kept_capacity);
        assert_eq!(out, a);
        assert_eq!(kept, vec![0, 1]);

        do_rule3_subgraph_cpu_into(&a, &[1u32, 0], 2, &mut out, &mut kept);
        assert_eq!(out.capacity(), out_capacity);
        assert_eq!(kept.capacity(), kept_capacity);
        assert_eq!(out, vec![0]);
        assert_eq!(kept, vec![0]);
    }

    #[test]
    fn generated_try_rule3_subgraph_matches_kept_shape_contracts() {
        for n in 1u32..=64 {
            let adjacency: Vec<u32> = (0..n)
                .flat_map(|row| {
                    (0..n).map(move |col| {
                        if row == col {
                            0
                        } else {
                            ((row + 1) * 17 + (col + 1) * 31) & 1
                        }
                    })
                })
                .collect();
            for seed in 0u32..64 {
                let keep_mask: Vec<u32> = (0..n)
                    .map(|node| ((node.wrapping_mul(5) + seed) % 3 == 0) as u32)
                    .collect();
                let mut reduced = vec![0xCAFE_BABEu32; 3];
                let mut kept = vec![0xDEAD_BEEFu32; 2];
                try_do_rule3_subgraph_cpu_into(&adjacency, &keep_mask, n, &mut reduced, &mut kept)
                    .unwrap();
                let expected_kept: Vec<u32> = keep_mask
                    .iter()
                    .enumerate()
                    .filter_map(|(index, &keep)| (keep != 0).then_some(index as u32))
                    .collect();
                assert_eq!(kept, expected_kept);
                assert_eq!(reduced.len(), kept.len() * kept.len());
                for (new_i, &old_i) in kept.iter().enumerate() {
                    for (new_j, &old_j) in kept.iter().enumerate() {
                        assert_eq!(
                            reduced[new_i * kept.len() + new_j],
                            adjacency[(old_i as usize) * n as usize + old_j as usize]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn keep_none_returns_empty() {
        let a = vec![0, 1, 1, 0];
        let mask = vec![0u32, 0];
        let (out, kept) = do_rule3_subgraph_cpu(&a, &mask, 2);
        assert!(out.is_empty());
        assert!(kept.is_empty());
    }

    #[test]
    fn keep_one_extracts_self_loop_only() {
        let a = vec![1, 1, 1, 1];
        let mask = vec![1u32, 0];
        let (out, kept) = do_rule3_subgraph_cpu(&a, &mask, 2);
        assert_eq!(out, vec![1]);
        assert_eq!(kept, vec![0]);
    }

    #[test]
    fn keep_two_of_three_drops_middle() {
        // 3-node chain 0→1→2. Keep {0, 2} → 1×... wait k=2.
        // After dropping node 1, 0 and 2 share no edge directly.
        let a = vec![
            0, 1, 0, // row 0
            0, 0, 1, // row 1
            0, 0, 0, // row 2
        ];
        let mask = vec![1u32, 0, 1];
        let (out, kept) = do_rule3_subgraph_cpu(&a, &mask, 3);
        assert_eq!(out, vec![0, 0, 0, 0]);
        assert_eq!(kept, vec![0, 2]);
    }

    #[test]
    fn keep_preserves_edges_between_kept_nodes() {
        // 4-node graph. Keep {1, 3}.
        // Edge 1→3 exists; should appear in 2×2 reduced.
        let n = 4;
        let mut a = vec![0u32; (n * n) as usize];
        a[(1 * n + 3) as usize] = 7;
        a[(3 * n + 1) as usize] = 5;
        let mask = vec![0u32, 1, 0, 1];
        let (out, kept) = do_rule3_subgraph_cpu(&a, &mask, n);
        // Reduced indices: 1 → new 0, 3 → new 1. So 1→3 lands at out[0,1] = 7.
        assert_eq!(out, vec![0, 7, 5, 0]);
        assert_eq!(kept, vec![1, 3]);
    }
}
