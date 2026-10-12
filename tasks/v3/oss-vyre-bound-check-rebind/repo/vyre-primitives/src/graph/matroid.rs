//! Matroid intersection augmenting-path step.
//!
//! Edmonds (1970) matroid intersection finds the max independent set
//! in the intersection of two matroids  -  generalizes bipartite
//! matching, common spanning forests, scheduling. Recent work
//! (Chakrabarty-Lee-Sidford 2021) cuts the per-iteration cost via
//! sparse linear-system solves.
//!
//! At each iteration, the algorithm searches for an "augmenting path"
//! in an exchange graph. This file ships the **exchange-graph BFS
//! step** primitive  -  given the current independent set and the
//! exchange-edge masks, advance the BFS frontier one layer.
//!
//! # Why this primitive is dual-use
//!
//! | Consumer | Use |
//! |---|---|
//! | `vyre-libs::opt::scheduling` | combinatorial scheduling |
//! | `vyre-libs::opt::bipartite` | bipartite matching |
//! | `vyre-runtime/src/megakernel/planner.rs` (#22 self-consumer) | **vyre's megakernel scheduler**  -  fusion-grouping subject to memory + sync constraints IS a matroid intersection problem (graphic matroid × partition matroid) |

use std::sync::Arc;

use vyre_foundation::ir::model::expr::Ident;
use vyre_foundation::ir::{BufferAccess, BufferDecl, DataType, Expr, Node, Program};

/// Op id.
pub const OP_ID: &str = "vyre-primitives::graph::matroid_exchange_bfs_step";

/// Emit one BFS layer of the matroid-exchange graph.
///
/// Inputs:
/// - `frontier_in`: length-`n` u32 lanes  -  `1` if node is in the
///   current frontier.
/// - `exchange_adj`: row-major `n × n` u32  -  `1` if edge `(i, j)`
///   exists in the exchange graph (i.e. swapping i for j preserves
///   independence in both matroids).
/// - `visited`: length-`n` u32  -  `1` if node already reached.
///
/// Output:
/// - `frontier_out`: length-`n` u32  -  `1` for newly-reached nodes
///   in this BFS layer (excludes already-visited).
/// - `any_change`: single-element u32  -  `1` if frontier_out has any
///   set bits (caller uses to detect convergence).
#[must_use]
pub fn matroid_exchange_bfs_step(
    frontier_in: &str,
    exchange_adj: &str,
    visited: &str,
    frontier_out: &str,
    any_change: &str,
    n: u32,
) -> Program {
    match try_matroid_exchange_bfs_step(
        frontier_in,
        exchange_adj,
        visited,
        frontier_out,
        any_change,
        n,
    ) {
        Ok(program) => program,
        Err(error) => crate::invalid_output_program(OP_ID, frontier_out, DataType::U32, error),
    }
}

/// Emit one BFS layer of the matroid-exchange graph with checked dense-matrix
/// sizing.
pub fn try_matroid_exchange_bfs_step(
    frontier_in: &str,
    exchange_adj: &str,
    visited: &str,
    frontier_out: &str,
    any_change: &str,
    n: u32,
) -> Result<Program, String> {
    if n == 0 {
        return Err(format!(
            "Fix: matroid_exchange_bfs_step requires n > 0, got {n}."
        ));
    }
    let dense_cells = checked_dense_cells(n, OP_ID)?;

    let t = Expr::InvocationId { axis: 0 };

    // Lane t computes frontier_out[t]:
    //   1 iff (visited[t] == 0)  AND  ∃ k. frontier_in[k] == 1 AND adj[k, t] == 1
    let body = vec![Node::if_then(
        Expr::lt(t.clone(), Expr::u32(n)),
        vec![
            Node::let_bind("reached", Expr::u32(0)),
            Node::if_then(
                Expr::eq(Expr::load(visited, t.clone()), Expr::u32(0)),
                vec![Node::loop_for(
                    "k",
                    Expr::u32(0),
                    Expr::u32(n),
                    vec![Node::if_then(
                        Expr::and(
                            Expr::ne(Expr::load(frontier_in, Expr::var("k")), Expr::u32(0)),
                            Expr::ne(
                                Expr::load(
                                    exchange_adj,
                                    Expr::add(Expr::mul(Expr::var("k"), Expr::u32(n)), t.clone()),
                                ),
                                Expr::u32(0),
                            ),
                        ),
                        vec![Node::assign("reached", Expr::u32(1))],
                    )],
                )],
            ),
            Node::store(frontier_out, t.clone(), Expr::var("reached")),
            // Lane 0 also writes any_change OR-reduced. To keep the
            // primitive single-pass without atomics, we write a per-
            // lane bit and let lane 0 OR-reduce in a final loop.
            Node::if_then(
                Expr::eq(t.clone(), Expr::u32(0)),
                vec![
                    Node::let_bind("changed", Expr::u32(0)),
                    Node::loop_for(
                        "j",
                        Expr::u32(0),
                        Expr::u32(n),
                        vec![Node::if_then(
                            Expr::ne(Expr::load(frontier_out, Expr::var("j")), Expr::u32(0)),
                            vec![Node::assign("changed", Expr::u32(1))],
                        )],
                    ),
                    Node::store(any_change, Expr::u32(0), Expr::var("changed")),
                ],
            ),
        ],
    )];

    Ok(Program::wrapped(
        vec![
            BufferDecl::storage(frontier_in, 0, BufferAccess::ReadOnly, DataType::U32)
                .with_count(n),
            BufferDecl::storage(exchange_adj, 1, BufferAccess::ReadOnly, DataType::U32)
                .with_count(dense_cells),
            BufferDecl::storage(visited, 2, BufferAccess::ReadOnly, DataType::U32).with_count(n),
            BufferDecl::storage(frontier_out, 3, BufferAccess::ReadWrite, DataType::U32)
                .with_count(n),
            BufferDecl::storage(any_change, 4, BufferAccess::ReadWrite, DataType::U32)
                .with_count(1),
        ],
        [256, 1, 1],
        vec![Node::Region {
            generator: Ident::from(OP_ID),
            source_region: None,
            body: Arc::new(body),
        }],
    ))
}

fn checked_dense_cells(n: u32, op_id: &'static str) -> Result<u32, String> {
    n.checked_mul(n).ok_or_else(|| {
        format!(
            "{op_id} n={n} overflows dense exchange matrix size. Fix: shard the exchange graph before GPU dispatch."
        )
    })
}

/// CPU reference for one BFS layer.
#[must_use]
#[cfg(any(test, feature = "cpu-parity"))]
pub fn matroid_exchange_bfs_step_cpu(
    frontier_in: &[u32],
    exchange_adj: &[u32],
    visited: &[u32],
    n: usize,
) -> (Vec<u32>, bool) {
    try_matroid_exchange_bfs_step_cpu(frontier_in, exchange_adj, visited, n)
        .unwrap_or_else(|error| panic!("{error}"))
}

/// Fallible CPU reference for one BFS layer.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_matroid_exchange_bfs_step_cpu(
    frontier_in: &[u32],
    exchange_adj: &[u32],
    visited: &[u32],
    n: usize,
) -> Result<(Vec<u32>, bool), String> {
    let mut out = Vec::new();
    let any =
        try_matroid_exchange_bfs_step_cpu_into(frontier_in, exchange_adj, visited, n, &mut out)?;
    Ok((out, any))
}

/// Fallible CPU reference for one BFS layer using caller-owned storage.
#[cfg(any(test, feature = "cpu-parity"))]
pub fn try_matroid_exchange_bfs_step_cpu_into(
    frontier_in: &[u32],
    exchange_adj: &[u32],
    visited: &[u32],
    n: usize,
    out: &mut Vec<u32>,
) -> Result<bool, String> {
    if frontier_in.len() != n {
        return Err(format!(
            "matroid_exchange_bfs_step CPU oracle received frontier_len={} for n={n}. Fix: pass one frontier slot per matroid element.",
            frontier_in.len()
        ));
    }
    if visited.len() != n {
        return Err(format!(
            "matroid_exchange_bfs_step CPU oracle received visited_len={} for n={n}. Fix: pass one visited slot per matroid element.",
            visited.len()
        ));
    }
    let expected_adj = n.checked_mul(n).ok_or_else(|| {
        format!(
            "matroid_exchange_bfs_step CPU oracle n={n} overflows dense exchange matrix size. Fix: shard the exchange graph before parity comparison."
        )
    })?;
    if exchange_adj.len() != expected_adj {
        return Err(format!(
            "matroid_exchange_bfs_step CPU oracle received exchange_adj_len={} for n={n}. Fix: pass a complete n*n dense exchange matrix.",
            exchange_adj.len()
        ));
    }

    out.clear();
    resize_matroid_cpu_vec(out, n, 0u32, "matroid_exchange_bfs_step CPU output")?;
    let mut any = false;
    for j in 0..n {
        if visited[j] != 0 {
            continue;
        }
        for k in 0..n {
            let frontier = frontier_in[k];
            let exchange = exchange_adj[k * n + j];
            if frontier != 0 && exchange != 0 {
                out[j] = 1;
                any = true;
                break;
            }
        }
    }
    Ok(any)
}

#[cfg(any(test, feature = "cpu-parity"))]
fn resize_matroid_cpu_vec<T: Clone>(
    out: &mut Vec<T>,
    len: usize,
    value: T,
    context: &str,
) -> Result<(), String> {
    if len > out.len() {
        crate::graph::scratch::reserve_graph_items(
            out,
            len - out.len(),
            "matroid exchange BFS CPU oracle",
            context,
        )?;
    }
    out.resize(len, value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_one_step_advances() {
        // 3 nodes; frontier = {0}; edges 0→1 in exchange graph.
        let f = vec![1, 0, 0];
        let adj = vec![
            0, 1, 0, // 0 → 1
            0, 0, 0, 0, 0, 0,
        ];
        let v = vec![0, 0, 0];
        let (out, any) = matroid_exchange_bfs_step_cpu(&f, &adj, &v, 3);
        assert_eq!(out, vec![0, 1, 0]);
        assert!(any);
    }

    #[test]
    fn cpu_visited_blocks_re_advance() {
        let f = vec![1, 0, 0];
        let adj = vec![0, 1, 0, 0, 0, 0, 0, 0, 0];
        let v = vec![0, 1, 0]; // node 1 already visited
        let (out, any) = matroid_exchange_bfs_step_cpu(&f, &adj, &v, 3);
        assert_eq!(out, vec![0, 0, 0]);
        assert!(!any);
    }

    #[test]
    fn cpu_empty_frontier_no_change() {
        let f = vec![0; 3];
        let adj = vec![1; 9];
        let v = vec![0; 3];
        let (out, any) = matroid_exchange_bfs_step_cpu(&f, &adj, &v, 3);
        assert_eq!(out, vec![0; 3]);
        assert!(!any);
    }

    #[test]
    #[should_panic(expected = "one frontier slot per matroid element")]
    fn cpu_malformed_inputs_fail_loudly() {
        let _ = matroid_exchange_bfs_step_cpu(&[1], &[], &[], 2);
    }

    #[test]
    fn cpu_multiple_sources_advance_all_targets() {
        // frontier = {0, 1}; adj 0→2, 1→3.
        let f = vec![1, 1, 0, 0];
        let adj = vec![
            0, 0, 1, 0, // 0 → 2
            0, 0, 0, 1, // 1 → 3
            0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let v = vec![0; 4];
        let (out, _) = matroid_exchange_bfs_step_cpu(&f, &adj, &v, 4);
        assert_eq!(out, vec![0, 0, 1, 1]);
    }

    #[test]
    fn generated_cpu_oracle_matches_dense_bfs_reference() {
        let mut out = Vec::new();
        for case in 0..4096usize {
            let n = case % 17;
            let frontier_in: Vec<u32> = (0..n)
                .map(|idx| u32::from(((case >> (idx % 9)) + idx) % 3 == 0))
                .collect();
            let visited: Vec<u32> = (0..n)
                .map(|idx| u32::from(((case / 5) + idx * 7) % 5 == 0))
                .collect();
            let exchange_adj: Vec<u32> = (0..n * n)
                .map(|idx| u32::from(((idx * 11 + case * 3) % 13) < 4))
                .collect();

            let any = try_matroid_exchange_bfs_step_cpu_into(
                &frontier_in,
                &exchange_adj,
                &visited,
                n,
                &mut out,
            )
            .expect("Fix: caller must pre-size buffers; use fallible reserve or return ResourceExhausted - generated matroid CPU oracle should reserve and evaluate");
            let expected = independent_dense_bfs(&frontier_in, &exchange_adj, &visited, n);

            assert_eq!(out, expected.0, "case {case}: frontier_out mismatch");
            assert_eq!(any, expected.1, "case {case}: any_change mismatch");
        }
    }

    fn independent_dense_bfs(
        frontier_in: &[u32],
        exchange_adj: &[u32],
        visited: &[u32],
        n: usize,
    ) -> (Vec<u32>, bool) {
        let mut out = Vec::new();
        out.resize(n, 0);
        let mut any = false;
        for target in 0..n {
            if visited[target] != 0 {
                continue;
            }
            for source in 0..n {
                if frontier_in[source] != 0 && exchange_adj[source * n + target] != 0 {
                    out[target] = 1;
                    any = true;
                    break;
                }
            }
        }
        (out, any)
    }

    #[test]
    fn ir_program_buffer_layout() {
        let p = matroid_exchange_bfs_step("fi", "adj", "v", "fo", "ch", 4);
        assert_eq!(p.workgroup_size, [256, 1, 1]);
        let names: Vec<&str> = p.buffers.iter().map(|b| b.name()).collect();
        assert_eq!(names, vec!["fi", "adj", "v", "fo", "ch"]);
        assert_eq!(p.buffers[0].count(), 4);
        assert_eq!(p.buffers[1].count(), 16);
        assert_eq!(p.buffers[2].count(), 4);
        assert_eq!(p.buffers[3].count(), 4);
        assert_eq!(p.buffers[4].count(), 1);
    }

    #[test]
    fn zero_n_traps() {
        let p = matroid_exchange_bfs_step("fi", "adj", "v", "fo", "ch", 0);
        assert!(p.stats().trap());
    }

    #[test]
    fn checked_builder_rejects_dense_matrix_overflow() {
        let error = try_matroid_exchange_bfs_step("fi", "adj", "v", "fo", "ch", u32::MAX)
            .expect_err("checked matroid exchange BFS builder must reject n*n overflow");

        assert!(
            error.contains("overflows dense exchange matrix size"),
            "error should describe the dense matrix overflow: {error}"
        );
    }

    #[test]
    fn legacy_builder_does_not_panic_on_dense_matrix_overflow() {
        let program = matroid_exchange_bfs_step("fi", "adj", "v", "fo", "ch", u32::MAX);

        assert!(program.stats().trap());
    }

    #[test]
    fn matroid_builder_source_has_checked_api_without_panics() {
        let source = include_str!("matroid.rs");
        let builder_source = source
            .split("/// Emit one BFS layer of the matroid-exchange graph.")
            .nth(1)
            .expect("Fix: matroid exchange BFS builder source must be present")
            .split("/// CPU reference for one BFS layer.")
            .next()
            .expect("Fix: matroid exchange BFS builder source must precede CPU oracle");

        assert!(
            builder_source.contains("pub fn try_matroid_exchange_bfs_step(")
                && !builder_source.contains(concat!("panic", "!("))
                && !builder_source.contains(".unwrap_or_else("),
            "Fix: matroid_exchange_bfs_step must expose checked release API and avoid production panics."
        );
    }

    #[test]
    fn matroid_cpu_source_uses_fallible_reusable_frontier() {
        let source = include_str!("matroid.rs");
        let cpu_source = source
            .split("/// CPU reference for one BFS layer.")
            .nth(1)
            .expect("Fix: matroid CPU source must be present")
            .split("#[cfg(test)]")
            .next()
            .expect("Fix: matroid CPU source must precede tests");

        assert!(
            cpu_source.contains("try_matroid_exchange_bfs_step_cpu_into")
                && cpu_source.contains("crate::graph::scratch::reserve_graph_items")
                && !cpu_source.contains("fn reserve_matroid_cpu_vec")
                && !cpu_source.contains("vec![0u32; n]")
                && !cpu_source.contains("Vec::with_capacity")
                && !cpu_source.contains(".reserve("),
            "Fix: matroid CPU oracle must use fallible caller-owned frontier storage."
        );
    }
}
